---
phase: 02-agent-native-features
plan: 04
subsystem: protocol-spec
tags: [memory, context-frame, archival, conversation, letta, memgpt, json-rpc]

# Dependency graph
requires:
  - phase: 01-protocol-kernel
    provides: "Threading model (thread.join), transport (JSON-RPC, capabilities), identity (per-participant), security (session auth)"
provides:
  - "Three-tier memory model specification (active context, conversation memory, archival memory)"
  - "Context frame schema and delivery semantics"
  - "Memory JSON-RPC methods: ainm.memory.store, ainm.memory.query, ainm.memory.evict, ainm.context.frame"
  - "Memory capability negotiation (memory.conversation, memory.archival, maxContextFrameSize)"
  - "Error code range -32050 to -32055 for memory operations"
affects: [agent-card, tool-visibility, compatibility, reference-implementation]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Three-tier memory model: active (always available), conversation (queryable), archival (per-participant persistent)"
    - "Context frame budget allocation: 50% recent messages, 20% summary, 20% archival, 10% participants"
    - "Private memory operations: queries are NOT thread-visible"
    - "Key collision replacement: storing to existing key replaces the fact"

key-files:
  created:
    - "spec/memory.md"
  modified: []

key-decisions:
  - "Memory operations are private JSON-RPC exchanges, not thread messages -- agents can query without broadcasting strategy"
  - "Archival facts use key collision replacement -- storing to same key updates rather than duplicating"
  - "Context frames delivered on thread.join, @-mention, and server-initiated refresh (max once/minute)"
  - "Recommended defaults: 32KB context frame, 1000 facts per participant, 100KB archival storage"

patterns-established:
  - "Error code range -32050 to -32059 for memory domain (follows -32001/-32010/-32020/-32030 pattern from prior specs)"
  - "Budget-constrained context assembly prevents context explosion in long-lived threads"
  - "Per-participant archival isolation with explicit sharing via thread messages"

requirements-completed: [MEMO-01, MEMO-02, MEMO-03, MEMO-04, MEMO-05, MEMO-06]

# Metrics
duration: 5min
completed: 2026-03-28
---

# Phase 02 Plan 04: Memory Management Summary

**Three-tier memory model (active/conversation/archival) with structured context frames, store/query/evict operations, and per-participant archival isolation -- covering MEMO-01 through MEMO-06**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-28T19:29:15Z
- **Completed:** 2026-03-28T19:34:52Z
- **Tasks:** 1
- **Files created:** 1

## Accomplishments
- Complete memory management specification (1133 lines) covering all 6 MEMO requirements
- Three-tier model with clear shared vs per-participant scoping (D-16 honored)
- Four JSON-RPC methods fully specified with request/response schemas and server behavior
- Context frame schema with budget allocation preventing context explosion (Pitfall 3)
- Capability negotiation extensions for memory features
- Six error codes in dedicated -32050 to -32055 range
- Five end-to-end examples including complete thread-join-to-response flow

## Task Commits

Each task was committed atomically:

1. **Task 1: Write Memory Management Specification** - `70d7399` (feat)

## Files Created/Modified
- `spec/memory.md` - Complete memory management specification: three-tier model, context frames, memory operations, capability negotiation, error handling, retention policies, security considerations, and examples

## Decisions Made
- Memory operations (store/query/evict) are private JSON-RPC request/response exchanges -- not visible to other thread participants (per RESEARCH.md Open Question 3 and UI-SPEC Contract 13)
- Archival facts use key collision replacement: storing a fact with the same key as an existing fact replaces it, avoiding explicit evict-then-store patterns
- Context frames delivered on three triggers: thread.join (MUST), @-mention (SHOULD), server-initiated refresh (MAY, max once/minute)
- Recommended defaults chosen: 32KB max context frame, 1000 facts max per participant, 100KB max archival storage per participant
- Error code range -32050 to -32055 continues the domain-segmented pattern from identity (-32001), security (-32010), transport (-32020), threading (-32030)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Known Stubs

None - the specification is complete prose with no placeholder content.

## Next Phase Readiness
- Memory spec is complete and ready for cross-referencing from other Phase 2 specs
- Agent Card spec (plan 03) can reference memory tier support capabilities
- Tool visibility spec (plan 02) can reference memory operations as private (non-thread) operations pattern
- Compatibility spec (plan 05) can map MCP/A2A memory concepts to AINM tiers

## Self-Check: PASSED

- spec/memory.md: FOUND
- 02-04-SUMMARY.md: FOUND
- commit 70d7399: FOUND

---
*Phase: 02-agent-native-features*
*Completed: 2026-03-28*
