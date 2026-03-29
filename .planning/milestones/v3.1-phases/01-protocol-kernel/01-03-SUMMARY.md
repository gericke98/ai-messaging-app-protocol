---
phase: 01-protocol-kernel
plan: 03
subsystem: protocol-spec
tags: [threading, sub-threads, mixed-participation, presence, mentions, human-agent-equality]

# Dependency graph
requires:
  - phase: 01-protocol-kernel plan 01
    provides: Identity specification (address format, type field) and security specification (message envelope, signing, trust annotations)
  - phase: 01-protocol-kernel plan 02
    provides: Content specification (TextBlock mentions array) and transport specification (thread operations, presence methods)
provides:
  - "AINM threading specification: flat thread model with UUIDv7 thread_id and chronological message ordering"
  - "Sub-threading with one-level nesting limit and explicit context inheritance rules"
  - "Mixed human-agent thread participation with uniform interface and adapted presence semantics"
  - "@-mention system working uniformly across human and agent participants"
  - "Complete Phase 1 protocol kernel: 5 spec documents covering identity, security, content, transport, and threading"
affects: [02-agent-native, 03-publication]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Thread-scoped presence: participants have independent presence per thread"
    - "One-level sub-threading with error -32030 for nested attempts"
    - "Minimal context inheritance: lineage yes, messages and presence no"
    - "Uniform participant interface regardless of human/agent type"

key-files:
  created:
    - spec/threading.md
  modified: []

key-decisions:
  - "One level of sub-threading only -- prevents Matrix-style complexity explosion from deep nesting"
  - "Minimal context inheritance: sub-threads inherit lineage and security context but NOT parent messages or presence"
  - "Presence is thread-scoped: a participant has independent presence per thread, preventing global-busy problem"
  - "Error code -32031 is overloaded for both already-a-participant and invalid-presence-state -- context distinguishes"
  - "Sub-thread creator must be a participant of the parent thread"
  - "Thread creator role is informational only -- equal messaging capabilities for creator and member roles"

patterns-established:
  - "Thread metadata structure with participants array including identity, type, joined_at, role"
  - "Agent presence lifecycle: available -> processing (with detail) -> available"
  - "Cross-spec integration pattern: threading.md integrates all 4 prior specs as the conversation layer"

requirements-completed: [THRD-01, THRD-02, THRD-03, THRD-04, MIXD-01, MIXD-02, MIXD-03, MIXD-04]

# Metrics
duration: 5min
completed: 2026-03-28
---

# Phase 01 Plan 03: Threading Specification Summary

**Thread model with flat conversations, one-level sub-threading, mixed human-agent participation with adapted presence states, and uniform @-mentions across participant types**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-28T08:35:23Z
- **Completed:** 2026-03-28T08:40:24Z
- **Tasks:** 2 (1 auto + 1 auto-approved checkpoint)
- **Files created:** 1

## Accomplishments
- Wrote complete threading specification (1091 lines) defining flat thread model, sub-threading, context inheritance, mixed human-agent participation, presence semantics, and @-mentions
- Covered all 8 requirements (THRD-01 through THRD-04, MIXD-01 through MIXD-04) with normative prose and concrete JSON examples
- Included 32 JSON code blocks and 2 Mermaid sequence diagrams (thread lifecycle and agent presence state transitions)
- Cross-referenced all four prior specs (identity.md, security.md, content.md, transport.md) completing the Phase 1 protocol kernel
- Auto-approved checkpoint: all 5 spec files verified present and internally consistent

## Task Commits

Each task was committed atomically:

1. **Task 1: Write Threading Primitives** - `3718e68` (feat)
2. **Task 2: Verify Complete Protocol Kernel** - auto-approved (checkpoint:human-verify)

## Files Created/Modified
- `spec/threading.md` - AINM threading specification: flat thread model (THRD-01), one-level sub-threading (THRD-02, THRD-04), minimal context inheritance (THRD-03), mixed human-agent participation (MIXD-01, MIXD-02), adapted presence states (MIXD-03), uniform @-mentions (MIXD-04)

## Decisions Made
- **One-level sub-threading:** Limited sub-threads to one nesting level to prevent complexity explosion. Sub-threads can only branch from root threads. Nested sub-thread attempts return error -32030.
- **Minimal context inheritance:** Sub-threads inherit thread lineage (parent_thread_id, parent_message_id) and security context (same identity keys), but NOT parent messages or presence. Keeps the model simple.
- **Thread-scoped presence:** Presence is independent per-thread. An agent can be `processing` in one thread and `available` in another, preventing the "global busy" problem.
- **Error code overloading:** Used -32031 for both "already a participant" (during thread.join) and "invalid presence state for participant type" (during presence.update). Context in the error data field distinguishes the two.
- **Creator role is informational:** Thread creator and member roles have equal messaging capabilities. No special permissions attached to the creator role.
- **Sub-thread creator must be parent participant:** Enforced that the sub-thread creator is already a member of the parent thread.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - all sections are fully specified with normative content and concrete examples.

## Next Phase Readiness
- Phase 1 protocol kernel is complete: 5 spec documents (identity.md, security.md, content.md, transport.md, threading.md) form a coherent, internally consistent specification
- All 25 Phase 1 requirements (IDNT-01-05, SECU-01-05, TRNS-01-05, CONT-01-02, THRD-01-04, MIXD-01-04) are covered
- The spec set is ready for Phase 2 (agent-native features: capability discovery, streaming, rich UI, memory) and Phase 3 (publication: JSON Schema, AsyncAPI, Docusaurus site)
- Thread operations in threading.md align with JSON-RPC methods in transport.md
- Presence states in threading.md align with ainm.presence.update/notify in transport.md
- Mention format in threading.md aligns with TextBlock mentions field in content.md

## Self-Check: PASSED

- spec/threading.md: FOUND
- 01-03-SUMMARY.md: FOUND
- commit 3718e68: FOUND

---
*Phase: 01-protocol-kernel*
*Completed: 2026-03-28*
