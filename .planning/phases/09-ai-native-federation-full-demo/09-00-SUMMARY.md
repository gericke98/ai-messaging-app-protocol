---
phase: 09-ai-native-federation-full-demo
plan: 00
subsystem: protocol-spec
tags: [json-rpc, federation, context-push, json-schema, asyncapi]

requires:
  - phase: 08-security-hardening-reliability-backfill
    provides: "Backfill spec pattern in transport.md Section 5.5 and transport.schema.json"
provides:
  - "ainm.context-push method specification in transport.md Section 5.6"
  - "ContextPushParams, ContextFrame, ContextPushResult JSON Schema definitions"
  - "sendContextPush AsyncAPI operation with federation channel messages"
affects: [09-02-context-push-implementation]

tech-stack:
  added: []
  patterns: ["context-push follows same relay endpoint pattern as ainm.backfill"]

key-files:
  created: []
  modified:
    - spec/transport.md
    - schemas/transport.schema.json
    - asyncapi.yaml

key-decisions:
  - "Context push uses Layer 2 validation only (server signature) -- context frames are server-originated operational data, not participant messages"
  - "Only active_context frame type is supported for context-push; conversation history and archival data use ainm.backfill"

patterns-established:
  - "Federation method specification pattern: prose spec section + JSON Schema params/result + AsyncAPI operation/messages"

requirements-completed: [AINF-03]

duration: 3min
completed: 2026-03-29
---

# Phase 9 Plan 0: Context Push Protocol Specification Summary

**Specified ainm.context-push JSON-RPC method for pushing active context frames from origin to remote servers during federated thread participation**

## What Was Done

### Task 1: Specify ainm.context-push in transport.md Section 5.6
- Added Section 5.6 after existing Backfill section (5.5)
- Defined full JSON-RPC 2.0 request/response format with example payloads
- Documented parameters: thread_id, context_frames (array of frames), from_server, server_signature
- Specified validation rules: Layer 2 only (server signature), no per-message signatures
- Defined authorization: pushing server must be origin server for the thread
- Documented triggering conditions: remote server first participation, new memory stored

### Task 2: Add schema definitions and AsyncAPI bindings
- Added ContextPushParams, ContextFrame, and ContextPushResult to transport.schema.json
- Added contextPushRequest/Response messages to federation channel in asyncapi.yaml
- Added sendContextPush operation with reply pattern
- Added ContextPushRequest and ContextPushResponse component message definitions

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | ea5c58c | feat(09-00): specify ainm.context-push in transport.md Section 5.6 |
| 2 | c96985d | feat(09-00): add ContextPushParams/Result schema and context-push to AsyncAPI |

## Deviations from Plan

None -- plan executed exactly as written.

## Known Stubs

None.

## Self-Check: PASSED
