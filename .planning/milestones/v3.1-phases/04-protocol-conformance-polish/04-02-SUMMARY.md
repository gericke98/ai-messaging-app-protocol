---
phase: 04-protocol-conformance-polish
plan: 02
subsystem: demo
tags: [rust, axum, ed25519, json-rpc, websocket, agent-card, streaming, conformance]

# Dependency graph
requires:
  - phase: 03-specification-publication-working-demo
    provides: "Working Rust CLI demo and JSON Schema definitions"
provides:
  - "Demo field names exactly match JSON Schema sources of truth"
  - "Ed25519-signed init handshake with server nonce response"
  - "Complete Agent Card with all required ainm sub-objects (streaming, threading, memory)"
  - "StreamDelta without undocumented sequence field"
affects: [04-protocol-conformance-polish]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "sign_init pattern for init handshake authentication"
    - "Agent Card ainm extensions with full sub-object hierarchy"

key-files:
  created: []
  modified:
    - "demo/src/agent.rs"
    - "demo/src/repl.rs"
    - "demo/src/server.rs"
    - "demo/src/render.rs"
    - "demo/src/streaming.rs"
    - "demo/src/signing.rs"
    - "demo/src/main.rs"

key-decisions:
  - "sign_init uses identity|timestamp payload format per transport.md Section 4.3"
  - "Server nonce uses UUIDv7 via protocol::new_message_id for simplicity"
  - "Agent Card ainm.type set to 'agent' for demo helper identity"

patterns-established:
  - "All demo JSON field names must exactly match JSON Schema $defs (camelCase enforcement)"
  - "Init handshake includes Ed25519 signature in params"

requirements-completed: [TRNS-01, SECU-04, STRM-01, CONT-03, CONT-04, DISC-02, SPEC-05]

# Metrics
duration: 3min
completed: 2026-03-28
---

# Phase 04 Plan 02: Demo Schema Conformance Summary

**Fix all demo-to-schema field name mismatches, add Ed25519-signed init handshake, and complete Agent Card with all required ainm sub-objects**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-28T23:04:15Z
- **Completed:** 2026-03-28T23:06:55Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Aligned all demo JSON field names with JSON Schema sources of truth (protocolVersion, contentTypes, toolVisibility, title, value)
- Added Ed25519-signed init handshake with sign_init function, server nonce response, and serverInfo object
- Completed Agent Card with all required ainm sub-objects: streaming (parallelBlocks, preferredChunkSize), threading (subThreads, maxConcurrentThreads), memory (archivalSupported, contextFramePreferred)
- Removed undocumented sequence field from StreamDelta
- Enhanced Agent Card terminal rendering with threading, memory, protocol version display

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix init field names, capabilities, ApprovalBlock, and remove sequence from StreamDelta** - `ee18a11` (fix)
2. **Task 2: Add init auth signature, server nonce/serverInfo, and fix Agent Card** - `b384e2c` (feat)

## Files Created/Modified
- `demo/src/agent.rs` - Fixed init field names (protocolVersion, contentTypes, toolVisibility), ApprovalBlock (title, value), added init signature
- `demo/src/repl.rs` - Fixed init field names, added init signature
- `demo/src/server.rs` - Fixed init response with serverInfo, nonce, schema-conformant capabilities
- `demo/src/render.rs` - Fixed ApprovalBlock to read title instead of prompt, added threading/memory/protocol Agent Card display
- `demo/src/streaming.rs` - Removed undocumented sequence field and variable from StreamDelta
- `demo/src/signing.rs` - Added sign_init function for Ed25519-signed init handshake
- `demo/src/main.rs` - Complete Agent Card with all ainm sub-objects, added /memory and /bridge help lines

## Decisions Made
- sign_init uses identity|timestamp payload format per transport.md Section 4.3
- Server nonce uses UUIDv7 via protocol::new_message_id for simplicity
- Agent Card ainm.type set to "agent" for demo helper identity

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Demo now uses exact schema field names across all 7 source files
- Init handshake includes cryptographic authentication
- Agent Card is complete with all required ainm extensions
- Ready for Plan 03 (memory/bridge command implementation using the help lines added here)

## Self-Check: PASSED

All 7 modified files verified present. Both task commits (ee18a11, b384e2c) verified in git log. SUMMARY.md created.

---
*Phase: 04-protocol-conformance-polish*
*Completed: 2026-03-28*
