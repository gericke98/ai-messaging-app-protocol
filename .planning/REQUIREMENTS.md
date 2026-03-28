# Requirements: AI-Native Messaging Protocol

**Defined:** 2026-03-28
**Core Value:** Enable mixed human-agent threads where any participant (human or agent) can seamlessly join, communicate, and coordinate with the same quality of experience.

## v1 Requirements

Requirements for the initial protocol specification and working demo.

### Transport

- [ ] **TRNS-01**: Protocol defines WebSocket transport binding for client-to-server real-time communication
- [ ] **TRNS-02**: Protocol defines HTTP transport binding for server-to-server message relay
- [ ] **TRNS-03**: All transport bindings require TLS 1.3 minimum
- [ ] **TRNS-04**: Protocol uses JSON-RPC 2.0 as the message framing format
- [ ] **TRNS-05**: Protocol supports dual delivery modes: streaming (foreground) and store-and-forward (background/offline)

### Identity

- [ ] **IDNT-01**: Humans and agents use email-like addresses (`name@provider`)
- [ ] **IDNT-02**: Identity format is uniform — no distinction between human and agent addresses at the protocol level
- [ ] **IDNT-03**: Identity is cryptographically bound to a signing key pair
- [ ] **IDNT-04**: Protocol defines identity registration and resolution via the `@provider` portion
- [ ] **IDNT-05**: Protocol defines key rotation mechanism for identities

### Security

- [ ] **SECU-01**: Every message is signed with Ed25519 by the sender
- [ ] **SECU-02**: Signatures use a canonical signing format (deterministic field ordering per RFC 8785)
- [ ] **SECU-03**: Receivers can verify message authenticity and integrity via signature validation
- [ ] **SECU-04**: Protocol defines trust annotations on message content blocks (user-generated vs agent-generated vs system)
- [ ] **SECU-05**: Content and instructions are structurally separated at the protocol message level (anti-prompt-injection)

### Streaming

- [ ] **STRM-01**: Agents can stream text responses token-by-token in real-time
- [ ] **STRM-02**: Protocol defines streaming lifecycle events (stream-start, content-delta, stream-end)
- [ ] **STRM-03**: Streaming messages collapse into permanent events upon completion
- [ ] **STRM-04**: Clients can receive and render partial messages during streaming

### Content

- [ ] **CONT-01**: Messages are composed of typed content blocks with declared schemas
- [ ] **CONT-02**: Protocol defines core block types: TextBlock (markdown), CodeBlock (language, content), ImageBlock, FileBlock (MIME type, digest)
- [ ] **CONT-03**: Protocol defines interactive block types: ButtonBlock, FormBlock, TableBlock, ChartBlock, ApprovalBlock
- [ ] **CONT-04**: Interactive blocks use declarative component model (UI as data, not code) — agents reference a client-controlled component catalog
- [ ] **CONT-05**: No arbitrary payload size limits at the protocol level

### Threading

- [ ] **THRD-01**: Conversations are flat by default (single stream of messages)
- [ ] **THRD-02**: Any participant can create a sub-thread from any message
- [ ] **THRD-03**: Sub-threads inherit parent thread context
- [ ] **THRD-04**: Agents can create sub-threads for side conversations (e.g., agent-to-agent coordination)

### Mixed Threads

- [ ] **MIXD-01**: Threads support any combination of human and agent participants as equals
- [ ] **MIXD-02**: Thread participant interface is uniform regardless of participant type
- [ ] **MIXD-03**: Presence semantics adapt per participant type (humans: online/offline; agents: available/busy/processing)
- [ ] **MIXD-04**: @-mentions work across participant types (humans can @-mention agents and vice versa)

### Agent Discovery

- [ ] **DISC-01**: Agents publish capability cards (Agent Cards) as JSON at a well-known endpoint
- [ ] **DISC-02**: Agent Cards are discoverable by resolving the `@provider` portion of an agent's identity
- [ ] **DISC-03**: Agent Cards include: name, description, supported content types, streaming capabilities, threading preferences, tools available
- [ ] **DISC-04**: Agent Cards are compatible with A2A Agent Card format (extend, don't reinvent)
- [ ] **DISC-05**: Agent Cards are cacheable with explicit versioning

### Tool Visibility

- [ ] **TOOL-01**: Protocol defines ToolCallStarted event (tool name, description)
- [ ] **TOOL-02**: Protocol defines ToolCallProgress event (intermediate results)
- [ ] **TOOL-03**: Protocol defines ToolCallCompleted event (result summary)
- [ ] **TOOL-04**: Tool events are visible to all thread participants
- [ ] **TOOL-05**: Clients render tool events as collapsible activity indicators

### Memory

- [ ] **MEMO-01**: Protocol defines three memory tiers: active context, conversation memory, archival memory
- [ ] **MEMO-02**: Active context includes current thread + recent messages, always available to agents
- [ ] **MEMO-03**: Conversation memory is the full thread history, queryable by participants
- [ ] **MEMO-04**: Archival memory stores cross-conversation knowledge, user preferences, long-term facts
- [ ] **MEMO-05**: Agents receive structured context payloads, not raw message dumps
- [ ] **MEMO-06**: Memory is conversation-scoped and shared across all participants in a thread

### Compatibility

- [ ] **COMP-01**: Protocol defines mapping from MCP tool calls to protocol message types
- [ ] **COMP-02**: An MCP server can be exposed as an agent participant in a thread
- [ ] **COMP-03**: Protocol defines mapping from A2A task lifecycle to thread message events
- [ ] **COMP-04**: A2A task updates can be rendered as thread messages

### Specification

- [ ] **SPEC-01**: Protocol spec is published as an open document (Markdown + JSON Schema + AsyncAPI)
- [ ] **SPEC-02**: All message types have formal JSON Schema definitions
- [ ] **SPEC-03**: Transport bindings have formal AsyncAPI v3.1 definitions
- [ ] **SPEC-04**: Spec is versioned with clear backwards compatibility rules
- [ ] **SPEC-05**: Working demo proves the protocol end-to-end (at minimum: 1 human + 1 agent in a thread with streaming and rich content)

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Federation

- **FEDR-01**: Full multi-server message routing with push-based federation model
- **FEDR-02**: Origin-server authority for thread state (not Matrix DAG)
- **FEDR-03**: Server-to-server mutual TLS authentication
- **FEDR-04**: Selective backfill for thread history across servers
- **FEDR-05**: Offline message relay queues

### Encryption

- **ENCR-01**: End-to-end encryption for human-to-human messages
- **ENCR-02**: Defined trust model for agent access to encrypted content
- **ENCR-03**: Key exchange and group key management for encrypted threads

### Advanced Agent Features

- **ADVN-01**: Human-in-the-loop interrupts and approval workflows
- **ADVN-02**: Multi-agent coordination primitives (handoffs, task delegation, delegation chains)
- **ADVN-03**: Conversation-level permissions with fine-grained scoping
- **ADVN-04**: Event sourcing and complete audit trail

### iOS Reference Client

- **IOSC-01**: Native SwiftUI iOS app implementing the protocol
- **IOSC-02**: GRDB.swift local storage for offline message access
- **IOSC-03**: Apple CryptoKit for Ed25519 signing
- **IOSC-04**: APNs integration for background message delivery
- **IOSC-05**: Native rendering of all content block types

## Out of Scope

| Feature | Reason |
|---------|--------|
| Arbitrary code execution in messages | Security — UI as data, not code (A2UI principle) |
| Payment / transaction layer | Regulatory complexity, not core to messaging |
| Agent marketplace / store | Contradicts federation — let third parties build directories |
| Voice / video messaging | Enormous complexity (WebRTC), agents are text-first |
| Message editing after delivery | Conflicts with event-sourced audit trail integrity |
| Server-side content processing | Servers are relays, not processors — enables E2E encryption |
| Unnamespaced protocol extensions | XMPP's 400+ XEPs proved uncontrolled extensions fragment ecosystems |
| Android / web clients for v1 | iOS reference client only — other platforms later |
| Self-hosting documentation | Protocol is open but easy deployment tooling is v2+ |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| TRNS-01 | — | Pending |
| TRNS-02 | — | Pending |
| TRNS-03 | — | Pending |
| TRNS-04 | — | Pending |
| TRNS-05 | — | Pending |
| IDNT-01 | — | Pending |
| IDNT-02 | — | Pending |
| IDNT-03 | — | Pending |
| IDNT-04 | — | Pending |
| IDNT-05 | — | Pending |
| SECU-01 | — | Pending |
| SECU-02 | — | Pending |
| SECU-03 | — | Pending |
| SECU-04 | — | Pending |
| SECU-05 | — | Pending |
| STRM-01 | — | Pending |
| STRM-02 | — | Pending |
| STRM-03 | — | Pending |
| STRM-04 | — | Pending |
| CONT-01 | — | Pending |
| CONT-02 | — | Pending |
| CONT-03 | — | Pending |
| CONT-04 | — | Pending |
| CONT-05 | — | Pending |
| THRD-01 | — | Pending |
| THRD-02 | — | Pending |
| THRD-03 | — | Pending |
| THRD-04 | — | Pending |
| MIXD-01 | — | Pending |
| MIXD-02 | — | Pending |
| MIXD-03 | — | Pending |
| MIXD-04 | — | Pending |
| DISC-01 | — | Pending |
| DISC-02 | — | Pending |
| DISC-03 | — | Pending |
| DISC-04 | — | Pending |
| DISC-05 | — | Pending |
| TOOL-01 | — | Pending |
| TOOL-02 | — | Pending |
| TOOL-03 | — | Pending |
| TOOL-04 | — | Pending |
| TOOL-05 | — | Pending |
| MEMO-01 | — | Pending |
| MEMO-02 | — | Pending |
| MEMO-03 | — | Pending |
| MEMO-04 | — | Pending |
| MEMO-05 | — | Pending |
| MEMO-06 | — | Pending |
| COMP-01 | — | Pending |
| COMP-02 | — | Pending |
| COMP-03 | — | Pending |
| COMP-04 | — | Pending |
| SPEC-01 | — | Pending |
| SPEC-02 | — | Pending |
| SPEC-03 | — | Pending |
| SPEC-04 | — | Pending |
| SPEC-05 | — | Pending |

**Coverage:**
- v1 requirements: 51 total
- Mapped to phases: 0
- Unmapped: 51

---
*Requirements defined: 2026-03-28*
*Last updated: 2026-03-28 after initial definition*
