---
phase: 06-demo-feature-gaps-cleanup
plan: 01
subsystem: demo
tags: [rust, axum, websocket, memory, etag, http-caching, json-rpc]

# Dependency graph
requires:
  - phase: 04-demo-conformance-hardening
    provides: "Working demo with memory store/query, archival_store HashMap, render_memory_event"
provides:
  - "ainm.memory.evict server handler with error codes -32051, -32053, -32054"
  - "handle_evict agent function for /evict REPL command"
  - "/evict REPL command with help listing and ainm.memory.evicted notification handler"
  - "ETag-aware Agent Card HTTP endpoint with If-None-Match 304 support"
affects: [06-demo-feature-gaps-cleanup]

# Tech tracking
tech-stack:
  added: []
  patterns: ["ETag caching via SHA-256 hash of serialized JSON body", "Memory evict follows store/query pattern with error code cascade"]

key-files:
  created: []
  modified:
    - demo/src/server.rs
    - demo/src/agent.rs
    - demo/src/repl.rs
    - demo/src/render.rs

key-decisions:
  - "Evict uses same key (user_preference.demo) as store for end-to-end demo coherence"
  - "ETag computed from SHA-256 of serde_json::to_vec output, base64url-encoded, wrapped in double quotes per HTTP spec"

patterns-established:
  - "Memory operation error cascade: wrong tier (-32051) > missing key (-32053) > not found (-32054)"
  - "ETag caching: hash body, compare If-None-Match, return 304 or 200 with ETag header"

requirements-completed: [MEMO-01, DISC-05, SPEC-05]

# Metrics
duration: 3min
completed: 2026-03-29
---

# Phase 06 Plan 01: Memory Evict + Agent Card ETag Summary

**End-to-end memory eviction via /evict REPL command and Agent Card ETag caching with If-None-Match 304 support**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-29T00:33:41Z
- **Completed:** 2026-03-29T00:36:51Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Full memory evict flow wired: REPL /evict -> agent handle_evict -> server ainm.memory.evict -> ainm.memory.evicted notification -> REPL render
- Agent Card endpoint returns ETag header on 200 and honors If-None-Match with 304 Not Modified
- Three error codes implemented for evict: wrong tier (-32051), missing key (-32053), fact not found (-32054)
- /help text updated to list /evict command with usage guidance

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement memory evict end-to-end** - `731e2eb` (feat)
2. **Task 2: Add ETag caching to Agent Card HTTP endpoint** - `1848c0e` (feat)

## Files Created/Modified
- `demo/src/server.rs` - Added ainm.memory.evict handler and ETag-aware agent_card_handler
- `demo/src/agent.rs` - Added handle_evict function and /evict dispatch
- `demo/src/repl.rs` - Added /evict command, /help listing, ainm.memory.evicted handler
- `demo/src/render.rs` - Added evicted branch to render_memory_event

## Decisions Made
- Evict uses same key (user_preference.demo) as store for end-to-end demo coherence -- user runs /memory then /evict to see full CRUD cycle
- ETag computed from SHA-256 of serde_json::to_vec output, base64url-encoded, wrapped in double quotes per HTTP spec (RFC 7232)
- Replaced Json<serde_json::Value> return type with Response for full header control in agent_card_handler

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Memory CRUD cycle complete (store, query, evict) -- all archival operations exercised
- Agent Card HTTP caching ready -- demo proves DISC-05 conformance
- Plan 02 (warning cleanup) can proceed immediately

## Self-Check: PASSED

All files exist, all commits verified.

---
*Phase: 06-demo-feature-gaps-cleanup*
*Completed: 2026-03-29*
