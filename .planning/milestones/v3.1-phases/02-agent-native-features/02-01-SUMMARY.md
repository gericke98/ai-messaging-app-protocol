---
phase: 02-agent-native-features
plan: 01
subsystem: protocol-spec
tags: [streaming, json-rpc, websocket, ed25519, collapse, parallel-streaming]

# Dependency graph
requires:
  - phase: 01-protocol-kernel
    provides: "transport.md (WebSocket binding, capability negotiation), security.md (message signing, ephemeral vs persistent), content.md (block base schema)"
provides:
  - "Complete streaming specification (spec/streaming.md) with 5 JSON-RPC methods"
  - "Streaming lifecycle: Start-Delta-End triplet with collapse into signed messages"
  - "Parallel multi-block streaming with block index addressing"
  - "Client-initiated cancellation protocol"
  - "Streaming error code range -32040 to -32044"
  - "parallelStreaming capability flag for ainm.initialize handshake"
affects: [02-agent-native-features, 03-publication-demo]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Ephemeral event authentication via WebSocket session (not individually signed)"
    - "Start-Delta-End triplet lifecycle for progressive content delivery"
    - "Block layout immutability -- declared upfront, no mid-stream changes"
    - "Collapse semantics -- ephemeral deltas consolidate into permanent signed messages"
    - "Block index addressing for multi-block delta routing"

key-files:
  created:
    - spec/streaming.md
  modified: []

key-decisions:
  - "Streaming deltas are ephemeral (session-authenticated) not individually signed -- performance over per-event integrity, with collapse providing final signed record"
  - "Block layout immutable after stream-start -- agents needing more blocks must end current stream and start new one"
  - "Only TextBlock and CodeBlock streamable in v1 -- other types populated in collapsed message"
  - "parallelStreaming defaults to false if omitted -- conservative default for client capability"
  - "Error code range -32040 to -32044 with five specific codes for streaming failures"

patterns-established:
  - "Ephemeral vs persistent event distinction: streaming deltas are ephemeral, collapsed messages are persistent"
  - "metadata object on message envelope for audit trail (stream_id, cancelled flag, error info)"
  - "Capability flag pattern for feature negotiation (parallelStreaming extends ainm.initialize)"

requirements-completed: [STRM-01, STRM-02, STRM-03, STRM-04]

# Metrics
duration: 4min
completed: 2026-03-28
---

# Phase 02 Plan 01: Streaming Specification Summary

**Complete streaming spec defining Start-Delta-End lifecycle with parallel multi-block support, collapse into Ed25519-signed messages, client cancellation, and error handling across 5 JSON-RPC methods**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-28T19:28:22Z
- **Completed:** 2026-03-28T19:33:14Z
- **Tasks:** 1
- **Files created:** 1

## Accomplishments

- Wrote 1010-line streaming specification covering the complete streaming lifecycle
- Defined 5 JSON-RPC methods (ainm.stream.start, delta, end, cancel, error) with full field definitions and constraints
- Specified parallel multi-block streaming with block index addressing and interleaved deltas
- Defined collapse semantics for completed, cancelled, and error-terminated streams
- Established error code range -32040 to -32044 with recovery procedures and error behavior matrix
- Added parallelStreaming capability flag extending the ainm.initialize handshake
- Documented security model distinguishing ephemeral session-authenticated deltas from Ed25519-signed collapsed messages

## Task Commits

Each task was committed atomically:

1. **Task 1: Write Streaming Specification** - `d5fe5ef` (feat)

## Files Created/Modified

- `spec/streaming.md` - Complete streaming specification: lifecycle, deltas, collapse, cancellation, errors, parallel streaming, capability negotiation, security model, examples

## Decisions Made

- **Ephemeral authentication model:** Streaming deltas authenticated by WebSocket session, not individually signed. Signing hundreds of deltas/second would add prohibitive CPU overhead. Only the collapsed permanent message at stream-end is Ed25519-signed.
- **Block layout immutability:** Once declared in stream-start, block layout cannot change. If an agent needs additional blocks, it ends the current stream and starts a new one. Prevents client rendering inconsistencies.
- **v1 streamable types:** Only TextBlock and CodeBlock support streaming deltas. ImageBlock and FileBlock appear in block layout but are populated in the collapsed message. Keeps v1 scope focused.
- **parallelStreaming defaults to false:** Conservative default -- clients must explicitly opt in. Servers may still send multi-block streams, and clients must handle them gracefully regardless.
- **Five error codes (-32040 to -32044):** Stream not found, invalid block index, stream already ended, agent generation error, stream timeout. Each with defined partial preservation behavior.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Streaming specification complete, ready for interactive content blocks (Plan 02)
- The parallelStreaming capability flag pattern can be reused for future capability additions
- The collapse semantics and metadata object pattern (stream_id, cancelled, error_code) establish conventions for other Phase 2 specs
- The ephemeral vs persistent event model applies to tool visibility events (Plan 04)

## Verification Results

| Check | Expected | Actual | Status |
|-------|----------|--------|--------|
| File length | >400 lines | 1010 lines | PASS |
| Streaming event references | >= 10 | 77 | PASS |
| RFC keywords (MUST/SHOULD/MAY) | >= 30 | 45 | PASS |
| block_index references | present | 30 | PASS |
| blocks[] array references | present | 3 | PASS |
| Collapse semantics | present | 37 references | PASS |
| parallelStreaming capability | present | 5 references | PASS |
| Error codes (-32040 to -32044) | 5 defined | 5 defined (23 refs) | PASS |
| JSON examples | >= 10 | 67 | PASS |
| Cross-references | present | 17 references | PASS |

## Self-Check: PASSED

- spec/streaming.md: FOUND
- 02-01-SUMMARY.md: FOUND
- Commit d5fe5ef: FOUND

---
*Phase: 02-agent-native-features*
*Completed: 2026-03-28*
