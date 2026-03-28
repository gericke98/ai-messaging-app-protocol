---
phase: 03-specification-publication-working-demo
plan: 02
subsystem: spec
tags: [json-schema, interactive, streaming, threading, agent-card, memory, tool-visibility, compatibility, ainm]

# Dependency graph
requires:
  - phase: 03-specification-publication-working-demo plan 01
    provides: common.schema.json with shared types (AINMAddress, UUIDv7, RFC3339Timestamp, ContentBlockBase, MessageEnvelope)
provides:
  - 7 JSON Schema 2020-12 files for Phase 2 protocol domains
  - interactive.schema.json (ButtonBlock, FormBlock, TableBlock, ApprovalBlock, ActionBlock)
  - streaming.schema.json (StreamStart, StreamDelta, StreamEnd, CancelStream)
  - threading.schema.json (ThreadCreateParams, SubThreadCreateParams, ThreadParticipant, PresenceUpdate)
  - agent-card.schema.json (AgentCard A2A-compatible with AINMExtensions)
  - memory.schema.json (ContextFrame, MemoryStoreParams, MemoryQueryParams, ArchivalFact)
  - tool-visibility.schema.json (ToolCallStarted, ToolCallProgress, ToolCallCompleted)
  - compatibility.schema.json (MCPBridgeConfig, MCPToolMapping, A2ABridgeConfig, A2ATaskMapping)
affects: [03-03 (AsyncAPI transport binding), 03-04 (Docusaurus spec site), 03-05 (working demo)]

# Tech tracking
tech-stack:
  added: []
  patterns: [allOf with ContentBlockBase $ref for interactive blocks, relative $ref to common.schema.json for shared types, error code enums as description-only objects]

key-files:
  created:
    - schemas/interactive.schema.json
    - schemas/streaming.schema.json
    - schemas/threading.schema.json
    - schemas/agent-card.schema.json
    - schemas/memory.schema.json
    - schemas/tool-visibility.schema.json
    - schemas/compatibility.schema.json
  modified: []

key-decisions:
  - "ActionBlock is NOT allOf ContentBlockBase -- it is a response message type, not a content block, per interactive.md"
  - "PresenceUpdate uses combined enum (online/offline/available/busy/processing) covering both human and agent states for schema-level validation"
  - "Tool visibility events include thread_id/from/timestamp fields because they are persistent signed thread messages (MessageEnvelope wrapped)"
  - "Agent Card AINMExtensions includes address ($ref AINMAddress) and type (const agent) per plan action spec"

patterns-established:
  - "Interactive blocks: allOf ContentBlockBase + domain-specific required/properties"
  - "Error codes: described as $defs objects with code-to-name mappings, not enforced enums"
  - "Streaming: ephemeral events (StreamDelta) have minimal fields; permanent records (StreamEnd) trigger collapse"
  - "Memory: MemoryStoreParams tier restricted to archival; MemoryQueryParams tier includes conversation+archival"

requirements-completed: [SPEC-02]

# Metrics
duration: 6min
completed: 2026-03-28
---

# Phase 3 Plan 2: Phase 2 Domain JSON Schemas Summary

**7 JSON Schema 2020-12 files formalizing interactive, streaming, threading, agent-card, memory, tool-visibility, and compatibility domains with $ref links to common.schema.json**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-28T20:53:58Z
- **Completed:** 2026-03-28T21:00:00Z
- **Tasks:** 2
- **Files created:** 7

## Accomplishments
- Created all 7 remaining JSON Schema files covering the Phase 2 agent-native protocol domains
- Combined with Plan 01's 5 schemas, all 12 per-domain schema files are now complete per D-05
- Every schema uses JSON Schema Draft 2020-12 with canonical $id URIs and relative $ref to common.schema.json
- All types derived from spec file inline examples -- field names, enums, and constraints match the specification exactly

## Task Commits

Each task was committed atomically:

1. **Task 1: Create interactive, streaming, and threading schemas** - `8ba63da` (feat)
2. **Task 2: Create agent-card, memory, tool-visibility, and compatibility schemas** - `24f7151` (feat)

## Files Created/Modified
- `schemas/interactive.schema.json` - ButtonBlock, FormBlock (8 field types), TableBlock, ApprovalBlock (4-state machine), ActionBlock (response type)
- `schemas/streaming.schema.json` - StreamStart (immutable block layout), StreamDelta (ephemeral), StreamEnd (triggers collapse), CancelStream
- `schemas/threading.schema.json` - ThreadCreateParams, SubThreadCreateParams (one-level nesting), ThreadParticipant, PresenceUpdate (5 combined states)
- `schemas/agent-card.schema.json` - AgentCard (A2A superset), AINMExtensions (tools, streaming, threading, memory capabilities)
- `schemas/memory.schema.json` - ContextFrame (32KB budget, 50/20/20/10 split), MemoryStore/Query/Evict params, ArchivalFact
- `schemas/tool-visibility.schema.json` - ToolCallStarted/Progress/Completed as persistent signed thread messages
- `schemas/compatibility.schema.json` - MCPBridgeConfig, MCPToolMapping, A2ABridgeConfig, A2ATaskMapping with proxy participant pattern

## Decisions Made
- ActionBlock does NOT extend ContentBlockBase (it is a full signed message, not a content block) per interactive.md
- Combined human+agent presence states into single enum for schema validation simplicity
- Tool visibility events include envelope context fields (thread_id, from, timestamp) since they are persistent thread messages
- Agent Card AINMExtensions includes address and type fields per plan specification

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - all schemas are complete with full type definitions.

## Next Phase Readiness
- All 12 JSON Schema files complete in schemas/ directory
- Ready for Plan 03 (AsyncAPI transport binding) which references these schemas
- Ready for Plan 04 (Docusaurus spec site) which will publish schema documentation
- Ready for Plan 05 (working demo) which will validate messages against these schemas

## Self-Check: PASSED

- All 7 schema files exist
- SUMMARY.md exists
- Commit 8ba63da found (Task 1)
- Commit 24f7151 found (Task 2)

---
*Phase: 03-specification-publication-working-demo*
*Completed: 2026-03-28*
