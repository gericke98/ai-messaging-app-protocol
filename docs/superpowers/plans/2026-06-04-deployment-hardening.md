# Deployment / Single-Node Production Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make one `ainm-server` node safely deployable and restart-safe on a generic single-node Docker host: graceful SIGTERM/SIGINT drain of in-flight work, a fail-fast `AINM_ENV=production` secrets gate, and a workspace-correct production container + single-node compose.

**Architecture:** Three independent slices. Slice 1 adds `lifecycle.rs` (a `ShutdownController` over a `watch<bool>` signal + a `tokio_util::task::TaskTracker` of in-flight agent turns), installs a SIGTERM+SIGINT handler, wires `.with_graceful_shutdown(...)` into the `axum::serve` calls, drains the WebSocket connection loops via `select!`, and replaces the abrupt `abort()` teardown in `main.rs` with a bounded drain. Slice 2 adds `prod_guard.rs` (a pure `validate_with(getter)` core + an env-reading wrapper) called at the top of `run_federation`, exiting non-zero in production when required secrets/config are missing. Slice 3 relocates the Docker build to the repo root so the Cargo workspace resolves, installs `bubblewrap`, runs non-root, and adds a single-node prod compose + env template.

**Tech Stack:** Rust 2024 / Axum 0.8 / Tokio 1.x / `tokio-util` (TaskTracker) / Docker (multi-stage) / docker-compose.

**Reference spec:** `docs/superpowers/specs/2026-06-04-deployment-hardening-design.md`

---

## Pre-flight: branch

This work commits directly to `main` (the project's established pattern). Confirm you are on `main` and the tree is clean before starting:

```bash
cd /Users/santiagogerickeparga/Proyectos/ai-native-messaging
rtk proxy git status
```
Expected: `On branch main`, working tree clean (the untracked `dist/` dirs from the status snapshot are fine to ignore). **Use `rtk proxy git` for ALL git operations in this project — never bare `git`.**

---

## File Structure

| File | Responsibility | Action |
|------|----------------|--------|
| `server/src/lifecycle.rs` | `ShutdownController`, `wait_for_shutdown`, `install_signals` | Create |
| `server/src/prod_guard.rs` | `is_production`, `validate`, pure `validate_with` | Create |
| `server/src/main.rs` | call `prod_guard` at top of `run_federation`; build controller; install signals; bounded drain replacing `abort()` | Modify |
| `server/src/server.rs` | `shutdown: ShutdownController` field on `ServerState`; `.with_graceful_shutdown` on serve; `select!` drain in the two WS connection loops; turn dispatch via `spawn_turn` | Modify |
| `server/Cargo.toml` | add `tokio-util` dep | Modify |
| `Dockerfile` | workspace-correct multi-stage build, non-root, bubblewrap | Create (repo root) |
| `.dockerignore` | shrink build context | Create (repo root) |
| `docker-compose.prod.yml` | single-node app + Postgres | Create (repo root) |
| `.env.production.example` | documented required vars | Create (repo root) |
| `.gitignore` | ignore real `.env.production` | Modify |
| `server/docker-compose.yml` | repoint federation-demo build context to repo root | Modify |
| `server/Dockerfile` | delete (replaced by repo-root `Dockerfile`) | Delete |
| `docs/runbooks/deployment.md` | deploy/configure/stop runbook | Create |

---

## SLICE 1 — Graceful shutdown (`lifecycle`)

### Task 1: `ShutdownController` + `lifecycle` module

**Files:**
- Modify: `server/Cargo.toml` (add dep)
- Create: `server/src/lifecycle.rs`
- Modify: `server/src/main.rs` (add `mod lifecycle;`)

- [ ] **Step 1: Add the `tokio-util` dependency**

In `server/Cargo.toml`, under `[dependencies]` (near the existing `tokio = { version = "1", features = ["full"] }` on line 16), add:

```toml
tokio-util = { version = "0.7", features = ["rt"] }
```

(The `rt` feature is required for `task::TaskTracker`. `tokio-util` is already present transitively in the workspace `Cargo.lock`, so this only promotes it to a direct dependency.)

- [ ] **Step 2: Write the failing test (create `server/src/lifecycle.rs` with tests first)**

Create `server/src/lifecycle.rs` containing ONLY the test module so it fails to compile (types undefined):

```rust
//! Process lifecycle: graceful shutdown signalling + in-flight agent-turn draining.
//!
//! `ShutdownController` is the single coordination point. A `watch<bool>` carries the
//! "shutting down" signal to every WebSocket connection loop and to each `axum::serve`
//! call's `with_graceful_shutdown` future. A `tokio_util::task::TaskTracker` counts
//! in-flight agent turns (which are spawned as detached tasks in `server.rs`), so the
//! drain can wait for a turn already underway to deliver its signed reply before exit.

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn trigger_flips_flag_and_is_idempotent() {
        let c = ShutdownController::new();
        assert!(!c.is_shutting_down());
        c.trigger();
        assert!(c.is_shutting_down());
        c.trigger(); // second call must not panic
        assert!(c.is_shutting_down());
    }

    #[tokio::test]
    async fn wait_for_shutdown_returns_once_triggered() {
        let c = ShutdownController::new();
        let rx = c.subscribe();
        c.trigger();
        tokio::time::timeout(Duration::from_millis(100), wait_for_shutdown(rx))
            .await
            .expect("wait_for_shutdown returns promptly after trigger");
    }

    #[tokio::test]
    async fn wait_idle_pending_until_turn_completes() {
        let c = ShutdownController::new();
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        c.spawn_turn(async move {
            let _ = rx.await;
        });
        // While the turn is alive, the drain must not complete.
        assert!(
            tokio::time::timeout(Duration::from_millis(50), c.wait_idle())
                .await
                .is_err(),
            "wait_idle must stay pending while a turn is in flight"
        );
        // Release the turn; the drain now completes.
        let _ = tx.send(());
        tokio::time::timeout(Duration::from_millis(500), c.wait_idle())
            .await
            .expect("wait_idle drains after the turn completes");
    }
}
```

Add `mod lifecycle;` to `server/src/main.rs` (alphabetically near `mod llm;` line 30) AND to `server/src/lib.rs` if it declares modules (check: `grep -n 'pub mod\|^mod' server/src/lib.rs`; add `pub mod lifecycle;` matching the existing style so integration tests can reach it).

- [ ] **Step 3: Run the test to verify it fails**

Run: `cd server && cargo test --lib lifecycle 2>&1 | head -30`
Expected: FAIL — `cannot find type ShutdownController`, `cannot find function wait_for_shutdown`.

- [ ] **Step 4: Implement `ShutdownController` + `wait_for_shutdown`**

Prepend to `server/src/lifecycle.rs` (above the `#[cfg(test)]` module):

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::watch;
use tokio_util::task::TaskTracker;

/// Cloneable handle coordinating graceful shutdown across the server.
#[derive(Clone)]
pub struct ShutdownController {
    tx: Arc<watch::Sender<bool>>,
    flag: Arc<AtomicBool>,
    tracker: TaskTracker,
}

impl ShutdownController {
    pub fn new() -> Self {
        let (tx, _rx) = watch::channel(false);
        Self {
            tx: Arc::new(tx),
            flag: Arc::new(AtomicBool::new(false)),
            tracker: TaskTracker::new(),
        }
    }

    /// A receiver that observes the shutdown signal. Hand to `wait_for_shutdown`
    /// or `select!` on its `changed()`.
    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.tx.subscribe()
    }

    /// Signal shutdown. Idempotent.
    pub fn trigger(&self) {
        self.flag.store(true, Ordering::SeqCst);
        let _ = self.tx.send(true);
    }

    pub fn is_shutting_down(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// Spawn an in-flight agent turn as a tracked task. The drain (`wait_idle`)
    /// waits for every spawned turn to finish. Use this instead of `tokio::spawn`
    /// for agent turns so a turn already underway at shutdown still completes.
    pub fn spawn_turn<F>(&self, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.tracker.spawn(fut);
    }

    /// Close the tracker and await all in-flight turns. Idempotent: calling twice
    /// is safe (the second call observes an already-closed, possibly-empty tracker).
    pub async fn wait_idle(&self) {
        self.tracker.close();
        self.tracker.wait().await;
    }
}

impl Default for ShutdownController {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve once the shutdown signal becomes `true` (including if already set).
pub async fn wait_for_shutdown(mut rx: watch::Receiver<bool>) {
    if *rx.borrow() {
        return;
    }
    while rx.changed().await.is_ok() {
        if *rx.borrow_and_update() {
            return;
        }
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd server && cargo test --lib lifecycle 2>&1 | tail -20`
Expected: PASS — `trigger_flips_flag_and_is_idempotent`, `wait_for_shutdown_returns_once_triggered`, `wait_idle_pending_until_turn_completes` all ok.

- [ ] **Step 6: Commit**

```bash
rtk proxy git add server/Cargo.toml server/Cargo.lock server/src/lifecycle.rs server/src/main.rs server/src/lib.rs
rtk proxy git commit -m "$(cat <<'EOF'
feat(server): ShutdownController for graceful drain (lifecycle)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: SIGTERM + SIGINT handler

**Files:**
- Modify: `server/src/lifecycle.rs` (add `install_signals`)

- [ ] **Step 1: Implement `install_signals`**

Signal handling is not practically unit-testable (the test process can't safely raise SIGTERM to itself without aborting the runner), so this task ships the function with documentation rather than a unit test; it is exercised by the manual live run in Task 12. Append to `server/src/lifecycle.rs` (above the test module):

```rust
/// Spawn a task that triggers `controller` on the first SIGTERM or SIGINT.
///
/// Today the server only caught SIGINT (`ctrl_c`) — Docker/orchestrators send
/// SIGTERM, which had no handler, so the process was killed immediately. This
/// catches both. Unix only; on other targets it falls back to SIGINT alone
/// (the production target is Linux).
pub fn install_signals(controller: ShutdownController) {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut term = match signal(SignalKind::terminate()) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("[lifecycle] failed to install SIGTERM handler: {e}");
                    return;
                }
            };
            tokio::select! {
                _ = term.recv() => tracing::info!("[lifecycle] received SIGTERM; beginning graceful shutdown"),
                _ = tokio::signal::ctrl_c() => tracing::info!("[lifecycle] received SIGINT; beginning graceful shutdown"),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("[lifecycle] received Ctrl-C; beginning graceful shutdown");
        }
        controller.trigger();
    });
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd server && cargo build 2>&1 | tail -15`
Expected: builds clean (warnings about `install_signals` being unused are acceptable until Task 5 wires it).

- [ ] **Step 3: Commit**

```bash
rtk proxy git add server/src/lifecycle.rs
rtk proxy git commit -m "$(cat <<'EOF'
feat(server): install SIGTERM+SIGINT graceful-shutdown handler

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: `ShutdownController` on `ServerState` + `with_graceful_shutdown`

**Files:**
- Modify: `server/src/server.rs` (`ServerState` struct + 3 construction sites + 3 serve calls)

- [ ] **Step 1: Add the field to `ServerState`**

In `server/src/server.rs`, add to the `ServerState` struct (near `quota` at the end of the field list, ~line 260):

```rust
    /// Graceful-shutdown coordinator. Default (never triggered) for legacy/UAT start paths;
    /// the real one is threaded in by `run_federation` for the production listener.
    pub shutdown: crate::lifecycle::ShutdownController,
```

- [ ] **Step 2: Populate it at the two legacy construction sites**

In `start` (the state literal ending ~line 769) and `start_with_port` (~line 839), add to each `ServerState { ... }` literal:

```rust
        shutdown: crate::lifecycle::ShutdownController::new(),
```

- [ ] **Step 3: Thread a real controller into `start_federation`**

Add a parameter to `start_federation` (after `quota: crate::config::QuotaConfig,`, ~line 899):

```rust
    shutdown: crate::lifecycle::ShutdownController,
```

In its `ServerState { ... }` literal (~line 919), add `shutdown: shutdown.clone(),`.

- [ ] **Step 4: Wire `with_graceful_shutdown` into all three serve calls**

Replace each of the three spawned serve blocks. They currently read:

```rust
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
```

In `start` and `start_with_port`, capture the state's controller receiver BEFORE `.with_state(state)` is consumed — simplest is to clone a controller and subscribe. Since those paths use a fresh default controller, change them to:

```rust
    let shutdown_rx = state.shutdown.subscribe();
    // ... existing `.with_state(state);` line stays ...
    let listener = /* existing bind */;
    let addr = listener.local_addr()?;
    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(crate::lifecycle::wait_for_shutdown(shutdown_rx))
            .await
            .ok();
    });
```

For `start_federation`, subscribe from the passed-in `shutdown` before it is moved into the state literal:

```rust
    let shutdown_rx = shutdown.subscribe();
    // ServerState { ..., shutdown: shutdown.clone(), ... }  // (Step 3)
    // ... router built, listener bound ...
    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(crate::lifecycle::wait_for_shutdown(shutdown_rx))
            .await
            .ok();
    });
```

> Ordering detail: `let shutdown_rx = ...subscribe();` must appear before the value (`state` or `shutdown`) is moved. Place the `subscribe()` line immediately after the relevant value is in scope.

- [ ] **Step 5: Fix the one in-tree `start_federation` call site**

Find the caller in `main.rs`:

Run: `grep -n 'start_federation(' server/src/main.rs`

At that call (in `run_federation`), pass the controller as the new final argument. This depends on the controller being created in Task 5; for now add the argument as `controller.clone()` — Task 5 creates `controller`. To keep this task independently compilable, create the controller here too (Task 5 will move/dedupe it):

In `run_federation`, just before the `start_federation(...)` call, add:

```rust
    let controller = crate::lifecycle::ShutdownController::new();
```

and append `controller.clone()` as the final argument to `start_federation(...)`.

- [ ] **Step 6: Verify it compiles and existing tests pass**

Run: `cd server && cargo build 2>&1 | tail -20`
Expected: builds clean. Then:
Run: `cargo test --lib server:: 2>&1 | tail -15` (or the project's usual `cargo nextest run` subset)
Expected: existing server tests still pass (no behavior change yet — controller is never triggered).

- [ ] **Step 7: Commit**

```bash
rtk proxy git add server/src/server.rs server/src/main.rs
rtk proxy git commit -m "$(cat <<'EOF'
feat(server): thread ShutdownController into axum serve (graceful_shutdown)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: Drain the WebSocket connection loops + track turns

**Files:**
- Modify: `server/src/server.rs` (`handle_human_connection` ~2179, `handle_agent_connection` ~1603, turn dispatch ~2477)

- [ ] **Step 1: Convert the turn dispatch to a tracked spawn**

At `server/src/server.rs:~2477`, replace:

```rust
                                        tokio::spawn(async move {
                                            crate::runtime_server::handle_turn(&state2, &tid, &requester, &user_text).await;
                                        });
```

with:

```rust
                                        state.shutdown.spawn_turn(async move {
                                            crate::runtime_server::handle_turn(&state2, &tid, &requester, &user_text).await;
                                        });
```

(`state2` is the clone captured for the closure; `state.shutdown` is the outer state's controller. Both are valid here.)

- [ ] **Step 2: Add a shutdown arm to the human connection receive loop**

In `handle_human_connection` (~2234), the main loop currently is:

```rust
    // Main loop: receive human messages
    while let Some(Ok(msg)) = ws_receiver.next().await {
        if let Message::Text(text) = msg {
            // ... body ...
        }
    }
```

Replace the `while let` header with a `select!` that also wakes on shutdown. Subscribe once before the loop, then:

```rust
    // Main loop: receive human messages, or drain on shutdown.
    let mut shutdown_rx = state.shutdown.subscribe();
    if state.shutdown.is_shutting_down() {
        // Already shutting down before we entered the loop.
    } else {
        loop {
            tokio::select! {
                biased;
                _ = shutdown_rx.changed() => {
                    tracing::info!("[lifecycle] draining human connection {human_addr}");
                    break;
                }
                maybe = ws_receiver.next() => {
                    let Some(Ok(msg)) = maybe else { break; };
                    if let Message::Text(text) = msg {
                        // ... existing body, UNCHANGED ...
                    }
                }
            }
        }
    }
    // On break, close the socket cleanly so the client sees a Close frame.
    {
        let mut sender = ws_sender.lock().await;
        let _ = sender.send(Message::Close(None)).await;
    }
```

> The existing loop body (everything inside `if let Message::Text(text) = msg { ... }`) moves verbatim into the `maybe = ws_receiver.next()` arm. Do not change its logic. The trailing Close-frame send replaces the implicit drop at the old loop's end; if a Close was already sent on an error path, a second is harmless.

- [ ] **Step 3: Add the same shutdown arm to the agent connection receive loop**

`handle_agent_connection` (~1603) has an analogous `while let Some(Ok(msg)) = ws_receiver.next().await` receive loop. Apply the identical `select!` transformation (subscribe to `state.shutdown`, `biased` shutdown arm that breaks, body in the `next()` arm, Close frame on break). Use the agent's address in the log line.

- [ ] **Step 4: Verify it compiles and tests pass**

Run: `cd server && cargo build 2>&1 | tail -20`
Expected: clean build.
Run: `cargo test --lib 2>&1 | tail -15`
Expected: existing suite green (no triggered shutdown in unit tests).

- [ ] **Step 5: Commit**

```bash
rtk proxy git add server/src/server.rs
rtk proxy git commit -m "$(cat <<'EOF'
feat(server): drain WS connection loops on shutdown; track agent turns

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: Bounded drain sequence in `main.rs` + `AINM_SHUTDOWN_DRAIN_SECS`

**Files:**
- Modify: `server/src/main.rs` (`run_federation` controller wiring + the headless `human`/`agent` teardown arms)
- Create: integration test `server/tests/graceful_shutdown.rs`

- [ ] **Step 1: Write the failing integration test**

Create `server/tests/graceful_shutdown.rs`. This exercises the real drain primitive (`spawn_turn` + `wait_idle` under a timeout) — the exact logic `main.rs` runs — without standing up a full WS server (the full close-frame path is verified live in Task 12):

```rust
//! Verifies the graceful-drain primitive: a turn already in flight when shutdown
//! is triggered completes before `wait_idle` returns, and the bounded timeout
//! fires when a turn overruns the deadline.

use ainm_server::lifecycle::ShutdownController;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn drain_waits_for_in_flight_turn() {
    let controller = ShutdownController::new();
    let done = Arc::new(AtomicBool::new(false));
    let done2 = done.clone();

    controller.spawn_turn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        done2.store(true, Ordering::SeqCst);
    });

    controller.trigger();
    // Generous deadline: the turn (100ms) finishes well inside it.
    tokio::time::timeout(Duration::from_secs(5), controller.wait_idle())
        .await
        .expect("drains within deadline");
    assert!(done.load(Ordering::SeqCst), "in-flight turn ran to completion");
}

#[tokio::test]
async fn drain_deadline_bounds_an_overrunning_turn() {
    let controller = ShutdownController::new();
    controller.spawn_turn(async {
        tokio::time::sleep(Duration::from_secs(30)).await;
    });
    controller.trigger();
    // A short deadline must elapse rather than hang on the 30s turn.
    let r = tokio::time::timeout(Duration::from_millis(150), controller.wait_idle()).await;
    assert!(r.is_err(), "wait_idle is bounded by the caller's timeout");
}
```

> This test imports from the `ainm_server` library crate (lib name `ainm_server`, per `server/Cargo.toml` line 7). `ShutdownController` must be reachable as `ainm_server::lifecycle::ShutdownController` — ensure `pub mod lifecycle;` is in `server/src/lib.rs` (added in Task 1 Step 2).

- [ ] **Step 2: Run the test to verify it fails (or passes trivially) — then confirm wiring**

Run: `cd server && cargo test --test graceful_shutdown 2>&1 | tail -20`
Expected: PASS if Task 1 exposed `lifecycle` publicly; FAIL to compile (`unresolved import`) if `lib.rs` lacks `pub mod lifecycle;`. If it fails to compile, add `pub mod lifecycle;` to `server/src/lib.rs` and re-run until PASS.

- [ ] **Step 3: Wire the controller + signals + bounded drain into `run_federation`**

In `run_federation`, after `tracing_subscriber::fmt()....init();` (~line 241) the controller created in Task 3 Step 5 already exists as `let controller = ShutdownController::new();` just before the `start_federation` call. Move that line up to just after tracing init so it is in scope for the whole function, and install signals right after creating it:

```rust
    let controller = crate::lifecycle::ShutdownController::new();
    crate::lifecycle::install_signals(controller.clone());
```

Keep passing `controller.clone()` as the final `start_federation(...)` argument (Task 3 Step 5).

Add the drain-deadline read near the other env reads (e.g. just before the mode-branching match, ~line 797):

```rust
    let drain_secs: u64 = std::env::var("AINM_SHUTDOWN_DRAIN_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(25);
```

- [ ] **Step 4: Replace the headless `human` teardown with a bounded drain**

In the `"human"` arm's headless `else` branch (~line 818), the current code is:

```rust
                tokio::select! {
                    res = server_handle => {
                        if let Err(e) = res { tracing::error!("[human] server task ended: {e}"); }
                    }
                    _ = tokio::signal::ctrl_c() => {
                        tracing::info!("[human] received Ctrl-C/SIGINT; shutting down");
                    }
                }
                fed_handle.abort();
```

Replace it with:

```rust
                tokio::select! {
                    res = server_handle => {
                        if let Err(e) = res { tracing::error!("[human] server task ended: {e}"); }
                        controller.trigger();
                    }
                    _ = crate::lifecycle::wait_for_shutdown(controller.subscribe()) => {
                        tracing::info!("[human] shutdown signal; draining in-flight turns (deadline {drain_secs}s)");
                    }
                }
                match tokio::time::timeout(
                    std::time::Duration::from_secs(drain_secs),
                    controller.wait_idle(),
                ).await {
                    Ok(()) => tracing::info!("[human] drained cleanly"),
                    Err(_) => tracing::warn!("[human] drain deadline exceeded; forcing shutdown"),
                }
                fed_handle.abort();
```

- [ ] **Step 5: Apply the same bounded drain to the `agent` mode teardown**

The `"agent"` arm (~line 835 onward) ends by `abort()`-ing its handles (around lines 1334–1335). Before those aborts, insert the same signal-wait + bounded `wait_idle` drain so agent-mode deploys also drain. If the agent arm has its own `select!`/loop on `agent_handle`, add a `_ = crate::lifecycle::wait_for_shutdown(controller.subscribe())` arm and then the `timeout(drain_secs, controller.wait_idle())` block before the existing `server_handle.abort(); agent_handle.abort();`.

- [ ] **Step 6: Build, run the integration test, and verify the dev path still boots**

Run: `cd server && cargo build 2>&1 | tail -15`
Expected: clean.
Run: `cargo test --test graceful_shutdown 2>&1 | tail -15`
Expected: both tests PASS.

Manual smoke (dev path unaffected, SIGTERM now drains): in one terminal:
```bash
AINM_LLM__MOCK=1 AINM_DATABASE__URL=postgres://santiagogerickeparga@localhost:5432/ainm \
  cargo run --bin ainm-server -- --mode human --port 8080 --identity localhost \
  --cert server/certs/localhost.pem --key server/certs/localhost-key.pem \
  --ca-cert server/certs/ca.pem < /dev/null
```
(Use the same cert paths `scripts/dogfood.sh` uses — read that script for exact paths.) Then in another terminal send SIGTERM:
```bash
kill -TERM $(pgrep -f 'ainm-server --mode human')
```
Expected in the server log: `received SIGTERM; beginning graceful shutdown` → `drained cleanly` → process exits 0. (Before this slice, SIGTERM killed it with no log line.)

- [ ] **Step 7: Commit**

```bash
rtk proxy git add server/src/main.rs server/tests/graceful_shutdown.rs
rtk proxy git commit -m "$(cat <<'EOF'
feat(server): bounded SIGTERM drain in run_federation (AINM_SHUTDOWN_DRAIN_SECS)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## SLICE 2 — Production secrets gate (`prod_guard`)

### Task 6: `prod_guard` module (pure `validate_with` + env wrapper)

**Files:**
- Create: `server/src/prod_guard.rs`
- Modify: `server/src/main.rs` (add `mod prod_guard;`) and `server/src/lib.rs` (`pub mod prod_guard;`)

- [ ] **Step 1: Write the failing tests (pure core, no env races)**

Create `server/src/prod_guard.rs` with the test module first:

```rust
//! Production configuration gate. When `AINM_ENV=production`, the server must refuse
//! to boot if required secrets/config are missing or unsafe, rather than silently
//! falling back (random token secret, BYO disabled, mock email/LLM). The validation
//! core is pure (`validate_with`) so it is testable without mutating process env.

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn good() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("AINM_TOKEN_SECRET".into(), "x".repeat(32));
        m.insert("AINM_LLM_ENCRYPTION_KEY".into(), "aGVsbG8gd29ybGQ=".into()); // valid base64
        m.insert("AINM_DATABASE__URL".into(), "postgres://u@h/db".into());
        m.insert("AINM_EMAIL__RESEND_API_KEY".into(), "re_123".into());
        m.insert("AINM_EMAIL__FROM".into(), "noreply@example.com".into());
        m
    }

    fn getter(m: HashMap<String, String>) -> impl Fn(&str) -> Option<String> {
        move |k: &str| m.get(k).cloned()
    }

    #[test]
    fn all_present_is_ok() {
        assert!(validate_with(&getter(good())).is_ok());
    }

    #[test]
    fn missing_token_secret_is_reported() {
        let mut m = good();
        m.remove("AINM_TOKEN_SECRET");
        let err = validate_with(&getter(m)).unwrap_err();
        assert!(err.iter().any(|p| p.contains("AINM_TOKEN_SECRET")));
    }

    #[test]
    fn short_token_secret_is_reported() {
        let mut m = good();
        m.insert("AINM_TOKEN_SECRET".into(), "short".into());
        let err = validate_with(&getter(m)).unwrap_err();
        assert!(err.iter().any(|p| p.contains("AINM_TOKEN_SECRET")));
    }

    #[test]
    fn invalid_encryption_key_base64_is_reported() {
        let mut m = good();
        m.insert("AINM_LLM_ENCRYPTION_KEY".into(), "not valid base64!!!".into());
        let err = validate_with(&getter(m)).unwrap_err();
        assert!(err.iter().any(|p| p.contains("AINM_LLM_ENCRYPTION_KEY")));
    }

    #[test]
    fn missing_database_url_is_reported() {
        let mut m = good();
        m.remove("AINM_DATABASE__URL");
        let err = validate_with(&getter(m)).unwrap_err();
        assert!(err.iter().any(|p| p.contains("AINM_DATABASE__URL")));
    }

    #[test]
    fn mock_llm_in_production_is_rejected() {
        let mut m = good();
        m.insert("AINM_LLM__MOCK".into(), "1".into());
        let err = validate_with(&getter(m)).unwrap_err();
        assert!(err.iter().any(|p| p.contains("AINM_LLM__MOCK")));
    }

    #[test]
    fn missing_email_config_is_reported() {
        let mut m = good();
        m.remove("AINM_EMAIL__FROM");
        let err = validate_with(&getter(m)).unwrap_err();
        assert!(err.iter().any(|p| p.contains("AINM_EMAIL")));
    }

    #[test]
    fn problems_aggregate_not_first_fail() {
        let mut m = good();
        m.remove("AINM_TOKEN_SECRET");
        m.remove("AINM_DATABASE__URL");
        let err = validate_with(&getter(m)).unwrap_err();
        assert!(err.iter().any(|p| p.contains("AINM_TOKEN_SECRET")));
        assert!(err.iter().any(|p| p.contains("AINM_DATABASE__URL")));
    }
}
```

Add `mod prod_guard;` to `server/src/main.rs` (near `mod persistence;`) and `pub mod prod_guard;` to `server/src/lib.rs`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd server && cargo test --lib prod_guard 2>&1 | head -20`
Expected: FAIL — `cannot find function validate_with`.

- [ ] **Step 3: Implement the gate**

Prepend to `server/src/prod_guard.rs` (above the test module):

```rust
use base64::Engine;

/// True when `AINM_ENV=production`.
pub fn is_production() -> bool {
    std::env::var("AINM_ENV").as_deref() == Ok("production")
}

/// Validate production config against the live process environment.
pub fn validate() -> Result<(), Vec<String>> {
    validate_with(&|k| std::env::var(k).ok())
}

/// Pure validation core. `get(name)` returns the env value or `None`.
/// Aggregates ALL problems so one boot attempt reports everything to fix.
/// Never echoes secret material — only names and length/format facts.
pub fn validate_with(get: &dyn Fn(&str) -> Option<String>) -> Result<(), Vec<String>> {
    let mut problems = Vec::new();

    match get("AINM_TOKEN_SECRET") {
        Some(s) if s.len() >= 32 => {}
        Some(_) => problems.push(
            "AINM_TOKEN_SECRET must be at least 32 bytes (used to sign enrollment/recovery tokens)".to_string(),
        ),
        None => problems.push("AINM_TOKEN_SECRET is required in production".to_string()),
    }

    match get("AINM_LLM_ENCRYPTION_KEY") {
        Some(b64) => match base64::engine::general_purpose::STANDARD.decode(b64.trim()) {
            Ok(bytes) if !bytes.is_empty() => {}
            Ok(_) => problems.push("AINM_LLM_ENCRYPTION_KEY decoded to empty bytes".to_string()),
            Err(_) => problems.push("AINM_LLM_ENCRYPTION_KEY must be valid base64".to_string()),
        },
        None => problems.push("AINM_LLM_ENCRYPTION_KEY is required in production".to_string()),
    }

    if get("AINM_DATABASE__URL").map(|s| s.trim().is_empty()).unwrap_or(true) {
        problems.push("AINM_DATABASE__URL is required in production".to_string());
    }

    if matches!(get("AINM_LLM__MOCK").as_deref(), Some("1") | Some("true")) {
        problems.push("AINM_LLM__MOCK must not be set in production".to_string());
    }

    let has_resend = get("AINM_EMAIL__RESEND_API_KEY").map(|s| !s.trim().is_empty()).unwrap_or(false);
    let has_from = get("AINM_EMAIL__FROM").map(|s| !s.trim().is_empty()).unwrap_or(false);
    if !(has_resend && has_from) {
        problems.push(
            "AINM_EMAIL__RESEND_API_KEY and AINM_EMAIL__FROM are both required in production (otherwise OTPs are only logged)".to_string(),
        );
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd server && cargo test --lib prod_guard 2>&1 | tail -20`
Expected: all 8 tests PASS.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add server/src/prod_guard.rs server/src/main.rs server/src/lib.rs
rtk proxy git commit -m "$(cat <<'EOF'
feat(server): prod_guard production secrets/config validation

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 7: Enforce the gate at boot

**Files:**
- Modify: `server/src/main.rs` (`run_federation`, just after tracing init)

- [ ] **Step 1: Call the gate before binding anything**

In `run_federation`, immediately after `tracing_subscriber::fmt()....init();` and the controller/signal lines from Task 5 Step 3, add:

```rust
    if crate::prod_guard::is_production() {
        if let Err(problems) = crate::prod_guard::validate() {
            eprintln!("[config] refusing to boot in production — fix these and restart:");
            for p in &problems {
                eprintln!("[config]   - {p}");
            }
            std::process::exit(1);
        }
        tracing::info!("[config] production configuration validated");
    }
```

- [ ] **Step 2: Verify the dev path is untouched**

Run: `cd server && cargo build 2>&1 | tail -10`
Expected: clean.

Dev (no `AINM_ENV`) still boots with fallbacks:
```bash
AINM_LLM__MOCK=1 AINM_DATABASE__URL=postgres://santiagogerickeparga@localhost:5432/ainm \
  cargo run --bin ainm-server -- --mode human --port 8080 --identity localhost \
  --cert server/certs/localhost.pem --key server/certs/localhost-key.pem --ca-cert server/certs/ca.pem < /dev/null 2>&1 | head -20
```
Expected: boots (warns about random token secret, as before). Ctrl-C to stop.

Production with missing secrets refuses to boot:
```bash
AINM_ENV=production cargo run --bin ainm-server -- --mode human --port 8080 --identity localhost \
  --cert server/certs/localhost.pem --key server/certs/localhost-key.pem --ca-cert server/certs/ca.pem < /dev/null; echo "exit=$?"
```
Expected: prints `[config] refusing to boot in production` + the bullet list, `exit=1`.

- [ ] **Step 3: Commit**

```bash
rtk proxy git add server/src/main.rs
rtk proxy git commit -m "$(cat <<'EOF'
feat(server): fail-fast production secrets gate (AINM_ENV=production)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## SLICE 3 — Container & compose

### Task 8: Workspace-correct repo-root Dockerfile + `.dockerignore`

**Files:**
- Create: `Dockerfile` (repo root)
- Create: `.dockerignore` (repo root)
- Delete: `server/Dockerfile`

- [ ] **Step 1: Create `.dockerignore` at the repo root**

```
target/
**/target/
ios/
website/
packages/
**/node_modules/
**/dist/
.git/
.github/
tmp/
.planning/
docs/
*.log
.DS_Store
```

(Keep `crates/`, `server/`, root `Cargo.toml`, root `Cargo.lock` — those are needed by the build.)

- [ ] **Step 2: Create `Dockerfile` at the repo root**

```dockerfile
# Stage 1: build the ainm-server workspace member.
FROM rust:1.85-bookworm AS builder
WORKDIR /build

# Copy the full workspace (the .dockerignore trims target/, ios/, etc.).
# A workspace member depends on ../crates/*, so the whole workspace must be present —
# this is the fix for the old server/-context Dockerfile that could not resolve it.
COPY Cargo.toml Cargo.lock ./
COPY server/ server/
COPY crates/ crates/

RUN cargo build --release -p ainm-server

# Stage 2: minimal runtime.
FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl bubblewrap \
 && rm -rf /var/lib/apt/lists/*
# Non-root runtime user.
RUN useradd --system --uid 10001 --create-home ainm
WORKDIR /app
COPY --from=builder /build/target/release/ainm-server /usr/local/bin/ainm-server
COPY --from=builder /build/server/migrations /app/migrations
COPY --from=builder /build/server/config /app/config
USER ainm
EXPOSE 8080 8443
# /health/ready is DB-aware (deeper than the always-200 /health/live).
HEALTHCHECK --interval=30s --timeout=3s --start-period=15s \
  CMD curl -sf http://localhost:8080/health/ready || exit 1
ENTRYPOINT ["ainm-server"]
```

> If `cargo build -p ainm-server` needs `migrations/` at compile time (sqlx offline), it is copied with `server/`. The build does not use a dependency-stub cache layer (the old Dockerfile's `echo "fn main(){}"` trick); the spec marks the cache mechanism as plan-level — we choose the simpler full-copy build here for correctness over rebuild speed. If rebuild speed becomes a problem later, introduce `cargo-chef` as a follow-up.

- [ ] **Step 3: Delete the stale server-local Dockerfile**

```bash
rtk proxy git rm server/Dockerfile
```

- [ ] **Step 4: Build the image**

Run: `docker build -t ainm-server:hardening . 2>&1 | tail -25`
Expected: build succeeds through both stages; final line `naming to docker.io/library/ainm-server:hardening` (or `writing image`). If the build fails on a missing workspace file, verify `.dockerignore` did not exclude `crates/` or `server/`.

- [ ] **Step 5: Verify bubblewrap + non-root in the image**

Run:
```bash
docker run --rm --entrypoint sh ainm-server:hardening -c 'id -un && which bwrap'
```
Expected: prints `ainm` then `/usr/bin/bwrap`.

- [ ] **Step 6: Commit**

```bash
rtk proxy git add Dockerfile .dockerignore
rtk proxy git rm --cached server/Dockerfile 2>/dev/null || true
rtk proxy git commit -m "$(cat <<'EOF'
build(server): workspace-correct repo-root Dockerfile (bubblewrap, non-root)

Replaces the stale server/-context Dockerfile that could not resolve the
../crates/ainm-escrow workspace dependency. Installs bubblewrap so sandboxed
MCP servers run; runs as a non-root user; healthchecks /health/ready.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 9: Single-node prod compose + env template + federation-demo repoint

**Files:**
- Create: `docker-compose.prod.yml` (repo root)
- Create: `.env.production.example` (repo root)
- Modify: `.gitignore` (ignore `.env.production`)
- Modify: `server/docker-compose.yml` (repoint build context to repo root)

- [ ] **Step 1: Create `.env.production.example`**

```bash
# Copy to .env.production (gitignored) and fill in real values before deploying.

# REQUIRED — signs enrollment/recovery tokens. Must be >= 32 bytes and STABLE across
# restarts (a fresh value invalidates in-flight onboarding/recovery tokens).
#   generate: openssl rand -base64 48
AINM_TOKEN_SECRET=

# REQUIRED — base64; HKDF master secret for encrypting BYO LLM keys at rest.
#   generate: openssl rand -base64 32
AINM_LLM_ENCRYPTION_KEY=

# REQUIRED — server's own LLM key (hosted agent). Do NOT set AINM_LLM__MOCK in production.
AINM_LLM__API_KEY=
AINM_LLM__MODEL=gpt-4o-mini

# REQUIRED — transactional email (OTP delivery). Without these the server refuses to boot in prod.
AINM_EMAIL__RESEND_API_KEY=
AINM_EMAIL__FROM=noreply@yourdomain.example

# REQUIRED — Postgres connection (points at the compose `postgres` service by default).
AINM_DATABASE__URL=postgres://ainm:CHANGE_ME@postgres:5432/ainm

# OPTIONAL — graceful-shutdown drain budget in seconds (default 25). Keep below the
# compose stop_grace_period (30s) so Docker does not SIGKILL mid-drain.
AINM_SHUTDOWN_DRAIN_SECS=25
```

- [ ] **Step 2: Create `docker-compose.prod.yml` at the repo root**

```yaml
# Single-node production deployment: ainm-server + Postgres.
# Usage: cp .env.production.example .env.production  (fill it in)
#        docker compose -f docker-compose.prod.yml up --build -d
services:
  postgres:
    image: postgres:16-bookworm
    environment:
      POSTGRES_DB: ainm
      POSTGRES_USER: ainm
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:-CHANGE_ME}
    volumes:
      - ainm-pg-data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ainm"]
      interval: 5s
      timeout: 3s
      retries: 5
    restart: unless-stopped

  app:
    build:
      context: .
      dockerfile: Dockerfile
    depends_on:
      postgres:
        condition: service_healthy
    ports:
      - "8080:8080"
      - "8443:8443"
    env_file:
      - .env.production
    environment:
      AINM_ENV: production
      RUST_LOG: info
    volumes:
      - ./server/certs:/certs:ro
      - ainm-app-data:/data
    command:
      - "--mode"
      - "human"
      - "--port"
      - "8080"
      - "--federation-port"
      - "8443"
      - "--identity"
      - "${AINM_IDENTITY:-localhost}"
      - "--cert"
      - "/certs/localhost.pem"
      - "--key"
      - "/certs/localhost-key.pem"
      - "--ca-cert"
      - "/certs/ca.pem"
      - "--data-dir"
      - "/data"
    stop_grace_period: 30s
    restart: unless-stopped
    healthcheck:
      test: ["CMD-SHELL", "curl -sf http://localhost:8080/health/ready || exit 1"]
      interval: 30s
      timeout: 3s
      start_period: 15s
      retries: 3

volumes:
  ainm-pg-data:
  ainm-app-data:
```

> No `tty: true`/`stdin_open: true` — the server detects no TTY and skips the REPL (main.rs already handles this), serving until SIGTERM. `stop_grace_period: 30s` exceeds the 25s default drain so Docker does not SIGKILL mid-drain. Cert paths assume `server/certs/` holds `localhost.pem`/`localhost-key.pem`/`ca.pem` (as `scripts/dogfood.sh` uses); adjust the four cert args for real domain certs.

- [ ] **Step 3: Ignore the real env file**

Append to `.gitignore` (check it isn't already covered first: `grep -n 'env.production' .gitignore`):

```
.env.production
```

- [ ] **Step 4: Repoint the federation-demo compose build context**

In `server/docker-compose.yml`, the `server-a` and `server-b` services use `build: .`. Since the Dockerfile moved to the repo root, change EACH service's `build:` from:

```yaml
    build: .
```

to:

```yaml
    build:
      context: ..
      dockerfile: Dockerfile
```

(Leave everything else in that file unchanged — it remains the two-server federation demo.)

- [ ] **Step 5: Validate both compose files parse**

Run:
```bash
docker compose -f docker-compose.prod.yml config >/dev/null && echo "prod OK"
docker compose -f server/docker-compose.yml config >/dev/null && echo "federation OK"
```
Expected: `prod OK` and `federation OK`. (The prod one may warn that `.env.production` is missing — create an empty one for the parse check: `touch .env.production`, then remove it, or ignore the warning.)

- [ ] **Step 6: Commit**

```bash
rtk proxy git add docker-compose.prod.yml .env.production.example .gitignore server/docker-compose.yml
rtk proxy git commit -m "$(cat <<'EOF'
build(server): single-node prod compose + env template; repoint federation demo

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 10: Full suite + clippy

**Files:** none (verification)

- [ ] **Step 1: Run the full server suite**

Run: `cd server && cargo test 2>&1 | tail -30` (or `cargo nextest run` if that's the project default)
Expected: green except the two known pre-existing failures noted in prior milestones. If any NEW failure appears, fix it before proceeding (do not accept "close enough").

- [ ] **Step 2: Clippy**

Run: `cd server && cargo clippy --all-targets 2>&1 | tail -30`
Expected: no new warnings introduced by this work (the new modules are warning-clean).

- [ ] **Step 3: Commit any fixes** (only if Steps 1–2 required changes)

```bash
rtk proxy git add -A
rtk proxy git commit -m "$(cat <<'EOF'
chore(server): clippy/test cleanup for deployment hardening

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 11: Runbook

**Files:**
- Create: `docs/runbooks/deployment.md`

- [ ] **Step 1: Write the runbook**

Create `docs/runbooks/deployment.md`:

```markdown
# Single-Node Production Deployment Runbook

## Configure

1. `cp .env.production.example .env.production`
2. Fill in every value:
   - `AINM_TOKEN_SECRET` — `openssl rand -base64 48` (>= 32 bytes; keep STABLE across deploys —
     rotating it invalidates in-flight enrollment/recovery tokens).
   - `AINM_LLM_ENCRYPTION_KEY` — `openssl rand -base64 32`.
   - `AINM_LLM__API_KEY`, `AINM_LLM__MODEL` — the hosted agent's LLM.
   - `AINM_EMAIL__RESEND_API_KEY`, `AINM_EMAIL__FROM` — OTP delivery.
   - `AINM_DATABASE__URL` — Postgres (defaults to the compose `postgres` service).
3. Keep `.env.production` out of git (already in `.gitignore`).

The server **refuses to boot** in production (`AINM_ENV=production`) if any required value is
missing/unsafe — read the `[config]` lines on stderr and fix them. `AINM_LLM__MOCK` must NOT be set.

## Build & run

```bash
docker compose -f docker-compose.prod.yml up --build -d
docker compose -f docker-compose.prod.yml logs -f app
```

Health: `curl -sf http://<host>:8080/health/ready` returns 200 once Postgres is up and migrations ran.

## Stop / restart (graceful)

```bash
docker compose -f docker-compose.prod.yml stop      # or: restart
```

On SIGTERM the server stops accepting new connections and drains in-flight agent turns up to
`AINM_SHUTDOWN_DRAIN_SECS` (default 25s). `stop_grace_period` (30s) exceeds that so Docker does not
SIGKILL mid-drain. The logs show `received SIGTERM; beginning graceful shutdown` → `drained cleanly`.

## Sandbox caveat (bubblewrap)

The image installs `bubblewrap` so MCP servers configured with `sandbox = "bubblewrap"` are isolated.
bubblewrap needs unprivileged user namespaces; some container hosts require enabling host userns or
adding a capability. If sandboxed MCP servers fail to launch, that is the cause — enable userns on the
host or grant the needed capability. (Hardening-only MCP servers, `sandbox = "none"`, are unaffected.)

## Federation demo (separate, unchanged)

`server/docker-compose.yml` is the two-server federation demo, not a production deploy. Its build
context now points at the repo root (`context: ..`) so it builds against the same Dockerfile.
```

- [ ] **Step 2: Commit**

```bash
rtk proxy git add docs/runbooks/deployment.md
rtk proxy git commit -m "$(cat <<'EOF'
docs: single-node production deployment runbook

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 12: MANUAL — live container verification

**Files:** none (manual verification; requires Docker + real-ish secrets). Mark complete only after a human runs it.

- [ ] **Step 1: Boot the prod stack with real secrets**

```bash
cp .env.production.example .env.production
# Fill AINM_TOKEN_SECRET, AINM_LLM_ENCRYPTION_KEY (openssl rand), a real or test
# AINM_LLM__API_KEY, Resend keys, and set AINM_DATABASE__URL to the compose postgres.
docker compose -f docker-compose.prod.yml up --build -d
docker compose -f docker-compose.prod.yml logs app | grep -E 'config|listening|ready'
```
Expected: `[config] production configuration validated`; server reaches a serving state; **no** `random per-process secret` warning (because `AINM_TOKEN_SECRET` is set).

- [ ] **Step 2: Verify the fail-fast gate (negative test)**

```bash
docker compose -f docker-compose.prod.yml stop app
# Temporarily blank AINM_TOKEN_SECRET in .env.production, then:
docker compose -f docker-compose.prod.yml up app
```
Expected: app container exits non-zero, logs `[config] refusing to boot in production` + the bullet list. Restore `.env.production` afterward.

- [ ] **Step 3: Verify graceful drain under load**

With the stack up and healthy, complete one end-to-end onboarding + assistant turn from the iOS client (per the IOS-11 runbook), then while a turn is plausibly in flight:
```bash
docker compose -f docker-compose.prod.yml stop app   # sends SIGTERM, 30s grace
docker compose -f docker-compose.prod.yml logs app | tail -20
```
Expected: logs show `received SIGTERM; beginning graceful shutdown` → `drained cleanly` (or `drain deadline exceeded` only if a turn genuinely overran 25s), and the container stopped within the grace period (no SIGKILL).

- [ ] **Step 4: Verify health endpoint**

```bash
curl -sf http://localhost:8080/health/ready && echo " ready"
```
Expected: `ready`.

- [ ] **Step 5: Tear down**

```bash
docker compose -f docker-compose.prod.yml down
```

- [ ] **Step 6: Record the result**

Append a short note to `docs/runbooks/deployment.md` (or update memory `project_ios11_live_validated.md`) recording the live verification date and outcome.

---

## DoD Checklist (verify before declaring the sub-project done)

- [ ] SIGTERM **and** SIGINT trigger a bounded graceful drain; in-flight agent turns finish within `AINM_SHUTDOWN_DRAIN_SECS` (default 25), then clean exit; stragglers bounded by the deadline. (Tasks 1–5, 12)
- [ ] `AINM_ENV=production` refuses to boot (exit 1, all problems listed) on missing/invalid gated secrets; non-production unchanged. (Tasks 6–7, 12)
- [ ] `docker build` from repo root produces a working image (workspace builds, bubblewrap present, non-root, `/health/ready` healthcheck). (Tasks 8, 12)
- [ ] `docker-compose.prod.yml` brings up app + Postgres; app passes healthcheck; `stop` drains within grace. (Tasks 9, 12)
- [ ] lifecycle + prod_guard covered by tests; full server suite green (minus the two known failures); `make dogfood`/`make uat` unaffected. (Tasks 5, 6, 10)
- [ ] Runbook documents required env vars, the bubblewrap userns caveat, and build/run/stop. (Task 11)
```
