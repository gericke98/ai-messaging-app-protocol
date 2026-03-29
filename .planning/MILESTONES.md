# Milestones

## v3.1 AI-Native Messaging Protocol v1 Specification & Demo (Shipped: 2026-03-29)

**Phases completed:** 6 phases, 20 plans, 38 tasks

**Key accomplishments:**

- AINM identity (name@provider, .well-known resolution, Ed25519 key binding) and security (RFC 8785 JCS + Ed25519 signing, trust annotations, deduplication) specifications with 3 test vectors
- Typed content block system with four core types and extensible namespacing, plus WebSocket/HTTP transport bindings with JSON-RPC 2.0 framing, connection lifecycle, and dual delivery modes
- Thread model with flat conversations, one-level sub-threading, mixed human-agent participation with adapted presence states, and uniform @-mentions across participant types
- Complete streaming spec defining Start-Delta-End lifecycle with parallel multi-block support, collapse into Ed25519-signed messages, client cancellation, and error handling across 5 JSON-RPC methods
- Declarative interactive content spec with 5 block types (button/form/table/approval/action), 8 form field types, approval state machine, and ActionBlock-as-thread-message response pattern
- A2A-compatible Agent Card schema with AINM extensions for discovery/capabilities, plus three-event tool call lifecycle (started/progress/completed) as transparent thread-scoped signed messages
- Three-tier memory model (active/conversation/archival) with structured context frames, store/query/evict operations, and per-participant archival isolation -- covering MEMO-01 through MEMO-06
- MCP/A2A compatibility bridges as proxy participants with Phase 1 spec extensions for capability flags, method registry, and message types
- 52 JSON Schema 2020-12 definitions across 5 per-domain schema files formalizing all AINM protocol types from inline spec examples
- 7 JSON Schema 2020-12 files formalizing interactive, streaming, threading, agent-card, memory, tool-visibility, and compatibility domains with $ref links to common.schema.json
- AsyncAPI 3.1.0 transport binding with 9 domain channels, 25 operations, 33 schema $refs, plus CHANGELOG.md and VERSION establishing v0.1.0 versioning
- Single-binary Rust CLI demo proving AINM protocol end-to-end: Ed25519 signing, JSON-RPC WebSocket transport, streaming with token-by-token rendering, and interactive block showcase
- Docusaurus 3.9.2 site with 11 spec pages, AsyncAPI explorer, schema downloads, and GitHub Pages CI/CD deployment
- Fixed 3 audit findings: removed invented sub_thread method, added StreamErrorNotification event schema, extended MessageEnvelope.type enum to 9 values
- Fix all demo-to-schema field name mismatches, add Ed25519-signed init handshake, and complete Agent Card with all required ainm sub-objects
- Added /memory and /bridge REPL commands exercising three-tier memory operations (store, query, context frame) and compatibility bridges (MCP tool bridging, A2A task lifecycle) with schema-conformant JSON
- Complete ainm.initialized handshake enforcement in demo -- both clients send notification, server blocks pre-handshake methods with -32021
- Corrected Agent Card URL from inverted agent-card/helper to spec-compliant /.well-known/ainm/helper/agent-card.json in all 4 demo locations, and fixed demo.md hardcoded port
- End-to-end memory eviction via /evict REPL command and Agent Card ETag caching with If-None-Match 304 support

---
