# Changelog

All notable changes to the AI-Native Messaging Protocol will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

**Version Policy:** During the 0.x series, breaking changes may occur in any release but will
always be documented here. After 1.0, breaking changes require a major version bump.

**Breaking change examples (require major bump after 1.0):**
- Field removal or rename in message schemas
- Type changes on existing fields
- Removal of a message type or JSON-RPC method
- Changes to signing rules or canonical format

**Non-breaking change examples:**
- Adding optional fields to existing schemas
- New message types or content block types
- New JSON-RPC methods
- New error codes in reserved ranges

## [0.1.0] - 2026-03-28

Initial working draft of the AI-Native Messaging Protocol.

### Added

#### Protocol Kernel (Phase 1)
- Identity model: email-like `name@provider` addresses with Ed25519 key binding
- Identity registration, resolution via `.well-known/ainm/` endpoints, and key rotation with 24-hour overlap
- Security: Ed25519 message signing with RFC 8785 canonical JSON serialization
- Trust annotations (user/agent/system) with structural content/instruction separation
- Transport: WebSocket client-to-server binding with JSON-RPC 2.0 framing over TLS 1.3
- Transport: HTTP server-to-server federation relay with mutual TLS
- Connection lifecycle: initialize/initialized/shutdown handshake with capability negotiation
- Dual delivery modes: real-time streaming and store-and-forward for offline participants
- Content block type system: TextBlock (markdown), CodeBlock, ImageBlock, FileBlock
- Extensibility: reverse-domain namespaced extension blocks with mandatory fallback
- Threading: flat conversation model with UUIDv7 thread IDs
- Sub-threading with one-level nesting limit and explicit context inheritance
- Mixed human-agent thread participation with uniform interface
- Thread-scoped presence (online/offline for humans, available/busy/processing for agents)
- @-mention system working across participant types

#### Agent-Native Features (Phase 2)
- Streaming: token-by-token delivery with Start/Delta/End lifecycle
- Parallel multi-block streaming with block index addressing
- Stream cancellation and collapse into permanent signed messages
- Stream error handling with partial content preservation
- Interactive content: ButtonBlock, FormBlock (8 field types), TableBlock, ApprovalBlock, ActionBlock
- Declarative component model (UI as data, not code)
- ApprovalBlock server-enforced state machine with immutable terminal states
- Agent Card discovery at `.well-known/ainm/agent-card` (A2A-compatible with AINM extensions)
- Tool visibility events: ToolCallStarted, ToolCallProgress, ToolCallCompleted
- Tool events as persistent signed messages in the thread (cryptographic audit trail)
- Three-tier memory: active context, conversation memory, archival memory
- Context frames with 32KB budget (50/20/20/10 split)
- Memory operations: store, query, evict (private JSON-RPC, not thread messages)
- MCP compatibility bridge: proxy participant pattern for MCP tool servers
- A2A compatibility bridge: task lifecycle mapping to AINM thread events

#### Specification Artifacts (Phase 3)
- 12 JSON Schema 2020-12 files formalizing all message types
- AsyncAPI 3.1.0 transport binding document
- Docusaurus specification website
- Working CLI demo (Rust + Axum) proving end-to-end protocol operation

### JSON-RPC Methods

| Method | Type | Spec |
|--------|------|------|
| ainm.initialize | request | transport.md |
| ainm.initialized | notification | transport.md |
| ainm.shutdown | notification | transport.md |
| ainm.message.send | request | transport.md |
| ainm.message.deliver | notification | transport.md |
| ainm.thread.create | request | threading.md |
| ainm.thread.join | request | threading.md |
| ainm.thread.leave | request | threading.md |
| ainm.sub_thread.create | request | threading.md |
| ainm.presence.update | notification | threading.md |
| ainm.presence.notify | notification | threading.md |
| ainm.ping | request | transport.md |
| ainm.pong | response | transport.md |
| ainm.stream.start | notification | streaming.md |
| ainm.stream.delta | notification | streaming.md |
| ainm.stream.end | notification | streaming.md |
| ainm.stream.cancel | request | streaming.md |
| ainm.stream.error | notification | streaming.md |
| ainm.memory.store | request | memory.md |
| ainm.memory.query | request | memory.md |
| ainm.memory.evict | request | memory.md |
| ainm.context.frame | notification | memory.md |
| ainm.relay | request | transport.md |

### Error Code Ranges

| Range | Domain |
|-------|--------|
| -32001 to -32007 | Identity |
| -32010 to -32013 | Security |
| -32020 to -32029 | Transport |
| -32030 to -32039 | Threading |
| -32040 to -32044 | Streaming |
| -32050 to -32055 | Memory |
| -32060 to -32069 | Agent Card |
| -32070 to -32074 | Interactive Content |
