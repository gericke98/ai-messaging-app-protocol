---
phase: 02-agent-native-features
plan: 05
subsystem: protocol-spec
tags: [mcp, a2a, compatibility, bridge, proxy-participant, tool-mapping, task-lifecycle]

# Dependency graph
requires:
  - phase: 02-agent-native-features (plans 01-04)
    provides: streaming.md, interactive.md, agent-card.md, tool-visibility.md, memory.md
  - phase: 01-protocol-kernel
    provides: transport.md, threading.md, identity.md, security.md, content.md
provides:
  - MCP compatibility bridge specification (MCP tools as AINM thread participants)
  - A2A compatibility bridge specification (A2A task lifecycle as AINM thread events)
  - Updated transport.md with Phase 2 capability flags, method registry, error code ranges
  - Updated threading.md with Phase 2 message types and thread-scoped event categories
  - Complete 11-file cross-referenced specification set
affects: [phase-03-publication, reference-server, ios-client]

# Tech tracking
tech-stack:
  added: []
  patterns: [proxy-participant-bridge, tool-discovery-mapping, task-lifecycle-mapping, bridge-identity-convention]

key-files:
  created: [spec/compatibility.md]
  modified: [spec/transport.md, spec/threading.md]

key-decisions:
  - "Bridges use proxy participant pattern -- own AINM identity, own Ed25519 keys, own Agent Card"
  - "Bridge identity naming convention: mcp-{name}@ for MCP bridges, a2a-{name}@ for A2A bridges"
  - "No new error code ranges for bridges -- reuse existing transport and tool error codes"
  - "A2A tasks map to sub-threads; input-required maps to FormBlock/ApprovalBlock"
  - "MCP tool names preserved verbatim from MCP server (no prefixing or modification)"
  - "Bridge Agent Cards include ainm.bridgeType and ainm.bridgeTarget extension fields"
  - "Phase 2 error code ranges: -32040..49 streaming, -32050..59 memory, -32060..69 agent-card, -32070..79 interactive"

patterns-established:
  - "Proxy participant pattern: bridge has own identity, translates between AINM and external protocol"
  - "Phase extension pattern: additive sections in Phase 1 specs clearly marked with 'Phase 2 Extensions' heading"
  - "Method registry pattern: central table in transport.md listing all JSON-RPC methods with spec references"
  - "Error code range reservation: each Phase 2 spec owns a 10-code range"

requirements-completed: [COMP-01, COMP-02, COMP-03, COMP-04]

# Metrics
duration: 8min
completed: 2026-03-28
---

# Phase 02 Plan 05: Compatibility Bridges & Phase 1 Updates Summary

**MCP/A2A compatibility bridges as proxy participants with Phase 1 spec extensions for capability flags, method registry, and message types**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-28T19:37:58Z
- **Completed:** 2026-03-28T19:46:01Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Complete MCP bridge specification: tool discovery mapping (tools/list -> ainm.tools), tool invocation lifecycle (tools/call -> tool events), error mapping, Agent Card conventions
- Complete A2A bridge specification: outbound/inbound modes, 6-state task lifecycle mapping, input-required -> FormBlock/ApprovalBlock, artifact mapping, streaming integration
- Phase 1 transport.md updated with 5 new capability flags, 9 new Phase 2 methods, 4 error code ranges
- Phase 1 threading.md updated with 4 new message types and comprehensive thread-scoped event documentation
- All 11 AINM spec files now exist and are internally cross-referenced

## Task Commits

Each task was committed atomically:

1. **Task 1: Write Compatibility Bridge Specification** - `e6e83a2` (feat)
2. **Task 2: Update Phase 1 Specs with Phase 2 Extensions** - `1836651` (feat)

## Files Created/Modified
- `spec/compatibility.md` - New 857-line MCP/A2A bridge specification with proxy participant architecture, tool/task mappings, identity/security model, and examples
- `spec/transport.md` - Phase 2 capability flags (parallelStreaming, toolVisibility, memory.*), method registry (9 methods), error code ranges (-32040 to -32079), negotiation example
- `spec/threading.md` - Phase 2 message type registry (tool.call.started/progress/completed, action), combined type table, thread-scoped event categories documentation

## Decisions Made
- Bridges use the proxy participant pattern with their own AINM identities and Ed25519 keys, ensuring all bridge messages are attributable and verifiable
- MCP tool names are preserved verbatim (no prefixing) for maximum compatibility with existing MCP tool ecosystems
- A2A tasks are mapped to sub-threads, leveraging the existing one-level nesting model from threading.md
- No new error codes for bridges -- they reuse existing transport and tool-visibility error codes, avoiding error code sprawl
- Bridge Agent Cards include ainm.bridgeType and ainm.bridgeTarget fields in the ainm extension object for provenance display
- Phase 2 error code ranges are reserved in blocks of 10, with each Phase 2 spec owning its range
- Phase 1 spec updates are strictly additive -- no restructuring or deletion of existing content

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - all spec sections are fully written with no placeholder content.

## Next Phase Readiness
- The complete 11-file specification set is ready for Phase 3 (publication and demo)
- All Phase 2 specs are cross-referenced and internally consistent
- The compatibility bridge spec provides the integration layer connecting all Phase 2 features
- Transport and threading specs now serve as the complete method and message type registries for the full protocol

---
*Phase: 02-agent-native-features*
*Completed: 2026-03-28*
