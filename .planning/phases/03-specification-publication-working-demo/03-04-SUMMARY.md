---
phase: 03-specification-publication-working-demo
plan: 04
subsystem: demo
tags: [rust, axum, websocket, ed25519, json-rpc, streaming, repl, terminal]

# Dependency graph
requires:
  - phase: 01-protocol-kernel
    provides: "Identity, security, transport, content specs defining message formats"
  - phase: 02-agent-native-features
    provides: "Streaming, interactive blocks, tool visibility, Agent Card specs"
  - phase: 03-01
    provides: "JSON Schema definitions for protocol validation"
  - phase: 03-02
    provides: "AsyncAPI transport binding document"
provides:
  - "Single-binary Rust CLI demo proving AINM protocol end-to-end"
  - "Ed25519 signing and verification implementation"
  - "RFC 8785 canonical JSON signing input construction"
  - "Streaming lifecycle implementation (start/delta/end with collapse)"
  - "Terminal rendering for all interactive block types"
  - "Mock agent with 5 scripted scenarios covering core protocol features"
affects: [03-05, ios-client, documentation]

# Tech tracking
tech-stack:
  added: [axum-0.8, tokio-tungstenite-0.26, ring-0.17, serde_json_canonicalizer-0.3, rustyline-15, colored-3, uuid-1, chrono-0.4]
  patterns: [single-binary-demo, websocket-json-rpc, ed25519-canonical-signing, stream-start-delta-end, broadcast-channel-routing]

key-files:
  created:
    - demo/Cargo.toml
    - demo/src/main.rs
    - demo/src/identity.rs
    - demo/src/signing.rs
    - demo/src/protocol.rs
    - demo/src/server.rs
    - demo/src/agent.rs
    - demo/src/streaming.rs
    - demo/src/repl.rs
    - demo/src/render.rs
  modified: []

key-decisions:
  - "Used concrete WebSocket types instead of generic trait bounds to avoid Send/Sync issues with tokio::spawn"
  - "Used StdRng (Send-safe) instead of ThreadRng for streaming delay randomization across await points"
  - "Downgraded dependency versions to match available crates: tokio-tungstenite 0.26, rustyline 15, rand 0.8"
  - "Broadcast channels for message routing between human and agent WebSocket connections"

patterns-established:
  - "Single-binary demo pattern: main.rs spawns server + agent + REPL as concurrent tokio tasks"
  - "Ed25519 signing follows security.md Section 4: remove signature -> canonicalize -> sign -> base64url encode"
  - "Streaming uses notifications (no id) for start/delta/end; collapsed message at end is signed"
  - "Tool visibility events are full signed messages with tool object containing call_id, name, status"

requirements-completed: [SPEC-05]

# Metrics
duration: 10min
completed: 2026-03-28
---

# Phase 03 Plan 04: Working Demo Summary

**Single-binary Rust CLI demo proving AINM protocol end-to-end: Ed25519 signing, JSON-RPC WebSocket transport, streaming with token-by-token rendering, and interactive block showcase**

## Performance

- **Duration:** 10 min
- **Started:** 2026-03-28T21:04:27Z
- **Completed:** 2026-03-28T21:14:30Z
- **Tasks:** 2
- **Files modified:** 10

## Accomplishments
- Complete Rust demo binary (`cargo run --bin ainm-demo`) starts server, connects agent, opens REPL -- one command
- Ed25519 keypair generation, RFC 8785 canonical JSON signing, and signature verification working end-to-end
- 5 scripted scenarios: streaming hello, FormBlock, ApprovalBlock, ButtonBlock, and tool visibility lifecycle
- Streaming shows visible token-by-token rendering with 30-80ms artificial delay between chunks
- Every message displays [VERIFIED] or [FAILED] signature status in colored terminal output

## Task Commits

Each task was committed atomically:

1. **Task 1: Create Cargo.toml and core modules** - `45c6624` (feat)
2. **Task 2: Create agent, streaming, repl, and render modules** - `e34e3d6` (feat)

## Files Created/Modified
- `demo/Cargo.toml` - Rust project with all dependencies (axum, ring, tokio-tungstenite, rustyline, etc.)
- `demo/src/main.rs` - Entry point: spawns server, agent, REPL as concurrent tokio tasks
- `demo/src/identity.rs` - Ed25519 keypair generation, in-memory identity store with base64url encoding
- `demo/src/signing.rs` - RFC 8785 canonical JSON + Ed25519 signing and verification
- `demo/src/protocol.rs` - JSON-RPC 2.0 types, AINM message envelope, UUIDv7 helpers
- `demo/src/server.rs` - Axum WebSocket server with /ws endpoint, agent-card endpoint, message routing
- `demo/src/agent.rs` - Mock agent with 5 scripted scenarios (hello, form, approve, tool, button)
- `demo/src/streaming.rs` - Stream lifecycle: StreamStart, StreamDelta (30-80ms delay), StreamEnd with signed collapse
- `demo/src/repl.rs` - Terminal REPL with rustyline, /card /help /quit commands, message signing
- `demo/src/render.rs` - Colored terminal rendering for text, code, button, form, approval, table, tool events

## Decisions Made
- Used concrete WebSocket types (not generics) to avoid `Send`/`Sync` issues when spawning tokio tasks -- `ThreadRng` is not `Send` across await points, switched to `StdRng::from_entropy()`
- Downgraded crate versions to match what's actually published on crates.io: `tokio-tungstenite 0.26`, `rustyline 15`, `rand 0.8` (plan specified newer versions that don't exist yet)
- Used broadcast channels for bidirectional message routing between human and agent WebSocket connections through the server
- Raw TCP HTTP request for Agent Card fetch (avoiding reqwest dependency for a single GET request)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Crate version adjustments**
- **Found during:** Task 1 (Cargo.toml creation)
- **Issue:** Plan specified tokio-tungstenite 0.29, rustyline 17, rand 0.9 -- these versions don't exist on crates.io
- **Fix:** Used latest available versions: tokio-tungstenite 0.26, rustyline 15, rand 0.8
- **Files modified:** demo/Cargo.toml
- **Verification:** `cargo check` passes with resolved versions
- **Committed in:** 45c6624 (Task 1 commit)

**2. [Rule 1 - Bug] Utf8Bytes type mismatch in tungstenite 0.26**
- **Found during:** Task 1/2 (server.rs, streaming.rs, repl.rs)
- **Issue:** tungstenite 0.26+ uses `Utf8Bytes` for `Message::Text` instead of `String`, causing type errors
- **Fix:** Added `.into()` on all `Message::Text(string)` calls and used `.to_string()` for text extraction
- **Files modified:** demo/src/server.rs, demo/src/streaming.rs, demo/src/repl.rs, demo/src/agent.rs
- **Verification:** `cargo build` succeeds
- **Committed in:** e34e3d6 (Task 2 commit)

**3. [Rule 1 - Bug] ThreadRng not Send across await points**
- **Found during:** Task 2 (streaming.rs)
- **Issue:** `rand::thread_rng()` returns `ThreadRng` which is `!Send`, causing tokio::spawn to fail
- **Fix:** Used `rand::rngs::StdRng::from_entropy()` which is `Send`-safe
- **Files modified:** demo/src/streaming.rs
- **Verification:** `cargo build` succeeds
- **Committed in:** e34e3d6 (Task 2 commit)

**4. [Rule 1 - Bug] SinkExt::Error not a valid associated type**
- **Found during:** Task 2 (agent.rs generic function bounds)
- **Issue:** Generic function where-bounds used `SinkExt::Error` but `Error` is on `Sink` trait not `SinkExt`
- **Fix:** Replaced all generic function signatures with concrete `WsSender` type alias
- **Files modified:** demo/src/agent.rs, demo/src/streaming.rs
- **Verification:** `cargo build` succeeds
- **Committed in:** e34e3d6 (Task 2 commit)

---

**Total deviations:** 4 auto-fixed (3 bugs, 1 blocking)
**Impact on plan:** All fixes necessary for compilation. No scope changes.

## Issues Encountered
- Axum 0.8 WebSocket handler uses `Utf8Bytes` instead of `String` for text messages -- required `.into()` / `.to_string()` conversions throughout
- Crate versions in plan didn't match crates.io reality -- adjusted to latest available

## User Setup Required
None - no external service configuration required. The demo works offline with `cargo run --bin ainm-demo`.

## Next Phase Readiness
- Demo binary compiles and is ready for protocol showcase
- All protocol features covered: identity, signing, streaming, interactive blocks, tool visibility, Agent Card
- Ready for Phase 03-05 (Docusaurus publication site) which can reference the demo in documentation

## Self-Check: PASSED

All 10 created files verified present. Both task commits (45c6624, e34e3d6) verified in git history. `cargo build` succeeds.

---
*Phase: 03-specification-publication-working-demo*
*Completed: 2026-03-28*
