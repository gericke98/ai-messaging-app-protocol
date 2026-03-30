# Phase 10: Foundation - Context

**Gathered:** 2026-03-29
**Status:** Ready for planning

<domain>
## Phase Boundary

App connects to the AINM Rust server, completes the protocol handshake, and exchanges Ed25519-signed messages with verified JCS canonicalization — all infrastructure proven before any UI beyond onboarding. Requirements: NET-01 through NET-06, PLT-01.

</domain>

<decisions>
## Implementation Decisions

### Onboarding Flow
- **D-01:** Single screen with server URL field, display name field, and Connect button. Keypair generated silently on connect.
- **D-02:** No connection status indicator in Phase 10 — defer polished status UI to Phase 11.

### Claude's Discretion (Onboarding)
- Error handling on failed connection (inline error vs alert sheet — Claude picks best SwiftUI pattern)
- Whether to pre-fill demo server URL (Claude decides based on target audience)

### Project Structure
- **D-03:** Monolith app target with folder-based layers: App/, Networking/, Crypto/, Persistence/, UI/, Models/. Single Xcode project, no local SPM packages.
- **D-04:** iOS project lives at `ios/AINMMessenger/` relative to repo root, alongside `demo/`, `spec/`, `schemas/`.
- **D-05:** Swift 6 strict concurrency mode enabled from day one in build settings.

### Reconnection & Error UX
- **D-06:** Auto-reconnect with exponential backoff (1s, 2s, 4s, 8s... capped at 30s). Silent reconnection with subtle status — no user action needed for transient failures.
- **D-07:** Always full re-handshake on reconnect (ainm.initialize / ainm.initialized). Stateless — server may have restarted.

### Integration Testing
- **D-08:** Shared test vectors (JSON files) for cross-platform crypto validation. Rust generates vectors, Swift validates against them. No live server needed for unit tests.
- **D-09:** Test vectors live at `tests/vectors/` at repo root — shared across Rust and Swift projects.
- **D-10:** Basic GitHub Actions CI in Phase 10 — builds iOS project and runs unit tests on every push (macos-14 runner, Xcode 16).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Protocol Specification
- `spec/transport.md` — WebSocket transport, JSON-RPC 2.0 framing, ainm.initialize/ainm.initialized handshake
- `spec/security.md` — Ed25519 signing, RFC 8785 JCS canonicalization, message integrity
- `spec/identity.md` — Email-like identity model, identity resolution, public key publishing

### Schemas
- `schemas/transport.schema.json` — JSON Schema for transport messages (initialize, initialized)
- `schemas/security.schema.json` — JSON Schema for signed message envelopes
- `schemas/identity.schema.json` — JSON Schema for identity objects
- `schemas/common.schema.json` — Shared type definitions
- `schemas/threading.schema.json` — Thread and message schema (for GRDB persistence modeling)
- `schemas/content.schema.json` — Content block schemas (for GRDB persistence modeling)

### AsyncAPI
- `asyncapi.yaml` — Transport bindings, channel definitions, operation specs

### Existing Rust Implementation (integration target)
- `demo/src/signing.rs` — Rust Ed25519 signing + JCS implementation (reference for cross-platform compatibility)
- `demo/src/server.rs` — WebSocket server + handshake handling
- `demo/src/protocol.rs` — JSON-RPC message types and routing
- `demo/src/persistence.rs` — Server-side persistence (reference for data modeling)
- `demo/src/identity.rs` — Identity management on server side

### Technology Stack
- `CLAUDE.md` §iOS Client Stack — SwiftUI, GRDB.swift, Apple CryptoKit, KeychainAccess decisions

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- No existing Swift code — this is a greenfield iOS project
- Rust demo server (`demo/src/`, 11 source files) is the integration target
- 12 JSON Schema files define all protocol types — can inform Swift Codable models
- AsyncAPI definition (`asyncapi.yaml`) documents all WebSocket operations

### Established Patterns
- Rust server uses Ed25519 via `ring` crate with RFC 8785 JCS — Swift must match exactly
- JSON-RPC 2.0 envelope pattern throughout: `{"jsonrpc":"2.0","method":"...","params":{...},"id":...}`
- Server handshake: client sends `ainm.initialize` with capabilities, server responds `ainm.initialized`

### Integration Points
- iOS app connects to `demo/` Rust server via WebSocket
- Shared test vectors at `tests/vectors/` bridge Rust and Swift test suites
- JSON Schemas at `schemas/` can be used to generate or validate Swift model structs

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 10-foundation*
*Context gathered: 2026-03-29*
