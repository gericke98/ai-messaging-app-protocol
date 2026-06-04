# Deployment / Single-Node Production Hardening — Design (v1)

**Date:** 2026-06-04
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)
**Context:** The AINM hosted runtime server (`ainm-server`, Rust/Axum, redb + Postgres) has never been
made safely deployable. Three concrete gaps block any real production run:

1. **No graceful shutdown.** `server/src/main.rs` only awaits `tokio::signal::ctrl_c()` (SIGINT).
   Docker and most orchestrators send **SIGTERM**, which has no handler, so the process is killed
   immediately. Teardown is `handle.abort()` on the server task — in-flight WebSocket connections and
   agent turns (including a live LLM call whose signed reply hasn't been delivered) are dropped.
2. **No production secrets gate.** Both `AINM_TOKEN_SECRET` (→ random per-process secret, a `warn!`,
   boots anyway: enrollment/recovery tokens silently break on every restart) and
   `AINM_LLM_ENCRYPTION_KEY` (→ BYO LLM keys silently disabled) fall back. Nothing refuses to boot on
   a misconfigured production deploy.
3. **Stale, broken container.** `server/Dockerfile` + `server/docker-compose.yml` predate the Cargo
   workspace refactor: the repo is a workspace (root `Cargo.toml` + root `Cargo.lock`, members
   `server`, `crates/ainm-escrow`, `crates/ainm-escrow-ffi`), but the Dockerfile builds from context
   `server/` copying only `server/Cargo.toml` + `src/`, so it cannot resolve the
   `../crates/ainm-escrow` path dependency — **the image does not build**. It also installs no
   `bubblewrap` in the runtime image (so sub-project D's sandboxed MCP servers can't run) and is
   shaped as a two-server federation demo, not a single-node production deploy.

## 1. Goal & scope

Make **one** `ainm-server` node safely deployable and restart-safe on a generic single-node Docker
host: graceful SIGTERM drain of in-flight work, a fail-fast production secrets/config gate, and a
reproducible production container image + single-node compose.

**Decisions (locked in brainstorming):**
- **Scope = single-node production hardening.** True multi-replica HA is explicitly deferred: the
  server's redb store takes a single-process lock (only one instance may run at a time), so multiple
  replicas behind a load balancer are impossible without a storage re-architecture. That is a separate
  future sub-project.
- **Deploy target = generic single-node Docker:** a clean, reproducible image + a single-server
  `docker-compose.prod.yml` (app + Postgres), secrets via env file. No PaaS/k8s lock-in.
- **Secrets-gate trigger = `AINM_ENV=production`.** The gate enforces only when that env var is set
  (in the production compose/env file). Unset/dev/UAT keeps today's friendly fallbacks, so
  `make dogfood` and `make uat` are undisturbed.
- **Drain policy = bounded drain, finish in-flight turns.** On signal, stop accepting new
  connections/turns; let in-flight agent turns + queued WS sends complete up to a configurable
  deadline (default 25s), then force-close. Preserves "a turn already underway still delivers its
  signed reply." Compose `stop_grace_period` is set to exceed the deadline.

**Out of scope:** multi-replica HA / redb replacement; TLS termination & reverse-proxy choice;
secret-rotation tooling; k8s manifests; PaaS (fly.io/Render/Railway) configs.

## 2. Architecture overview

Three independent slices, two small new modules, no big-bang rewrite of the 1300-line `main.rs`:

```
server/src/lifecycle.rs    (new)   ShutdownController + signal install + drain sequence
server/src/prod_guard.rs   (new)   AINM_ENV production gate: validate-or-die before binding
server/src/main.rs         (edit)  call prod_guard at top of run_federation; build the controller,
                                   install signal handler, run the drain sequence on exit
server/src/server.rs       (edit)  thread the shutdown signal into the 4 axum::serve calls and into
                                   the WebSocket handler loop
Dockerfile                 (new, repo root)   workspace-correct multi-stage build (replaces server/Dockerfile)
.dockerignore              (new, repo root)   exclude target/, ios/, .git, node_modules
docker-compose.prod.yml    (new, repo root)   single-node app + Postgres
.env.production.example    (new, repo root)   documented required vars (real .env.production gitignored)
server/docker-compose.yml  (edit)             federation demo: repoint build context to repo root
```

**New dependency:** `tokio-util` (for `task::TaskTracker`), used by `ShutdownController` to track and
await in-flight turns. (Alternative considered: hand-rolled `AtomicUsize` + `Notify`; rejected as more
error-prone for the same behavior.)

Each slice is independently testable and shippable. Build order: lifecycle → prod_guard → container
(container depends on neither being complete, but is verified last so the live smoke run exercises
both).

## 3. Slice 1 — Graceful shutdown (`lifecycle`)

### 3.1 `ShutdownController`

`server/src/lifecycle.rs`, a `Clone`-able handle wrapping `Arc` internals:

- `signal: tokio::sync::watch::Sender<bool>` — flips `false → true` exactly once on shutdown.
- `tracker: tokio_util::task::TaskTracker` — counts in-flight agent turns.

API:
- `ShutdownController::new() -> Self`
- `subscribe(&self) -> watch::Receiver<bool>` — for serve calls and WS handlers to await.
- `trigger(&self)` — set the watch to `true` (idempotent).
- `turn_guard(&self) -> TurnGuard` — RAII guard; the WS handler holds one for the duration of each
  agent turn so the drain knows a turn is in flight.
- `is_shutting_down(&self) -> bool`
- `async wait_idle(&self)` — closes the tracker and awaits all outstanding `TurnGuard`s dropping.

`TurnGuard` holds a `tokio_util::task::TaskTracker` token (via `tracker.token()` / spawned-tracked
future) so that `wait_idle()` returns only once every guard has dropped.

> **Implementation note for the planner:** `TaskTracker` is designed around `spawn`/tracked futures.
> Agent turns are *not* spawned — they run inline in the WS handler. Use `TaskTracker::token()`
> (a `TaskTrackerToken` held by `TurnGuard` and dropped at end of turn) so `tracker.close()` +
> `tracker.wait()` resolves when all tokens are dropped, without spawning. If the installed
> `tokio-util` version lacks `token()`, fall back to an `Arc<AtomicUsize>` + `tokio::sync::Notify`
> in `ShutdownController` with the same `turn_guard()`/`wait_idle()` surface.

### 3.2 Signal installation

`pub fn install_signals(controller: ShutdownController)` spawns a task that selects on both:
- SIGTERM via `tokio::signal::unix::signal(SignalKind::terminate())`
- SIGINT via `tokio::signal::ctrl_c()`

On either, log `"[lifecycle] received <signal>; beginning graceful shutdown"` and call
`controller.trigger()`. (On non-Unix targets, fall back to `ctrl_c()` only — the server's production
target is Linux, so this is acceptable.)

### 3.3 Wiring into the listeners

The four `axum::serve(listener, app).await.ok()` sites in `server.rs` (`start`, `start_with_port`,
`start_federation`, and the `0.0.0.0` production listener) each gain
`.with_graceful_shutdown(async move { let mut rx = controller.subscribe(); let _ = rx.wait_for(|v| *v).await; })`
so the listener stops accepting **new** connections once shutdown is triggered. The controller is
threaded into these functions as a parameter. UAT/test start paths (`start`, `start_with_port`) may
pass a fresh detached controller so their existing Ctrl-C/abort behavior is preserved.

### 3.4 WS handler drain

The WebSocket connection handler loop (server.rs, around the per-connection `select!`) gains an arm
on `shutdown_rx.changed()` (subscribed from the controller). On shutdown:
- stop reading new client requests,
- allow the current in-flight turn (if any) to finish — its `TurnGuard` keeps it counted,
- send a WebSocket `Close` frame to the client,
- return from the handler.

Each agent turn dispatched within the handler is bracketed by `let _g = controller.turn_guard();` so
a turn already underway when the signal arrives runs to completion (and delivers its signed reply)
within the drain budget.

### 3.5 Drain sequence (in `main.rs`)

Replaces the current `fed_handle.abort()` teardown in the headless `human`/`agent` arms:

1. Await the controller's watch turning `true` (or the existing server-task-ended branch).
2. Listeners have already stopped accepting new connections (`with_graceful_shutdown`).
3. `tokio::time::timeout(drain_deadline, controller.wait_idle()).await` — wait for in-flight turns to
   drain, bounded.
4. On timeout: log `"[lifecycle] drain deadline exceeded; forcing shutdown"` and `abort()` the
   straggler tasks.
5. Flush/close the Postgres pool and the redb persistence layer, then return `Ok(())`.

`drain_deadline = Duration::from_secs(env AINM_SHUTDOWN_DRAIN_SECS, default 25)`.

## 4. Slice 2 — Production secrets gate (`prod_guard`)

`server/src/prod_guard.rs`:

- `pub fn is_production() -> bool` — `std::env::var("AINM_ENV").as_deref() == Ok("production")`.
- `pub fn validate() -> Result<(), Vec<String>>` — collects **all** problems (not first-fail) so one
  boot attempt reports everything to fix. Each entry is a human-actionable string.

Called at the very top of `run_federation` (after env is readable, before any listener binds). When
`is_production()` and `validate()` returns `Err(problems)`: print each problem to stderr prefixed
`[config] `, then `std::process::exit(1)`. When not production: skip entirely — today's fallbacks
stand.

**Gated in production:**

| Requirement | Rule | Why |
|---|---|---|
| `AINM_TOKEN_SECRET` | set, ≥ 32 bytes | random per-process fallback invalidates enrollment/recovery tokens on every restart |
| `AINM_LLM_ENCRYPTION_KEY` | set, valid base64, decodes to the length `CredCipher::new` requires | unset silently disables BYO LLM keys |
| `AINM_DATABASE__URL` | set | production must not run the no-DB/in-memory path |
| `AINM_LLM__MOCK` | must **not** be set/truthy | production must not serve constant-mock agent replies |
| Email sender | `AINM_EMAIL__RESEND_API_KEY` **and** `AINM_EMAIL__FROM` both set | otherwise the mock sender only logs OTPs — real signups can't receive codes |

The two named must-haves are `AINM_TOKEN_SECRET` and `AINM_LLM_ENCRYPTION_KEY`; the remaining three
were added in design with the user's agreement to make a production boot actually serviceable. (If
later trimmed, drop the corresponding `validate()` checks and their unit-test cases.)

`validate()` must read the encryption-key length requirement from the same place `CredCipher::new`
enforces it — to avoid logging plaintext, it validates length only and never echoes key material.

## 5. Slice 3 — Container & compose

### 5.1 Dockerfile (repo root, build context = repo root)

This relocation is the fix for the broken workspace build.

```dockerfile
# Stage 1: build
FROM rust:1.85-bookworm AS builder
WORKDIR /build
# Workspace manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY server/Cargo.toml server/Cargo.toml
COPY crates/ainm-escrow/Cargo.toml crates/ainm-escrow/Cargo.toml
COPY crates/ainm-escrow-ffi/Cargo.toml crates/ainm-escrow-ffi/Cargo.toml
# (dependency-cache layer: build a stub, then real sources — see plan for exact mechanism)
COPY server/ server/
COPY crates/ crates/
RUN cargo build --release -p ainm-server

# Stage 2: runtime
FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl bubblewrap \
 && rm -rf /var/lib/apt/lists/*
RUN useradd --system --uid 10001 --create-home ainm
WORKDIR /app
COPY --from=builder /build/target/release/ainm-server /usr/local/bin/ainm-server
COPY --from=builder /build/server/migrations /app/migrations
COPY --from=builder /build/server/config /app/config
USER ainm
EXPOSE 8080 8443
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s \
  CMD curl -sf http://localhost:8080/health/ready || exit 1
ENTRYPOINT ["ainm-server"]
```

Notes: `bubblewrap` is installed so sub-project D's `sandbox = "bubblewrap"` MCP servers run.
Healthcheck uses `/health/ready` (DB-aware) rather than the always-200 `/health/live`. Runs as a
non-root system user. `config/` is copied so `config/default.toml` resolves relative to `WORKDIR`.

**Exact dependency-cache mechanism** (cargo-chef vs. manual stub layer) is a plan-level decision; the
manifest-first ordering above is the intent.

### 5.2 `.dockerignore` (repo root)

Excludes `target/`, `ios/`, `**/node_modules`, `.git`, `website/`, `packages/*/dist`, `tmp/`, and
other non-build paths to keep the build context small and fast.

### 5.3 `docker-compose.prod.yml` (repo root)

One app service + Postgres 16:

- `postgres`: `postgres:16-bookworm`, named volume, `pg_isready` healthcheck.
- `app`: `build: { context: ., dockerfile: Dockerfile }`, `depends_on: postgres healthy`,
  `restart: unless-stopped`, `stop_grace_period: 30s` (exceeds the 25s drain deadline so Docker does
  not SIGKILL mid-drain), `env_file: .env.production`, plus `AINM_ENV=production` and
  `AINM_DATABASE__URL` pointing at the `postgres` service. Command runs `--mode human` headless
  (no tty → REPL is skipped, already handled in `main.rs`).
- Healthcheck identical to the Dockerfile's.

### 5.4 `.env.production.example` (repo root, committed)

Template documenting every required var with placeholder values and a one-line comment each:
`AINM_TOKEN_SECRET`, `AINM_LLM_ENCRYPTION_KEY`, `AINM_LLM__API_KEY`, `AINM_LLM__MODEL`,
`AINM_EMAIL__RESEND_API_KEY`, `AINM_EMAIL__FROM`, `AINM_DATABASE__URL`, `AINM_SHUTDOWN_DRAIN_SECS`.
The real `.env.production` is added to `.gitignore`.

### 5.5 Federation demo compose

`server/docker-compose.yml` (the two-server federation demo) gets its `build:` for each service
repointed to the repo-root context + Dockerfile so it keeps building against the fixed image. No
other changes — it stays a demo artifact.

### 5.6 Runbook caveat (documented, not papered over)

`bubblewrap` needs unprivileged user namespaces. On some container hosts this requires
`--cap-add` or host userns enabled. The runbook documents this; sandboxed MCP servers degrade per
sub-project D's existing behavior when unavailable.

## 6. Testing

**`lifecycle` (unit):**
1. A held `TurnGuard` keeps `wait_idle()` pending; after it drops, `wait_idle()` resolves.
2. Drain-timeout path: with a guard never dropped, `timeout(deadline, wait_idle())` elapses and the
   force-close branch is taken.
3. `trigger()` flips the watch and is idempotent; `subscribe()` receivers observe `true`.

**`lifecycle` (integration):** boot a server with a controller, open a WS client, begin a turn, call
`trigger()`, assert the client receives a `Close` frame and the server's drain returns within the
deadline.

**`prod_guard` (unit):**
1. `AINM_ENV=production` + all required vars valid → `Ok`.
2. Each required var missing/invalid in isolation → `Err` containing that var's name (aggregated:
   removing two → both named).
3. `AINM_LLM__MOCK=1` in production → `Err`.
4. Not production → `Ok` regardless of which vars are set.

(Env-var tests must serialize / use isolated env to avoid cross-test interference — use a guard that
sets/restores vars, or run them `#[serial]`.)

**Container (manual + smoke):** `docker build` from repo root succeeds; the image contains
`bubblewrap`; the process runs as non-root; `GET /health/ready` returns 200 once Postgres is up. The
full live `docker-compose.prod.yml` run is a **manual** verification step (consistent with prior
live-validation phases — onboard a user end-to-end, then `docker compose stop` and confirm a clean
bounded drain in the logs).

**Regression:** full server suite green except the two known pre-existing failures; `make dogfood`
and `make uat` still boot (proving the gate did not touch the non-production path).

## 7. Definition of Done

1. SIGTERM **and** SIGINT trigger a graceful drain; in-flight agent turns finish (delivering their
   signed reply) within `AINM_SHUTDOWN_DRAIN_SECS` (default 25), then the process exits cleanly;
   stragglers are force-closed past the deadline.
2. With `AINM_ENV=production`, the server refuses to boot (exit 1, all problems listed) when any
   gated secret/config is missing or invalid; without `AINM_ENV=production`, behavior is unchanged.
3. `docker build` from the repo root produces a working image (workspace builds, `bubblewrap`
   present, non-root, `/health/ready` healthcheck); `docker-compose.prod.yml` brings up app +
   Postgres and the app passes its healthcheck.
4. `docker compose stop` drains within the grace period with a clean shutdown logged (no aborted
   in-flight turn when one was underway and within budget).
5. lifecycle + prod_guard covered by tests; full server suite green (minus the two known failures);
   `make dogfood`/`make uat` unaffected.
6. A runbook documents: required production env vars, the bubblewrap userns caveat, and the
   build/run/stop flow.

## 8. Build order

Slice 1 (lifecycle) → Slice 2 (prod_guard) → Slice 3 (container). 1 and 2 are independent and could
swap; the container is verified last so its manual live run exercises both the gate (boots with real
secrets) and the drain (`docker compose stop`). The `tokio-util` token-vs-fallback detail (§3.1) is
the riskiest implementation choice — resolve it first in Slice 1's plan.

## 9. Runbook (deploy / configure / stop)

- **Configure:** copy `.env.production.example` → `.env.production`; fill `AINM_TOKEN_SECRET`
  (≥32 random bytes), `AINM_LLM_ENCRYPTION_KEY` (base64), `AINM_LLM__API_KEY`, Resend email vars,
  and `AINM_DATABASE__URL`. Keep `.env.production` out of git.
- **Build & run:** `docker compose -f docker-compose.prod.yml up --build -d`. The app refuses to boot
  if any production secret is missing — read the `[config]` lines on stderr and fix them.
- **Health:** `curl -sf http://<host>:8080/health/ready` returns 200 when ready.
- **Stop / restart:** `docker compose -f docker-compose.prod.yml stop` (or `restart`). The server
  drains in-flight turns up to `AINM_SHUTDOWN_DRAIN_SECS` before exiting; `stop_grace_period` (30s)
  exceeds that so Docker does not SIGKILL mid-drain.
- **Sandbox caveat:** if bubblewrap-sandboxed MCP servers fail to launch, the host likely lacks
  unprivileged user namespaces — enable host userns or add the required capability.
