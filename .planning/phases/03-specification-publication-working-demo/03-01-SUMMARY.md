---
phase: 03-specification-publication-working-demo
plan: 01
subsystem: spec
tags: [json-schema, json-schema-2020-12, protocol-schemas, ainm]

# Dependency graph
requires:
  - phase: 01-protocol-kernel
    provides: spec/identity.md, spec/security.md, spec/content.md, spec/transport.md with inline JSON examples
  - phase: 02-agent-native-features
    provides: Phase 2 capability extensions referenced in transport capabilities
provides:
  - schemas/common.schema.json with 11 shared $defs for cross-file $ref
  - schemas/identity.schema.json with 7 identity domain definitions
  - schemas/security.schema.json with 7 security domain definitions
  - schemas/content.schema.json with 6 content block definitions
  - schemas/transport.schema.json with 21 transport domain definitions
affects: [03-02 (AsyncAPI $refs schema files), 03-03 (Docusaurus schema pages), 03-05 (demo validates against schemas)]

# Tech tracking
tech-stack:
  added: [JSON Schema 2020-12]
  patterns: [per-domain schema bundles mirroring spec/ structure, common.schema.json for shared $defs, relative $ref linking]

key-files:
  created:
    - schemas/common.schema.json
    - schemas/identity.schema.json
    - schemas/security.schema.json
    - schemas/content.schema.json
    - schemas/transport.schema.json
  modified: []

key-decisions:
  - "RegistrationResponse references IdentityRecord via $ref rather than duplicating fields"
  - "Error codes documented as const arrays within descriptive wrapper objects for machine readability"
  - "ExtensionBlock type pattern uses full reverse-domain regex rather than simple prefix check"
  - "Transport schema includes relay params for server-to-server federation alongside client-server methods"

patterns-established:
  - "Schema $id convention: https://ainm.dev/schemas/{domain}.schema.json"
  - "Cross-file $ref convention: relative path common.schema.json#/$defs/{TypeName}"
  - "Per-domain bundle pattern: one .schema.json per spec/ markdown file"

requirements-completed: [SPEC-01, SPEC-02]

# Metrics
duration: 5min
completed: 2026-03-28
---

# Phase 3 Plan 1: JSON Schema Foundation Summary

**52 JSON Schema 2020-12 definitions across 5 per-domain schema files formalizing all AINM protocol types from inline spec examples**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-28T20:52:26Z
- **Completed:** 2026-03-28T20:57:42Z
- **Tasks:** 2
- **Files created:** 5

## Accomplishments
- Created common.schema.json with 11 shared definitions ($defs) referenced by all domain schemas: AINMAddress, UUIDv7, RFC3339Timestamp, Ed25519Signature, Ed25519PublicKey, ContentBlockBase, MessageEnvelope, JsonRpcRequest, JsonRpcResponse, JsonRpcError, JsonRpcNotification
- Created 4 domain schemas (identity, security, content, transport) with 41 combined definitions covering all protocol types from Phase 1 and Phase 2 specs
- All schemas use relative $ref to common.schema.json for shared types, establishing the cross-file linking pattern for remaining domain schemas (interactive, streaming, etc.)
- Schema field names and constraints derived directly from inline JSON examples in spec files -- no invented fields

## Task Commits

Each task was committed atomically:

1. **Task 1: Create common.schema.json with all shared definitions** - `473ed7f` (feat)
2. **Task 2: Create identity, security, content, and transport domain schemas** - `c8e74cb` (feat)

## Files Created/Modified
- `schemas/common.schema.json` - 11 shared $defs for AINMAddress, UUIDv7, RFC3339Timestamp, Ed25519 types, ContentBlockBase, MessageEnvelope, JSON-RPC framing types
- `schemas/identity.schema.json` - 7 defs: IdentityRecord, RegistrationRequest/Response, KeyRotationRecord/Request, WellKnownAINMConfig, IdentityErrorCodes
- `schemas/security.schema.json` - 7 defs: SignedMessage, CanonicalSigningInput, TrustAnnotation, VerificationResult, DeduplicationPolicy, SecurityErrorCodes, ReplayProtection
- `schemas/content.schema.json` - 6 defs: TextBlock, MentionAnnotation, CodeBlock, ImageBlock, FileBlock, ExtensionBlock (each with allOf ContentBlockBase)
- `schemas/transport.schema.json` - 21 defs: InitializeParams/Result, Client/ServerCapabilities, MemoryCapabilities, MessageSend/Deliver, ThreadCreate/Join/Leave params/results, PresenceUpdate, Ping, Shutdown, Relay params/result, TransportErrorCodes

## Decisions Made
- RegistrationResponse references IdentityRecord via $ref to avoid field duplication
- Error codes represented as const arrays in descriptive wrapper objects for both documentation and machine readability
- ExtensionBlock type pattern uses full reverse-domain regex (`^[a-z][a-z0-9-]*\.[a-z][a-z0-9-]*...`) for proper validation
- Transport schema includes relay params alongside client-server methods for completeness

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None - all 52 definitions are complete with proper types, constraints, and descriptions derived from spec files.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All 5 core schema files ready for $ref from AsyncAPI definitions (Plan 03-02)
- Schema files ready for Docusaurus schema explorer pages (Plan 03-03)
- Remaining domain schemas (interactive, streaming, agent-card, memory, tool-visibility, compatibility) to be created in Plan 03-02
- Demo (Plan 03-05) can validate messages against these schemas at runtime

## Self-Check: PASSED

- All 5 schema files exist on disk
- Commit 473ed7f (Task 1) found in git log
- Commit c8e74cb (Task 2) found in git log

---
*Phase: 03-specification-publication-working-demo*
*Completed: 2026-03-28*
