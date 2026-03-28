---
phase: 02-agent-native-features
plan: 02
subsystem: protocol-spec
tags: [interactive-blocks, declarative-ui, action-block, form-block, button-block, table-block, approval-block, a2ui]

# Dependency graph
requires:
  - phase: 01-protocol-kernel
    provides: "Content block base schema (type/version/source/fallback), message signing, thread-addressed delivery"
provides:
  - "Complete interactive content block specification (ButtonBlock, FormBlock, TableBlock, ApprovalBlock, ActionBlock)"
  - "Declarative component model for agent-driven UI"
  - "ActionBlock as signed thread message response mechanism"
  - "Error code range -32070 to -32074 for interactive content"
  - "8 form field types for structured data collection"
  - "ApprovalBlock state machine (pending -> approved/rejected/changes_requested)"
affects: [streaming, agent-card, compatibility, demo]

# Tech tracking
tech-stack:
  added: []
  patterns: ["Declarative component model (spec IS the catalog)", "ActionBlock as thread message for transparent interactions", "Semantic styles (primary/secondary/danger) for intent-based UI"]

key-files:
  created: ["spec/interactive.md"]
  modified: []

key-decisions:
  - "ActionBlocks are full signed messages visible to all thread participants (transparency by design)"
  - "8 form field types: text, textarea, number, select, multiselect, checkbox, date, datetime"
  - "3 semantic styles only (primary/secondary/danger) -- clients map to their own design system"
  - "ApprovalBlock uses server-enforced state machine with immutable terminal states"
  - "No arbitrary payload size limits at protocol level -- servers configure via capability negotiation"

patterns-established:
  - "Interactive block pattern: base schema + type-specific fields + action_id + fallback"
  - "ActionBlock response pattern: ref_message_id + ref_block_index + action_id + data"
  - "Error code allocation: -32070 to -32079 range for interactive content"

requirements-completed: [CONT-03, CONT-04, CONT-05]

# Metrics
duration: 5min
completed: 2026-03-28
---

# Phase 2 Plan 02: Interactive Content Blocks Summary

**Declarative interactive content spec with 5 block types (button/form/table/approval/action), 8 form field types, approval state machine, and ActionBlock-as-thread-message response pattern**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-28T19:28:28Z
- **Completed:** 2026-03-28T19:33:28Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Complete interactive content specification (1076 lines) covering all 5 block types with full JSON examples
- Declarative component model that prevents code injection by design (A2UI security model)
- End-to-end workflow examples showing form submission + approval lifecycle with full message envelopes
- Error handling with 5 defined error codes for interactive content validation
- Cross-references to content.md (base schema), security.md (signing, trust), and threading.md (delivery)

## Task Commits

Each task was committed atomically:

1. **Task 1: Write Interactive Content Specification** - `8f2856f` (feat)

## Files Created/Modified
- `spec/interactive.md` - Complete interactive content block specification defining ButtonBlock, FormBlock, TableBlock, ApprovalBlock, and ActionBlock

## Decisions Made
- ActionBlocks are full signed messages visible to all thread participants -- intentional transparency per D-07
- 8 form field types chosen for practical coverage: text, textarea, number, select, multiselect, checkbox, date, datetime
- 3 semantic styles only (primary/secondary/danger) -- clients map to their own design system, spec does not prescribe colors
- ApprovalBlock uses server-enforced state machine with immutable terminal states -- first ActionBlock wins in race conditions
- No arbitrary payload size limits at protocol level (CONT-05) -- servers configure via maxContentBlocks/maxMessageSize capability flags
- ActionBlock message type "action" added to Phase 2 message type set

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - spec is complete and self-contained.

## Next Phase Readiness
- Interactive content spec is ready for streaming spec (plan 01) to reference when defining which block types are streamable
- Agent Card spec (plan 03) can reference interactive block types for capability declaration
- Compatibility spec (plan 05) can map interactive blocks to A2A task artifacts

## Self-Check: PASSED

- spec/interactive.md: FOUND (1076 lines)
- 02-02-SUMMARY.md: FOUND
- Commit 8f2856f: FOUND

---
*Phase: 02-agent-native-features*
*Completed: 2026-03-28*
