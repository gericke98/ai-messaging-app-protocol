---
phase: 01-protocol-kernel
plan: 02
subsystem: protocol-spec
tags: [content-blocks, transport, websocket, http-relay, json-rpc, federation, tls]

# Dependency graph
requires:
  - phase: 01-protocol-kernel plan 01
    provides: Identity and security spec documents (identity.md, security.md) referenced by content and transport specs
provides:
  - Content block type system with base schema, four core types (text, code, image, file), and extensibility model
  - WebSocket transport binding with full connection lifecycle and all protocol methods
  - HTTP transport binding for server-to-server federation relay with mTLS
  - JSON-RPC 2.0 framing definition for all protocol communication
  - Streaming and store-and-forward delivery modes
affects: [01-protocol-kernel plan 03, 02-agent-native, 03-publication]

# Tech tracking
tech-stack:
  added: []
  patterns: [spec-document-structure, json-rpc-framing, content-block-schema, transport-binding-pattern]

key-files:
  created:
    - spec/content.md
    - spec/transport.md
  modified: []

key-decisions:
  - "Content blocks use mandatory fallback field on every block for graceful degradation"
  - "Extension blocks use reverse-domain namespacing (com.example.weather-card) without a centralized registry"
  - "Block-level version field uses MAJOR.MINOR semantics -- MAJOR breaks backward compat, MINOR adds optional fields"
  - "WebSocket lifecycle follows MCP pattern: initialize -> initialized -> operation -> shutdown"
  - "Server-to-server relay uses triple-layer validation: mTLS + server signature + individual message signatures"
  - "Store-and-forward queue retention SHOULD be at least 7 days with server-configurable policy"

patterns-established:
  - "Spec document structure: Overview, Terminology, core sections, Error Handling, Security Considerations, Examples"
  - "Content block base schema: type + version + source + fallback on every block"
  - "JSON-RPC method naming: ainm.{domain}.{action} hierarchical namespace"
  - "Transport error code ranges: -32010s for security, -32020s for transport, -32030s for threads"

requirements-completed: [CONT-01, CONT-02, TRNS-01, TRNS-02, TRNS-03, TRNS-04, TRNS-05]

# Metrics
duration: 6min
completed: 2026-03-28
---

# Phase 01 Plan 02: Content and Transport Specification Summary

**Typed content block system with four core types and extensible namespacing, plus WebSocket/HTTP transport bindings with JSON-RPC 2.0 framing, connection lifecycle, and dual delivery modes**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-28T08:25:01Z
- **Completed:** 2026-03-28T08:31:37Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Wrote complete content specification defining the typed content block system: base schema (type, version, source, fallback), four core block types (TextBlock, CodeBlock, ImageBlock, FileBlock), extension block system with reverse-domain namespacing, and mandatory fallback rendering for graceful degradation
- Wrote complete transport specification defining WebSocket binding (client-to-server with full lifecycle), HTTP binding (server-to-server federation relay with mTLS), JSON-RPC 2.0 framing, all protocol methods, dual delivery modes (streaming + store-and-forward), and transport-specific error codes
- Both specs include extensive inline JSON examples (9 in content, 35 in transport) following the D-03 design decision

## Task Commits

Each task was committed atomically:

1. **Task 1: Write Content Specification** - `9a5548c` (feat)
2. **Task 2: Write Transport Specification** - `06aa777` (feat)

## Files Created/Modified
- `spec/content.md` - Content block type system: base schema, TextBlock, CodeBlock, ImageBlock, FileBlock, extension blocks, ordering, error handling, security considerations
- `spec/transport.md` - Transport bindings: JSON-RPC 2.0 framing, WebSocket lifecycle (initialize/initialized/shutdown), all protocol methods (message.send/deliver, thread.create/join/leave, presence.update/notify, ping/pong), HTTP relay for federation, streaming and store-and-forward delivery modes

## Decisions Made
- Content blocks use mandatory `fallback` field on every block (core and extension) -- guarantees graceful degradation for any client
- Extension blocks use reverse-domain namespacing without a centralized registry -- collisions resolved by client rendering logic
- Block-level `version` uses MAJOR.MINOR format where MAJOR breaks compat and MINOR adds optional fields
- WebSocket connection lifecycle follows the MCP pattern (initialize -> initialized -> shutdown) with capability negotiation
- Server-to-server federation relay uses triple-layer validation: mTLS certificate + server Ed25519 signature + individual message Ed25519 signatures
- Store-and-forward queue retention is a SHOULD-level recommendation (7 days) with server-configurable policy via capability exchange
- Presence status values are type-adapted: humans get online/offline/away/dnd, agents get available/busy/processing/offline

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- `spec/content.md` and `spec/transport.md` are complete and ready for Plan 03 (threading.md) which will normatively define thread semantics, sub-threading, and mixed participant interactions
- Both specs correctly cross-reference `security.md`, `identity.md`, and `threading.md` which are being written in parallel plans (01 and 03)
- Thread operations (`ainm.thread.create`, `ainm.thread.join`, `ainm.thread.leave`) are transport-defined in transport.md with thread semantics deferred to threading.md

## Self-Check: PASSED

- spec/content.md: FOUND
- spec/transport.md: FOUND
- 01-02-SUMMARY.md: FOUND
- Commit 9a5548c (Task 1): FOUND
- Commit 06aa777 (Task 2): FOUND

---
*Phase: 01-protocol-kernel*
*Completed: 2026-03-28*
