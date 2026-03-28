---
phase: 02-agent-native-features
plan: 03
subsystem: protocol-spec
tags: [agent-card, a2a, tool-visibility, discovery, etag, mcp, json-schema]

# Dependency graph
requires:
  - phase: 01-protocol-kernel
    provides: identity.md (agent_card_url field), security.md (message signing), content.md (block base schema), threading.md (thread model)
provides:
  - Agent Card discovery and schema specification (spec/agent-card.md)
  - Tool call visibility event lifecycle specification (spec/tool-visibility.md)
affects: [02-agent-native-features remaining plans, reference implementation, iOS client]

# Tech tracking
tech-stack:
  added: []
  patterns: [A2A-compatible superset with namespaced ainm extension object, MCP-aligned tool listing format, thread-scoped signed tool events]

key-files:
  created:
    - spec/agent-card.md
    - spec/tool-visibility.md
  modified: []

key-decisions:
  - "AINM Agent Cards are A2A superset with all AINM extensions under top-level ainm namespace object to avoid A2A validator breakage"
  - "Tool events are persistent signed thread messages with TextBlock content[] fallback for backward compatibility"
  - "Agent Card error codes use -32060 to -32069 range, tool visibility reuses existing transport error codes"
  - "Tool call_id uses UUIDv7 with optional tc- prefix for lifecycle linking across started/progress/completed events"

patterns-established:
  - "A2A compatibility pattern: top-level fields match A2A spec exactly, AINM extensions namespaced under ainm object"
  - "Thread-scoped event pattern: tool events follow same envelope as regular messages with specialized type values"
  - "Collapsible indicator pattern: default collapsed render with tool name + status, expandable to show full details"

requirements-completed: [DISC-01, DISC-02, DISC-03, DISC-04, DISC-05, TOOL-01, TOOL-02, TOOL-03, TOOL-04, TOOL-05]

# Metrics
duration: 5min
completed: 2026-03-28
---

# Phase 02 Plan 03: Agent Card & Tool Visibility Summary

**A2A-compatible Agent Card schema with AINM extensions for discovery/capabilities, plus three-event tool call lifecycle (started/progress/completed) as transparent thread-scoped signed messages**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-28T19:28:49Z
- **Completed:** 2026-03-28T19:34:45Z
- **Tasks:** 2
- **Files created:** 2

## Accomplishments
- Agent Card specification with A2A-compatible top-level fields and namespaced AINM extension object covering contentTypes, streaming, threading, memory, and tools
- Tool Visibility specification with complete three-event lifecycle (started, progress, completed) including determinate/indeterminate progress, collapsible rendering guidance, and security considerations for input visibility
- Both specs follow Phase 1 conventions: RFC keywords, inline JSON examples, cross-references to identity.md/security.md/threading.md/content.md

## Task Commits

Each task was committed atomically:

1. **Task 1: Write Agent Card Specification** - `319341d` (feat)
2. **Task 2: Write Tool Visibility Specification** - `9660391` (feat)

## Files Created/Modified
- `spec/agent-card.md` - Agent Card discovery, A2A-compatible schema, AINM extension object, ETag caching, error codes, three full examples (755 lines)
- `spec/tool-visibility.md` - Tool call lifecycle events, thread visibility, collapsible rendering guidance, security considerations, full lifecycle examples (645 lines)

## Decisions Made
- Agent Card error code range -32060 to -32069 (Agent Card Not Found, Invalid Format, Version Mismatch) -- does not overlap with identity (-32001 to -32007) or security (-32010 to -32013) ranges
- Tool visibility reuses existing transport error codes rather than introducing new ones -- tool failures are application-level (status: "error" on completed event), not protocol-level
- Tool call_id uses UUIDv7 with optional tc- prefix for clear identification in logs and message streams
- Agent Card tools array exposes only input schema (not output schema) as a deliberate information boundary per security considerations

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - both specifications are complete standalone documents with no placeholder content.

## Next Phase Readiness
- Agent Card spec provides the ainm.tools array that tool-visibility.md references (tool.name MUST match ainm.tools[].name)
- Both specs are ready for use by remaining Phase 2 plans (streaming, interactive content, memory, compatibility)
- The three-event tool lifecycle pattern established here can be referenced by the memory spec for memory operation events

## Self-Check: PASSED

- [x] spec/agent-card.md exists (755 lines)
- [x] spec/tool-visibility.md exists (645 lines)
- [x] 02-03-SUMMARY.md exists
- [x] Commit 319341d found (Task 1)
- [x] Commit 9660391 found (Task 2)

---
*Phase: 02-agent-native-features*
*Completed: 2026-03-28*
