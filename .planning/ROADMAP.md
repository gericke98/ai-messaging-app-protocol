# Roadmap: AI-Native Messaging Protocol

## Overview

This roadmap delivers the v1 protocol specification and working demo in three phases. Phase 1 defines the protocol kernel -- the foundational primitives (identity, security, transport, content, threading, mixed threads) that make it a messaging protocol. Phase 2 extends the kernel with the agent-native features that differentiate it from every existing protocol (streaming, rich interactive content, agent discovery, tool visibility, memory, ecosystem compatibility). Phase 3 formalizes the complete specification with machine-readable schemas, publishes it, and proves the protocol end-to-end with a working demo: at minimum one human and one agent in a thread with streaming and rich content.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Protocol Kernel** - Define the foundational protocol: identity, security, transport, content format, threading, and mixed human-agent threads
- [ ] **Phase 2: Agent-Native Features** - Extend the protocol with streaming, rich interactive content, agent discovery, tool visibility, memory management, and MCP/A2A compatibility
- [ ] **Phase 3: Specification Publication & Working Demo** - Formalize the spec with JSON Schema and AsyncAPI, publish it, and build a reference server demo proving end-to-end operation

## Phase Details

### Phase 1: Protocol Kernel
**Goal**: A complete specification of the foundational messaging primitives -- anyone reading the Phase 1 output can understand how identities work, how messages are signed and transported, what content blocks look like, and how threads (including mixed human-agent threads) behave
**Depends on**: Nothing (first phase)
**Requirements**: IDNT-01, IDNT-02, IDNT-03, IDNT-04, IDNT-05, SECU-01, SECU-02, SECU-03, SECU-04, SECU-05, TRNS-01, TRNS-02, TRNS-03, TRNS-04, TRNS-05, CONT-01, CONT-02, THRD-01, THRD-02, THRD-03, THRD-04, MIXD-01, MIXD-02, MIXD-03, MIXD-04
**Success Criteria** (what must be TRUE):
  1. A developer can read the identity spec and register a `name@provider` address with a bound Ed25519 key pair, with no ambiguity about format, resolution, or key rotation
  2. A developer can read the security spec and produce a canonically-signed message (RFC 8785 + Ed25519) that another implementation can independently verify, including trust annotations distinguishing user/agent/system content
  3. A developer can read the transport spec and implement a WebSocket client-to-server connection and an HTTP server-to-server relay, both using JSON-RPC 2.0 framing over TLS 1.3, with dual delivery modes (streaming and store-and-forward)
  4. A developer can read the content and threading specs and construct a flat conversation with typed content blocks (text/markdown, code, image, file) that supports sub-threading, context inheritance, and mixed human-agent participation with uniform interfaces and adapted presence semantics
**Plans**: 3 plans

Plans:
- [x] 01-01-PLAN.md -- Identity and Security specifications (address format, .well-known resolution, Ed25519 signing, trust annotations)
- [x] 01-02-PLAN.md -- Content and Transport specifications (typed content blocks, WebSocket/HTTP bindings, JSON-RPC lifecycle)
- [x] 01-03-PLAN.md -- Threading specification with mixed participation (flat threads, sub-threads, presence, mentions)

### Phase 2: Agent-Native Features
**Goal**: The protocol specification is extended with every feature that makes it AI-native -- streaming responses, rich interactive UI, agent capability discovery, tool-use transparency, hierarchical memory, and bridges to MCP and A2A
**Depends on**: Phase 1
**Requirements**: STRM-01, STRM-02, STRM-03, STRM-04, CONT-03, CONT-04, CONT-05, DISC-01, DISC-02, DISC-03, DISC-04, DISC-05, TOOL-01, TOOL-02, TOOL-03, TOOL-04, TOOL-05, MEMO-01, MEMO-02, MEMO-03, MEMO-04, MEMO-05, MEMO-06, COMP-01, COMP-02, COMP-03, COMP-04
**Success Criteria** (what must be TRUE):
  1. A developer can read the streaming spec and implement token-by-token agent responses with a defined lifecycle (start, delta, end) that collapse into permanent events, and a client can render partial messages during streaming
  2. A developer can read the content and discovery specs and build an agent that publishes a capability card at a well-known endpoint (A2A-compatible), and sends interactive content (forms, buttons, tables, charts, approvals) using the declarative component model -- no arbitrary code, just data referencing a client component catalog
  3. A developer can read the tool visibility spec and implement events (ToolCallStarted, ToolCallProgress, ToolCallCompleted) that are visible to all thread participants and renderable as collapsible activity indicators
  4. A developer can read the memory spec and implement three-tier context management (active context, conversation memory, archival memory) that is conversation-scoped, shared across participants, and delivers structured context payloads to agents
  5. A developer can read the compatibility spec and map MCP tool calls to protocol message types (exposing an MCP server as a thread participant) and map A2A task lifecycle to thread message events
**Plans**: 5 plans

Plans:
- [x] 02-01-PLAN.md -- Streaming specification (start/delta/end lifecycle, parallel multi-block, cancellation, collapse semantics)
- [x] 02-02-PLAN.md -- Interactive content specification (ButtonBlock, FormBlock, TableBlock, ApprovalBlock, ActionBlock)
- [x] 02-03-PLAN.md -- Agent Card discovery and Tool Visibility specifications (A2A-compatible cards, tool event lifecycle)
- [x] 02-04-PLAN.md -- Memory management specification (three-tier memory, context frames, store/query/evict)
- [x] 02-05-PLAN.md -- Compatibility bridges (MCP/A2A mapping) and Phase 1 spec updates (transport + threading extensions)

### Phase 3: Specification Publication & Working Demo
**Goal**: The complete protocol specification is published as an open document with formal schemas, and a working reference server demonstrates the protocol end-to-end
**Depends on**: Phase 2
**Requirements**: SPEC-01, SPEC-02, SPEC-03, SPEC-04, SPEC-05
**Success Criteria** (what must be TRUE):
  1. The protocol spec is published as an open document (Markdown + JSON Schema + AsyncAPI) with formal definitions for every message type, AsyncAPI v3.1 transport bindings, and clear versioning with backwards compatibility rules
  2. A working demo runs end-to-end: at minimum one human and one agent communicate in a thread, with the agent streaming responses in real-time and sending at least one rich interactive content block -- all messages cryptographically signed and verifiable
  3. An external developer can clone the demo repo, run it locally, and observe the full protocol in action (identity registration, message signing, thread creation, streaming, rich content, agent discovery) without needing to read the spec first
**Plans**: TBD

Plans:
- [ ] 03-01: TBD
- [ ] 03-02: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Protocol Kernel | 3/3 | Complete | 2026-03-28 |
| 2. Agent-Native Features | 0/5 | Planning complete | - |
| 3. Spec Publication & Demo | 0/2 | Not started | - |
