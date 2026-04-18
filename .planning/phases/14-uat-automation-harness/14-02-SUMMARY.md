---
phase: 14-uat-automation-harness
plan: 02
subsystem: uat-harness
tags: [rust, clap, axum, makefile, ci, github-actions, docs]
dependency-graph:
  requires:
    - "demo/src/main.rs existing clap Cli struct"
    - "demo/src/server.rs existing legacy start() and federation start_federation()"
    - ".github/workflows/ios-ci.yml destination-string conventions"
  provides:
    - "ainm-demo --uat flag (run_uat entry point)"
    - "server::start_with_port() on the legacy non-federation path with /health + /health/live"
    - "Makefile uat + uat-debug + uat-clean targets"
    - ".github/workflows/ios-uat.yml (workflow_dispatch-only)"
    - "docs/testing.md UAT invocation documentation"
  affects:
    - "demo/Cargo.toml (added tempfile = 3 dependency)"
    - ".gitignore (added tmp/ to exclude runtime output)"
tech-stack:
  added:
    - "tempfile 3 (Rust crate, ephemeral tempdir for UAT state isolation)"
  patterns:
    - "Binary-name check: ainm-demo is the correct binary (CONTEXT.md D-05 said ainm-server; RESEARCH.md corrected this; every artifact uses ainm-demo)"
    - "Plain /health on legacy path, /health/live retained on federation path; start_with_port registers both for symmetry"
    - "Makefile trap EXIT INT TERM guarantees teardown across all exit shapes (normal, Ctrl-C, SIGTERM)"
    - "Workflow_dispatch-only CI until deferred push/PR matrix activation (D-08)"
key-files:
  created:
    - "Makefile"
    - ".github/workflows/ios-uat.yml"
    - "docs/testing.md"
  modified:
    - "demo/Cargo.toml"
    - "demo/Cargo.lock"
    - "demo/src/main.rs"
    - "demo/src/server.rs"
    - ".gitignore"
decisions:
  - "Added plain /health route (not just /health/live) on start_with_port so the Makefile poll URL (http://localhost:8080/health) works without reaching into the federation-style path"
  - "Registered BOTH /health and /health/live in start_with_port — symmetry with federation mode lets future UAT debuggers reuse whichever URL they already have in muscle memory"
  - "Used tempfile = 3 as a full dependency (not dev-dep) since run_uat is compiled into the release binary; tempdir lifecycle is tied to the tokio::signal::ctrl_c await so cleanup is guaranteed"
  - "Makefile uses (cd demo && cargo run --quiet --bin ainm-demo -- --uat > ../tmp/uat-server.log 2>&1 &) to keep paths repo-root-relative — same tmp/uat-server.log the Rust side writes to"
  - "Deterministic identities provisioned: uat-user@localhost + helper@localhost (matches D-04 fixture expectations)"
metrics:
  duration: "6 minutes"
  completed: "2026-04-18"
  tasks: 3
  files_touched: 8
---

# Phase 14 Plan 02: UAT Server Boot + Make Orchestrator Summary

One-liner: ainm-demo --uat boots an ephemeral Axum server on 127.0.0.1:8080 with /health, Makefile orchestrates xcodebuild test with trap-guaranteed teardown, workflow_dispatch CI placeholder and docs published.

## Overview

This plan delivered the Rust + CI + docs half of the Phase 14 UAT automation harness. After this plan, `make uat` (when invoked against the iOS UITests scheme from the parallel 14-01 plan) will:

1. Verify port 8080 is free (fails fast via `lsof`).
2. Boot `cargo run --bin ainm-demo -- --uat` in the background, logging to `tmp/uat-server.log`.
3. Poll `http://localhost:8080/health` for up to 30s until 200 OK.
4. Invoke `xcodebuild test -scheme AINMMessengerUITests` against the iPhone 16 / iOS 18.0 simulator.
5. Tear the server down on exit, Ctrl-C, or SIGTERM via `trap EXIT INT TERM`.

The server-side boot and health poll were proven live during Task 1 verification (curl returned HTTP 200 against `/health` and `/health/live`). The full end-to-end xcodebuild step depends on the AINMMessengerUITests scheme being created by the parallel 14-01 plan.

## Tasks Executed

### Task 1: --uat flag + run_uat() + start_with_port() — commit 1ee2e5a

Added `uat: bool` field to the clap `Cli` struct in `demo/src/main.rs`. Extended `main()` to short-circuit to `run_uat(cli)` whenever `--uat` is set, independent of any other flags. Implemented `run_uat`:

- Resolves `tmp/uat-server.log` relative to `env!("CARGO_MANIFEST_DIR").parent()` so both `cd demo && cargo run` and `cargo run --manifest-path demo/Cargo.toml` invocation shapes write to the same repo-root-relative file.
- Creates a fresh `tempfile::tempdir()` for ephemeral storage (guard dropped at process exit).
- Provisions deterministic identities `uat-user@localhost` (human) and `helper@localhost` (agent) matching D-04 fixture contract.
- Calls new `server::start_with_port(identity_store, agent_card, 8080)`.
- Blocks on `tokio::signal::ctrl_c()` for clean Makefile teardown.

Added `build_uat_agent_card()` helper returning a minimal agent card with streaming/memory/sub-thread capabilities flagged true (sufficient for AINMClient handshake + Phase 13 scenario fixtures).

Added `pub async fn start_with_port(identity_store, agent_card, port) -> Result<(SocketAddr, JoinHandle<()>)>` in `demo/src/server.rs`:

```rust
pub async fn start_with_port(
    identity_store: IdentityStore,
    agent_card: serde_json::Value,
    port: u16,
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)>
```

Signature mirrors the legacy `start()` plus a `port: u16` parameter. Routes registered: `/ws`, `/.well-known/ainm/{agent_name}/agent-card.json`, `/health` (returns `StatusCode::OK`), `/health/live` (same). Binds `127.0.0.1:{port}` instead of the legacy `127.0.0.1:0`. `start()` itself is unchanged — all legacy callers continue to bind a random port.

Added `tempfile = "3"` to `demo/Cargo.toml` under `[dependencies]`.

Live verification: `cargo run --bin ainm-demo -- --uat` on port 8080 responded `HTTP 200` to both `GET /health` and `GET /health/live`; `tmp/uat-server.log` captured `[uat] ephemeral db dir:` and `[uat] server ready on ws://127.0.0.1:8080/ws, GET /health returns 200`; server terminated cleanly on SIGTERM.

### Task 2: Makefile with uat + uat-debug + uat-clean targets — commit 02faa0d

Created `Makefile` at repo root with three phony targets:

- `uat`: preflight `lsof -nP -iTCP:8080 -sTCP:LISTEN` check; `mkdir -p tmp`; backgrounds `(cd demo && cargo run --quiet --bin ainm-demo -- --uat > ../tmp/uat-server.log 2>&1 &)` with PID captured to `tmp/uat-server.pid`; `trap 'kill $(cat tmp/uat-server.pid) ...' EXIT INT TERM`; loops `curl -sf -o /dev/null http://localhost:8080/health` for 30s with 1s sleep; on ready, `cd ios/AINMMessenger && xcodebuild test -scheme AINMMessengerUITests -destination 'platform=iOS Simulator,name=iPhone 16,OS=18.0' CODE_SIGNING_ALLOWED=NO -resultBundlePath ../../tmp/UAT-Results`.
- `uat-debug`: boots `cargo run --bin ainm-demo -- --uat` in foreground (no tests, no teardown — Ctrl-C to stop). Useful for manual browser/curl inspection or attaching the iOS app without the test runner.
- `uat-clean`: `rm -rf tmp/`.

All recipe lines are TAB-indented. Destination string (`iPhone 16 / OS=18.0`) and `CODE_SIGNING_ALLOWED=NO` flag mirror `.github/workflows/ios-ci.yml` exactly.

Smoke-tested the server-boot + health-poll + teardown portion independently (the xcodebuild step depends on the AINMMessengerUITests scheme being created in parallel 14-01): server reached ready state after 3 seconds and terminated cleanly on `kill`.

`make -n uat` dry-run prints the expected recipe containing `cargo run --quiet --bin ainm-demo -- --uat`, `trap`, `EXIT INT TERM`, `xcodebuild test`, and `CODE_SIGNING_ALLOWED=NO`.

### Task 3: ios-uat.yml + docs/testing.md — commit eaae41b

Created `.github/workflows/ios-uat.yml`:

- `on: workflow_dispatch:` only (no push/pull_request triggers — D-08).
- `runs-on: macos-14`, `timeout-minutes: 30`.
- Steps: `actions/checkout@v4` → `sudo xcode-select -s /Applications/Xcode_16.0.app/Contents/Developer` → `actions/cache@v4` for `~/.cargo/registry` + `demo/target` keyed on `hashFiles('demo/Cargo.lock')` → `xcodebuild -resolvePackageDependencies` for the main scheme → `make uat` → conditional `actions/upload-artifact@v4` steps for `tmp/UAT-Results.xcresult` and `tmp/uat-server.log` on failure.
- Existing `.github/workflows/ios-ci.yml` left untouched.

Created `docs/testing.md` documenting:

- Local invocation: `make uat` and what the five-step orchestration does.
- Ancillary targets: `make uat-debug`, `make uat-clean`.
- Server behavior table: port, storage, log level, log path, identities.
- CI integration note: workflow_dispatch-only until deferred matrix activation.
- Known dependencies: Xcode 16+, Rust 1.85+, curl, GNU-compatible Make, free port 8080.

## Deviations from Plan

None. Plan executed exactly as written.

The phase directory (`.planning/phases/14-uat-automation-harness/`) was copied from the main repository into the worktree at the start of execution — the worktree only had phases 07-13 prior. This is a worktree-setup detail, not a deviation from the plan's intent.

## Plan-Specific Output Requirements

Per `<output>` in 14-02-PLAN.md:

- **Exact signature of the new `start_with_port` in server.rs (for 14-06 debugging):**
  ```rust
  pub async fn start_with_port(
      identity_store: IdentityStore,
      agent_card: serde_json::Value,
      port: u16,
  ) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)>
  ```
  Routes in the Router chain: `/ws`, `/.well-known/ainm/{agent_name}/agent-card.json`, `/health`, `/health/live`. Binds `127.0.0.1:{port}`.

- **Makefile template deviation for GNU Make vs BSD Make on macOS:** None required. The macOS-shipped BSD Make accepts the syntax as-written (TAB-indented recipes, `$$` for runtime shell variables, `$(VAR)` for Make variables). `make -n uat` dry-run emitted the expected recipe on the first attempt.

- **iOS simulator destination string that worked on the executor's machine:** Not independently re-verified on this executor — `make uat`'s xcodebuild step depends on the AINMMessengerUITests scheme being created by the parallel 14-01 plan. The destination string `'platform=iOS Simulator,name=iPhone 16,OS=18.0'` was copied verbatim from the existing `ios-ci.yml` for consistency.

- **`tempfile` dependency status:** Added as a new entry to `[dependencies]` in `demo/Cargo.toml`. The existing demo crate did not use any tempdir helper; run_uat is the first consumer. Chose `tempfile = "3"` per the crate's current major version and the Rust community convention.

## Authentication Gates

None. All work was local, no credentials or external services required.

## Known Stubs

None. The Makefile's `make uat` target depends on the AINMMessengerUITests scheme from the parallel 14-01 plan, but this is an explicit cross-plan dependency (not a stub). The xcodebuild step will exit non-zero with "no tests found" until 14-06 lands actual test bodies — this is documented in the plan as an expected transient state, not a stub.

## Commits

| Task  | Hash     | Message                                                                 |
| ----- | -------- | ----------------------------------------------------------------------- |
| 1     | 1ee2e5a  | feat(14-02): add --uat flag with /health endpoint for automation harness |
| 2     | 02faa0d  | feat(14-02): add Makefile with uat + uat-debug + uat-clean targets       |
| 3     | eaae41b  | feat(14-02): add placeholder ios-uat CI workflow and docs/testing.md     |

## Files Changed

**Created:**
- `Makefile` — repo-root orchestrator with uat/uat-debug/uat-clean targets
- `.github/workflows/ios-uat.yml` — workflow_dispatch-only CI placeholder
- `docs/testing.md` — UAT invocation documentation

**Modified:**
- `demo/Cargo.toml` — added `tempfile = "3"` under `[dependencies]`
- `demo/Cargo.lock` — resolved tempfile + transitive deps
- `demo/src/main.rs` — added `uat: bool` Cli field, `run_uat` async fn, `build_uat_agent_card` helper, early dispatch from `main()`
- `demo/src/server.rs` — added `pub async fn start_with_port` with `/health` + `/health/live` routes; `start()` unchanged
- `.gitignore` — added `tmp/` entry to exclude server log + PID + result bundle runtime output

## Verification Results

All automated verification from the plan passed:

- `grep -q 'uat: bool' demo/src/main.rs` — OK
- `grep -q 'fn run_uat' demo/src/main.rs` — OK
- `grep -q 'start_with_port' demo/src/main.rs` — OK
- `grep -q 'pub async fn start_with_port' demo/src/server.rs` — OK
- `grep -q '"/health"' demo/src/server.rs` — OK
- `cargo check --manifest-path demo/Cargo.toml` — OK (no warnings introduced)
- `cargo build --bin ainm-demo` — OK (Finished)
- `cargo run --bin ainm-demo -- --uat` + `curl http://localhost:8080/health` — HTTP 200 OK
- `tmp/uat-server.log` written with `[uat] server ready` banner
- `grep -q 'uat:' Makefile` + all sub-string checks — OK
- `make -n uat` dry-run contains `cargo run --quiet --bin ainm-demo -- --uat` — OK
- `.github/workflows/ios-uat.yml` has `workflow_dispatch:` and no `push:`/`pull_request:` — OK
- `docs/testing.md` contains all required substrings (`make uat`, `AINMMessengerUITests`, `tmp/uat-server.log`, `ainm-demo`, `uat-debug`, `uat-clean`, `port 8080`) — OK
- No `ainm-server` references in any touched file — OK

## Self-Check: PASSED

All claimed files present on disk, all three task commits present in `git log`, SUMMARY.md itself present.
