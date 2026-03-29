---
phase: 05-demo-handshake-discovery-conformance
plan: 01
subsystem: demo
tags: [websocket, handshake, json-rpc, ainm-initialized, transport]

# Dependency graph
requires:
  - phase: 04-protocol-conformance-polish
    provides: "Demo with ainm.initialize handshake and Ed25519 signing"
provides:
  - "Complete ainm.initialized handshake lifecycle in demo (clients + server enforcement)"
  - "Server-side -32021 error enforcement for pre-handshake method calls"
affects: [05-02, demo, transport-conformance]

# Tech tracking
tech-stack:
  added: []
  patterns: ["Per-connection inline handshake enforcement loop (no global state)"]

key-files:
  created: []
  modified:
    - demo/src/repl.rs
    - demo/src/agent.rs
    - demo/src/server.rs

key-decisions:
  - "Handshake enforced per-connection inline with no ServerState additions"
  - "Empty params {} used for ainm.initialized notification (pragmatically acceptable per spec)"

patterns-established:
  - "Pre-dispatch enforcement loop: server reads messages in loop before handler dispatch, breaks on expected notification"

requirements-completed: [TRNS-01, TRNS-04, SPEC-05]

# Metrics
duration: 4min
completed: 2026-03-29
---

# Phase 05 Plan 01: Demo Handshake Lifecycle Summary

**Complete ainm.initialized handshake enforcement in demo -- both clients send notification, server blocks pre-handshake methods with -32021**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-29T00:06:03Z
- **Completed:** 2026-03-29T00:10:03Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- REPL and agent clients now send `ainm.initialized` JSON-RPC notification after receiving init response, completing the handshake lifecycle per transport.md Section 4.3
- Server enforces handshake sequence: waits for `ainm.initialized` before dispatching to agent/human connection handlers
- Server returns `-32021` ("Not initialized") error for any JSON-RPC request received before `ainm.initialized`
- Non-initialized notifications silently dropped per JSON-RPC spec (no error response for notifications)
- Closes INT-01 from v3.1 milestone audit

## Task Commits

Each task was committed atomically:

1. **Task 1: Add ainm.initialized notification to both demo clients** - `83fddb9` (feat)
2. **Task 2: Add server-side handshake enforcement loop** - `a59d01e` (feat)

## Files Created/Modified
- `demo/src/repl.rs` - Added ainm.initialized notification send after init response
- `demo/src/agent.rs` - Added ainm.initialized notification send after init response
- `demo/src/server.rs` - Added handshake enforcement loop with -32021 error for pre-handshake methods

## Decisions Made
- Handshake enforced per-connection inline (loop in handle_ws_connection) with no new fields on ServerState -- keeps state management simple and scoped
- Used `serde_json::json!({})` as params for ainm.initialized since `new_notification` requires a Value argument; spec does not forbid empty params object

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Known Stubs

None - all code is fully functional with no placeholder data.

## Next Phase Readiness
- Handshake lifecycle complete, ready for Plan 02 (discovery conformance)
- `cargo build` succeeds with no new warnings or errors

---
*Phase: 05-demo-handshake-discovery-conformance*
*Completed: 2026-03-29*
