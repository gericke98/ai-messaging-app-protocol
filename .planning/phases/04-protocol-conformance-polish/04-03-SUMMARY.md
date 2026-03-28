---
phase: 04-protocol-conformance-polish
plan: 03
subsystem: demo
tags: [rust, json-rpc, memory, context-frame, mcp-bridge, a2a-bridge, conformance]

# Dependency graph
requires:
  - phase: 04-protocol-conformance-polish
    provides: "Demo schema conformance fixes, init auth, complete Agent Card"
provides:
  - "/memory REPL command exercising ainm.memory.store, ainm.memory.query, ainm.context.frame"
  - "/bridge REPL command exercising MCP tool bridging and A2A task lifecycle mapping"
  - "Server-side archival fact store with store/query handlers"
  - "Context frame delivery after memory query"
  - "render_memory_event and render_bridge_event terminal renderers"
affects: [protocol-conformance-polish]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Memory operations use private JSON-RPC exchanges routed through server"
    - "Bridge compatibility uses existing AINM message types (no new protocol methods)"
    - "Server-side archival store with HashMap keyed by fact key"

key-files:
  created: []
  modified:
    - "demo/src/agent.rs"
    - "demo/src/server.rs"
    - "demo/src/repl.rs"
    - "demo/src/render.rs"

key-decisions:
  - "Memory store/query routed through server as JSON-RPC requests, not direct agent-to-agent"
  - "Bridge demo reuses existing tool visibility events for MCP section (no new rendering needed)"
  - "A2A task lifecycle mapped to ainm.presence.update notification for processing state"

patterns-established:
  - "Memory notifications forwarded to human client via custom ainm.memory.stored and ainm.memory.query.result methods"
  - "ainm.presence.update used for A2A task working state indicator"

requirements-completed: [MEMO-01, MEMO-02, MEMO-03, MEMO-04, MEMO-05, COMP-01, COMP-02, COMP-03, COMP-04]

# Metrics
duration: 4min
completed: 2026-03-28
---

# Phase 04 Plan 03: Memory and Bridge Demo Scenarios Summary

**Added /memory and /bridge REPL commands exercising three-tier memory operations (store, query, context frame) and compatibility bridges (MCP tool bridging, A2A task lifecycle) with schema-conformant JSON**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-28T23:11:08Z
- **Completed:** 2026-03-28T23:15:24Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Implemented /memory scenario: ainm.memory.store, ainm.memory.query, and ainm.context.frame with full schema-conformant JSON field names
- Implemented /bridge scenario: MCP tool call bridging (tool.call.started/progress/completed) and A2A task lifecycle (submitted/working/completed) using existing AINM message types
- Added server-side archival fact store with HashMap, store/query handlers, and automatic context frame delivery after query
- Added render_memory_event (store confirmation, query results, context frame sections with byte counts) and render_bridge_event (A2A lifecycle rendering) terminal renderers
- All 9 previously undemonstrated requirements (MEMO-01..05, COMP-01..04) now exercised in the demo

## Task Commits

Each task was committed atomically:

1. **Task 1: /memory scenario (agent handler, server handlers, REPL command, rendering)** - `f65c232` (feat)
2. **Task 2: /bridge scenario (agent handler, REPL command, rendering) and full demo verification** - `8f477b7` (feat)

## Files Created/Modified
- `demo/src/agent.rs` - Added handle_memory (store/query requests) and handle_bridge (MCP tool events, A2A presence updates, lifecycle messages)
- `demo/src/server.rs` - Added archival_store HashMap, ainm.memory.store handler, ainm.memory.query handler, ainm.context.frame notification, ainm.presence.update passthrough
- `demo/src/repl.rs` - Added /memory and /bridge REPL commands, memory/context_frame/presence notification rendering dispatch, updated /help
- `demo/src/render.rs` - Added render_memory_event (stored/query_result/context_frame) and render_bridge_event (mcp_header/a2a_header/a2a_submitted/a2a_working/a2a_completed)

## Decisions Made
- Memory store/query routed through server as JSON-RPC requests (agent sends request, server handles and returns response), not direct agent-to-agent
- Bridge demo reuses existing tool visibility events for MCP section -- render_tool_event handles all tool rendering, no duplication
- A2A task lifecycle mapped to ainm.presence.update notification (status: "processing") for the working state, matching the spec's status_mapping

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Passed from_addr to handle_agent_connection**
- **Found during:** Task 1
- **Issue:** from_addr (authenticated identity) was not in scope inside handle_agent_connection, needed for ainm.memory.store stored_by field
- **Fix:** Added from_addr parameter to handle_agent_connection function signature
- **Files modified:** demo/src/server.rs
- **Verification:** cargo check passes
- **Committed in:** f65c232 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Minor function signature change. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All 3 plans in Phase 04 complete
- Demo now exercises all protocol features: streaming, forms, buttons, approvals, tool visibility, memory operations, and bridge compatibility
- All 9 previously undemonstrated requirements now covered
- Protocol conformance polish phase is complete

## Self-Check: PASSED

---
*Phase: 04-protocol-conformance-polish*
*Completed: 2026-03-28*
