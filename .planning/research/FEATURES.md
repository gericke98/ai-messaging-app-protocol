# Feature Landscape: AI-Native Messaging Protocol

**Domain:** Federated AI-native messaging protocol and reference client
**Researched:** 2026-03-27
**Overall confidence:** HIGH (grounded in existing protocol specs, published research, and OWASP security framework)

---

## Table Stakes

Features users and developers expect. Missing any of these means the protocol feels incomplete, untrustworthy, or not worth adopting over existing alternatives.

### TS-1: Federated Message Routing

| Attribute | Detail |
|-----------|--------|
| **Why Expected** | Federation is the core promise. Without it, this is just another centralized chat API. Matrix and XMPP proved federation is achievable; email proved it scales. |
| **Complexity** | HIGH |
| **Dependencies** | TS-2 (Identity), TS-5 (Server-to-Server Transport) |
| **Existing Coverage** | AMP defines REST + WebSocket + Relay Queue routing. Matrix uses event-based distributed replication. XMPP uses server-to-server XML streams. |
| **How Well** | AMP is closest to our vision but is v0.1.2 with no production federation. Matrix is battle-tested but designed for human chat, not agent communication. Neither handles agent-specific routing needs (capability-based routing, load-aware routing). |
| **Notes** | Must support offline delivery via relay queues. Message ordering guarantees are critical -- WhatsApp's lack of ordering guarantees is a known pain point. |

### TS-2: Email-Like Identity (`name@provider`)

| Attribute | Detail |
|-----------|--------|
| **Why Expected** | Human-readable, familiar, works naturally with federation. Every competing approach (AMP, ANP, ANS) converges on addressable identity. |
| **Complexity** | MEDIUM |
| **Dependencies** | TS-1 (Federation -- identity must be routable across servers) |
| **Existing Coverage** | AMP uses `agent-name@tenant.provider`. ANS uses `a2a://textProcessor.DocumentTranslation.AcmeCorp.v2.1`. ANP uses W3C DIDs. |
| **How Well** | AMP's format is closest. ANS is too verbose for human use. ANP's DIDs are technically superior for decentralization but terrible UX. |
| **Notes** | Both humans and agents get the same identity format. No second-class citizens. Identity must be cryptographically bound (see TS-7). |

### TS-3: Agent Capability Discovery (Agent Cards)

| Attribute | Detail |
|-----------|--------|
| **Why Expected** | Without capability discovery, adding an agent to a conversation is a leap of faith. A2A's Agent Cards (JSON at `/.well-known/agent-card.json`) have become the de facto standard with 50+ enterprise partners. |
| **Complexity** | MEDIUM |
| **Dependencies** | TS-2 (Identity -- need to resolve who to query for capabilities) |
| **Existing Coverage** | A2A Agent Cards (v0.3, digitally signable via JWS). AMP has no explicit capability discovery. MCP has tool/resource declarations per-server. |
| **How Well** | A2A Agent Cards are well-designed and adopted. Must be compatible, not reinvented. Extend with messaging-specific fields (supported content types, threading preferences, streaming capabilities). |
| **Notes** | Agent Cards should be fetchable by resolving the `@provider` portion of an identity. Cards must be cacheable but versioned. |

### TS-4: Streaming Message Delivery

| Attribute | Detail |
|-----------|--------|
| **Why Expected** | Token-by-token streaming is the baseline UX for AI responses since ChatGPT. Every current messaging platform lacks this (Telegram/Slack simulate via message-edit hacks). AG-UI has proven the event model. |
| **Complexity** | MEDIUM |
| **Dependencies** | TS-5 (Transport -- streaming requires persistent connections or SSE) |
| **Existing Coverage** | AG-UI defines ~16 event types including TEXT_MESSAGE_START/CONTENT/END, TOOL_CALL, STEP lifecycle events. A2A supports SSE-based streaming for task updates. A2UI supports progressive JSONL rendering. |
| **How Well** | AG-UI's event model is production-proven (2M+ agent-user interactions/week). Adopt its event taxonomy, adapt transport for federation. |
| **Notes** | Must support: token streaming for text, progress events for tool calls, lifecycle events (run started/step started/finished), and interrupts for human approval. |

### TS-5: Transport Layer (Client-to-Server + Server-to-Server)

| Attribute | Detail |
|-----------|--------|
| **Why Expected** | Protocol is useless without a concrete transport. Must work for native iOS (persistent connections), real-time streaming (SSE/WebSocket), and server-to-server federation (HTTP/gRPC). |
| **Complexity** | HIGH |
| **Dependencies** | None -- foundational layer |
| **Existing Coverage** | AMP: REST + WebSocket + Relay Queue. A2A v0.3: HTTP + SSE + gRPC. AG-UI: HTTP + WebSocket. Matrix: HTTP long-polling + WebSocket. MCP: HTTP + stdio (for local). |
| **How Well** | No single protocol covers all three needs (native mobile, streaming, federation). Must define transport bindings for each context. |
| **Notes** | WebSocket for client-to-server (real-time, bidirectional). HTTP/SSE for streaming agent responses. gRPC or HTTP for server-to-server. TLS 1.3 mandatory everywhere. |

### TS-6: Mixed Human-Agent Threads

| Attribute | Detail |
|-----------|--------|
| **Why Expected** | This is the core value proposition. No existing protocol handles mixed human-agent threads where both are equal participants. Every protocol handles either human-to-agent OR agent-to-agent, never both seamlessly. |
| **Complexity** | HIGH |
| **Dependencies** | TS-2 (Identity), TS-3 (Capability Discovery), TS-4 (Streaming) |
| **Existing Coverage** | No protocol addresses this directly. A2A is agent-to-agent task delegation. AG-UI is agent-to-user in embedded app contexts. AMP is agent-to-agent messaging. |
| **How Well** | This is the primary gap in the ecosystem. Nobody has solved it. |
| **Notes** | Thread participants must have a uniform interface regardless of type. Agents can @-mention humans and vice versa. Typing indicators adapt per participant type (streaming cursor for agents, typing dots for humans). Presence semantics differ (humans: online/offline; agents: available/busy/processing). |

### TS-7: Cryptographic Message Signing

| Attribute | Detail |
|-----------|--------|
| **Why Expected** | OWASP Top 10 for Agentic Applications 2026 lists agent-to-agent communication lacking authentication as a critical risk. 48% of security professionals identify agentic AI as the #1 attack vector for 2026. Non-negotiable. |
| **Complexity** | MEDIUM |
| **Dependencies** | TS-2 (Identity -- keys bound to identity) |
| **Existing Coverage** | AMP: Ed25519 signatures with canonical signing format (`from|to|subject|priority|in_reply_to|SHA256(payload)`). A2A v0.3: JWS-signed Agent Cards. ANP: W3C DID-based identity with cryptographic proofs. |
| **How Well** | AMP's approach is practical and well-specified. Ed25519 is fast, widely supported, and appropriate for message signing. Use it. |
| **Notes** | Every message must be signed. Verification must be efficient (Ed25519 is ~70x faster than RSA-2048 for verification). Key distribution via identity provider. Key rotation protocol needed. |

### TS-8: End-to-End Encryption

| Attribute | Detail |
|-----------|--------|
| **Why Expected** | E2E encryption has evolved from competitive advantage to baseline expectation in 2026. Users simply expect private conversations remain private. Matrix (Olm/Megolm) and Signal proved this is achievable in federated/group contexts. |
| **Complexity** | HIGH |
| **Dependencies** | TS-7 (Signing -- encryption builds on key infrastructure), TS-1 (Federation -- E2E across servers) |
| **Existing Coverage** | Matrix: Olm (1:1) + Megolm (group). Signal Protocol: Double Ratchet. AMP: TLS 1.2+ transport-level only, no E2E spec. |
| **How Well** | Matrix's Megolm for group encryption is the best model for multi-participant threads. But agent threads introduce unique challenges: agents may need to process plaintext (for LLM inference), creating inherent tension with E2E. |
| **Notes** | Design decision needed: E2E between human participants with agents as trusted processors? Or full E2E with agents having their own keys? The former is more practical for v1. At minimum, transport encryption (TLS 1.3) is mandatory. Full E2E is table stakes for human-to-human messages within the protocol, with a clear trust model for agent access. |

### TS-9: Basic Rich Content (Text, Code, Files, Images)

| Attribute | Detail |
|-----------|--------|
| **Why Expected** | Any messaging protocol that only supports plain text is DOA. Users expect markdown, code blocks with syntax highlighting, inline images, and file attachments. Every competitor has this. |
| **Complexity** | LOW-MEDIUM |
| **Dependencies** | TS-5 (Transport -- file transfer needs defined) |
| **Existing Coverage** | Every messaging platform supports some subset. Slack's Block Kit is the richest (buttons, selects, etc.). AMP supports digest-verified attachments. |
| **How Well** | None are designed for agent output (200-line code files, structured data, charts). Slack's 50-block limit and ~13KB payload cap fail for agent use cases. |
| **Notes** | Message payload should be typed content blocks: text (markdown), code (with language, line numbers), image, file (with MIME type, digest). No arbitrary size limits -- let the client handle display. |

### TS-10: Conversation Threading

| Attribute | Detail |
|-----------|--------|
| **Why Expected** | Multi-participant conversations become unreadable without threading. Flat-by-default with optional sub-threading is the right model (matches the PROJECT.md design). |
| **Complexity** | MEDIUM |
| **Dependencies** | TS-6 (Mixed Threads -- threading model must work for both humans and agents) |
| **Existing Coverage** | Slack: native threads. Discord: thread channels. Telegram: reply-to chains (not true threads). Matrix: relations/threads. |
| **How Well** | Slack's threading is the best UX reference. But no platform supports agent-initiated sub-threads (e.g., agent creates a sub-thread for a tool-call discussion). |
| **Notes** | Flat by default. Any participant can start a sub-thread. Sub-threads inherit parent thread context. Agents can create sub-threads for side conversations (e.g., coordinating with another agent without cluttering the main thread). |

---

## Differentiators

Features that set the protocol apart. Not expected (users would adopt without them), but create competitive advantage and define the product's unique identity.

### D-1: Interactive Agent UI (Forms, Buttons, Charts, Approvals)

| Attribute | Detail |
|-----------|--------|
| **Value Proposition** | Goes beyond text + code to declarative, interactive UI components rendered natively by the client. Agents can send forms, approval workflows, data visualizations, and interactive dashboards -- not as web links, but inline in the conversation. |
| **Complexity** | HIGH |
| **Dependencies** | TS-9 (Rich Content -- builds on typed content blocks), TS-4 (Streaming -- UI updates need real-time delivery) |
| **Existing Coverage** | A2UI: declarative JSON spec, flat adjacency list model, LLM-friendly, SwiftUI renderer exists, moving toward v1.0. MCP Apps: sandboxed HTML iframes in chat (ChatGPT, Claude, VS Code support). Slack Block Kit: rich but limited (50 blocks, ~13KB). |
| **How Well** | A2UI is the strongest model. "UI as data, not code" is the right security model -- agents reference a client-controlled component catalog, preventing UI injection. MCP Apps is powerful but iframe-based (not native). |
| **Notes** | Adopt A2UI's declarative model. Define a component catalog for the protocol (Button, Form, Table, Chart, ApprovalFlow, CodeEditor, ProgressBar). Clients render using native components (SwiftUI for iOS). Agents can only reference catalog components, not inject arbitrary UI. |

### D-2: Hierarchical Memory / Context Management

| Attribute | Detail |
|-----------|--------|
| **Value Proposition** | Protocol-level memory management so agents don't each need external databases. Conversation context is a first-class protocol concept, not an afterthought. No existing messaging platform provides this. |
| **Complexity** | HIGH |
| **Dependencies** | TS-10 (Threading -- memory scoped to conversations/threads), TS-6 (Mixed Threads -- memory shared across participant types) |
| **Existing Coverage** | Letta/MemGPT: core memory (in-context), conversational memory (recent), archival memory (long-term). AG-UI: typed state store with event-sourced diffs. No messaging protocol has memory as a primitive. |
| **How Well** | Letta's architecture is the best reference but is per-agent, not per-conversation. The protocol needs conversation-scoped memory that all participants (human and agent) can contribute to and read from. |
| **Notes** | Three tiers: (1) Active context -- current thread + recent messages, always available to agents. (2) Conversation memory -- full thread history, queryable. (3) Archival memory -- cross-conversation knowledge, user preferences, long-term facts. Server-side persistence. Agents get a structured context payload, not raw message history. |

### D-3: Tool-Use Visibility and Status

| Attribute | Detail |
|-----------|--------|
| **Value Proposition** | When an agent invokes tools, all participants see what's happening in real-time: "Searching order history...", "Running code...", "Waiting for API response...". Transforms the opaque "agent is thinking" into transparent "agent is doing X." |
| **Complexity** | MEDIUM |
| **Dependencies** | TS-4 (Streaming -- status updates need real-time delivery) |
| **Existing Coverage** | AG-UI: TOOL_CALL events, STEP lifecycle events. No messaging platform has this. Telegram's `sendChatAction` is the closest (typing indicator), but conveys nothing about what the agent is doing. |
| **How Well** | AG-UI's model is excellent. Adapt for messaging context: tool-call events as lightweight protocol messages visible to all thread participants. |
| **Notes** | Define protocol message types: ToolCallStarted (tool name, description), ToolCallProgress (intermediate results), ToolCallCompleted (result summary). Clients render as collapsible activity indicators. Users can expand to see details. |

### D-4: Human-in-the-Loop Interrupts and Approvals

| Attribute | Detail |
|-----------|--------|
| **Value Proposition** | Agents can pause execution and request human approval before taking sensitive actions. Protocol-level support, not app-level hacks. Critical for enterprise adoption. |
| **Complexity** | MEDIUM |
| **Dependencies** | D-1 (Interactive UI -- approval forms need rich rendering), TS-4 (Streaming -- interrupt events) |
| **Existing Coverage** | AG-UI: INTERRUPT events that pause agent flow until user responds. A2A: `input-required` task state. No messaging protocol has native interrupts. |
| **How Well** | AG-UI's interrupt model is production-proven. Integrate at the protocol level as a first-class message type. |
| **Notes** | InterruptRequest message type with: action description, risk level, options (approve/reject/modify), timeout, default action if no response. Responses are cryptographically signed to prove human approval. |

### D-5: Multi-Agent Coordination Primitives

| Attribute | Detail |
|-----------|--------|
| **Value Proposition** | Protocol-level support for multiple agents collaborating in a thread: structured handoffs, task delegation, shared state, and role-based permissions. No messaging platform has this. |
| **Complexity** | HIGH |
| **Dependencies** | TS-3 (Capability Discovery -- agents must know what others can do), TS-6 (Mixed Threads), D-2 (Memory -- shared context across agents) |
| **Existing Coverage** | A2A: task lifecycle (submitted/working/input-required/completed/failed/canceled/rejected), context sharing, UX negotiation. Microsoft Agent Framework: GroupChat pattern with selector. |
| **How Well** | A2A's task lifecycle is well-designed for delegation. But it's HTTP-based point-to-point, not conversational. Need to adapt task lifecycle into messaging thread semantics. |
| **Notes** | Define: HandoffRequest (context transfer + scope), TaskUpdate (status changes visible to all), DelegationChain (cryptographic proof of who authorized what). Agents can delegate to other agents in-thread. Users see delegation happening transparently. |

### D-6: Anti-Prompt-Injection at Protocol Level

| Attribute | Detail |
|-----------|--------|
| **Value Proposition** | Protocol-level defenses against prompt injection -- not just application-level guardrails. Messages carry trust annotations, content is separated from instructions by protocol structure, and semantic validation layers verify message patterns. |
| **Complexity** | HIGH |
| **Dependencies** | TS-7 (Signing -- trust chain verification), TS-3 (Capability Discovery -- expected behavior patterns) |
| **Existing Coverage** | AMP: trust-level annotations on message content for prompt injection defense. OWASP: recommends message-level encryption + digital signatures + semantic validation layers. MCP Secure: cryptographic layer analogous to TLS for MCP. |
| **How Well** | AMP's trust annotations are the right approach. OWASP provides the threat model. No protocol has comprehensive prompt injection defense yet. |
| **Notes** | Separate user content from system/protocol instructions at the message structure level (not just application-level). Trust annotations on content blocks (user-generated vs. agent-generated vs. system). Agents declare expected input patterns in Agent Cards; receiving agents validate against these patterns. |

### D-7: Structured Message Payloads with Typed Blocks

| Attribute | Detail |
|-----------|--------|
| **Value Proposition** | Messages composed of typed, renderable blocks -- not flat text with formatting hints. Each block has a declared type (text, code, table, chart, form, artifact) with associated schema. Receivers know exactly what they're getting. |
| **Complexity** | MEDIUM |
| **Dependencies** | TS-9 (Rich Content -- extends basic content types) |
| **Existing Coverage** | Slack Block Kit: typed blocks but limited (50 blocks, ~13KB). A2UI: flat adjacency list of typed components. MCP Apps: sandboxed HTML. |
| **How Well** | A2UI's flat adjacency list is the best structural model. Slack proved typed blocks work for messaging UX. Combine both approaches. |
| **Notes** | Define block types: TextBlock (markdown), CodeBlock (language, content, executable flag), TableBlock (headers, rows, sortable), ChartBlock (type, data, config), FormBlock (fields, validation, submit action), ArtifactBlock (grouped related outputs). No arbitrary payload size limits. |

### D-8: Event Sourcing and Audit Trail

| Attribute | Detail |
|-----------|--------|
| **Value Proposition** | Complete, replayable history of all events in a conversation -- not just messages but state changes, tool invocations, handoffs, approval decisions, and metadata updates. Critical for enterprise trust and debugging. |
| **Complexity** | HIGH |
| **Dependencies** | D-3 (Tool Visibility -- tool events must be logged), D-5 (Multi-Agent -- coordination events must be logged), TS-7 (Signing -- events must be tamper-evident) |
| **Existing Coverage** | Matrix: event-based architecture where all communications are stored as events. AG-UI: event-sourced state diffs. No agent protocol has a comprehensive audit trail. |
| **How Well** | Matrix's event-based architecture is the right model for message history. Extend with agent-specific event types (tool calls, delegations, approval decisions). |
| **Notes** | Every protocol action produces a signed event. Events are ordered, immutable, and replayable. Clients can reconstruct conversation state from events. Enables debugging ("why did the agent do X?") and compliance ("who approved action Y?"). |

### D-9: MCP / A2A Compatibility Layer

| Attribute | Detail |
|-----------|--------|
| **Value Proposition** | Agents that already speak MCP (tools) or A2A (task delegation) can participate in the messaging protocol without rewriting. The protocol bridges the gap between existing agent protocols and conversational messaging. |
| **Complexity** | MEDIUM |
| **Dependencies** | TS-3 (Capability Discovery -- must map to/from A2A Agent Cards), D-5 (Multi-Agent -- must map to/from A2A task lifecycle) |
| **Existing Coverage** | MCP: 97M+ monthly SDK downloads, universal standard for tool access. A2A v0.3: 50+ enterprise partners, gRPC support, signed cards. Both are HTTP-based, not messaging-native. |
| **How Well** | No messaging protocol bridges to MCP/A2A today. This is the key integration gap. |
| **Notes** | Define protocol-level message types that map to MCP tool calls and A2A task operations. An MCP server can be exposed as an agent in a thread. A2A task updates can be rendered as thread messages. This makes the protocol the "missing layer" between agent infrastructure and human-facing messaging. |

### D-10: Conversation-Level Permissions and Scoping

| Attribute | Detail |
|-----------|--------|
| **Value Proposition** | Fine-grained permissions per conversation, not binary "in or out." Agents can be granted specific capabilities (read-only, can-send-messages, can-execute-tools, can-delegate, can-access-memory-tier-X) scoped to a specific thread. |
| **Complexity** | MEDIUM |
| **Dependencies** | TS-2 (Identity), TS-7 (Signing -- permissions must be verifiable) |
| **Existing Coverage** | AMP: allowlist-based ACLs with wildcard matching. Slack: OAuth scopes (but per-workspace, not per-conversation). A2A: authentication requirements in Agent Cards. |
| **How Well** | No protocol has per-conversation agent permissions. AMP's ACLs are identity-level, not conversation-level. This is novel. |
| **Notes** | When adding an agent to a thread, the inviter specifies a permission scope. Permissions are signed by the inviter and verifiable by the agent and other participants. Supports delegation chains with scope narrowing (Agent A can delegate to Agent B with equal or lesser permissions, never greater). |

---

## Anti-Features

Things to deliberately NOT build, and why.

### AF-1: Arbitrary Code Execution in Messages

| Anti-Feature | DO NOT allow agents to send executable code that runs on the client |
|--------------|-----|
| **Why Avoid** | A2UI's core insight: "UI as data, not code." Allowing code execution in messages is the #1 UI injection attack vector. MCP Apps uses sandboxed iframes as a compromise; for a native protocol, declarative components are safer and more predictable. OWASP lists tool misuse and unintended execution as a top-10 agentic risk. |
| **What to Do Instead** | Declarative component catalog. Agents reference pre-approved component types. Clients render using their own native implementations. The agent describes what to show; the client decides how to show it. |

### AF-2: Built-In Payment / Transaction Layer

| Anti-Feature | DO NOT build payment or commerce features into v1 |
|--------------|-----|
| **Why Avoid** | Massively increases regulatory complexity (PCI-DSS, financial regulations vary by jurisdiction). Messaging protocols that try to be payment protocols end up being mediocre at both (WeChat is the exception, not the rule, and it's centralized). The PROJECT.md explicitly scopes this out. |
| **What to Do Instead** | Design the rich content system (D-1, D-7) to be extensible enough that a payment block type can be added later by a third-party extension. Don't preclude it, but don't build it. |

### AF-3: Agent Marketplace / Store

| Anti-Feature | DO NOT build a centralized agent marketplace |
|--------------|-----|
| **Why Avoid** | Contradicts the federation model. A centralized marketplace creates a gatekeeper, which is exactly the problem with existing platforms (App Store model). Agent discovery should be decentralized, matching the protocol's federated architecture. |
| **What to Do Instead** | Agent Cards (TS-3) enable discovery. DNS-based resolution of agent capabilities via the `@provider` portion of identity. Third parties can build directories/marketplaces on top of the discovery protocol, but the protocol itself should not privilege any single directory. |

### AF-4: Voice / Video Messaging

| Anti-Feature | DO NOT add voice or video in v1 |
|--------------|-----|
| **Why Avoid** | Enormous complexity (codecs, bandwidth, NAT traversal, SRTP/DTLS). WebRTC alone is a multi-year engineering effort. AI agents primarily communicate via text and structured data. Adding voice/video dilutes focus and delays the core protocol. PROJECT.md explicitly scopes this out. |
| **What to Do Instead** | Design message types to be extensible so voice/video block types can be added in v2+. Support audio transcription as a text message type if needed. |

### AF-5: Message Editing / Deletion After Delivery

| Anti-Feature | DO NOT allow retroactive message editing or deletion |
|--------------|-----|
| **Why Avoid** | Conflicts with the event-sourced audit trail (D-8). If agents can edit past messages, the audit trail becomes unreliable. If messages can be deleted, accountability is lost. In agent contexts, message integrity is more important than the convenience of editing. Agents should send corrections as new messages, not edit history. |
| **What to Do Instead** | Corrections are new messages that reference the original (with a `corrects` relation type). The original remains in the event log. Clients can display the correction inline while preserving history. |

### AF-6: "Smart" Server-Side Message Processing

| Anti-Feature | DO NOT have servers inspect, analyze, or modify message content in transit |
|--------------|-----|
| **Why Avoid** | Federation servers are relays, not processors. If servers can read/modify messages, E2E encryption is impossible, trust model breaks down, and server operators become attack surfaces. This is a lesson learned from email (where servers do inspect content, leading to endless spam/privacy issues). |
| **What to Do Instead** | Servers route opaque, signed, optionally encrypted message payloads. All processing happens at endpoints (clients and agents). Servers verify signatures for routing decisions but do not access payload content. |

### AF-7: Proprietary Extensions Without Namespace

| Anti-Feature | DO NOT allow unnamespaced protocol extensions |
|--------------|-----|
| **Why Avoid** | XMPP's 400+ XEPs proved that uncontrolled extensions fragment the ecosystem. Clients can't know which extensions other clients support. Leads to "works in my client, breaks in yours" problems that killed XMPP's consumer viability. |
| **What to Do Instead** | Extension mechanism with mandatory namespacing and capability negotiation. Clients advertise supported extensions in handshake. Unknown extensions are ignored gracefully, not cause errors. Follow XMPP's lesson: fewer, better-specified core features beat hundreds of optional extensions. |

---

## Feature Dependencies

```
                    TS-5: Transport
                    /            \
                   /              \
            TS-1: Federation    TS-4: Streaming
               |      \            |        \
               |       \           |         \
         TS-2: Identity  \   D-3: Tool      D-4: Interrupts
          /    |    \     \   Visibility        |
         /     |     \     \                    |
   TS-3:    TS-7:   D-10:  TS-10:           D-1: Interactive
   Agent    Signing  Perms  Threading          UI
   Cards      |            |      \
    |     TS-8: E2E    D-2: Memory  TS-6: Mixed
    |                      |         Threads
    |                      |
   D-9: MCP/A2A       D-5: Multi-Agent
   Compat              Coordination
                           |
                       D-8: Event
                       Sourcing

   D-6: Anti-Prompt-Injection <-- TS-7 + TS-3
   D-7: Structured Payloads  <-- TS-9
```

### Critical Path (features that unblock the most downstream work)

1. **TS-5: Transport** -- everything depends on this
2. **TS-2: Identity** -- federation, signing, permissions, capability discovery all depend on identity
3. **TS-7: Signing** -- encryption, permissions, audit trail, anti-injection all depend on signing
4. **TS-4: Streaming** -- tool visibility, interrupts, interactive UI all depend on streaming
5. **TS-6: Mixed Threads** -- the core value proposition; depends on identity + threading + streaming

---

## MVP Recommendation

### Phase 1: Protocol Foundation (must ship first)

Prioritize these table stakes -- they form the minimum viable protocol:

1. **TS-5: Transport** -- define client-to-server and server-to-server transport bindings
2. **TS-2: Identity** -- email-like addressing with provider resolution
3. **TS-7: Cryptographic Signing** -- Ed25519 message signing from day one
4. **TS-9: Basic Rich Content** -- typed content blocks (text, code, image, file)
5. **TS-4: Streaming** -- event-based streaming for agent responses
6. **TS-10: Threading** -- flat-by-default with optional sub-threads

### Phase 2: Agent-Native Features (what makes it worth switching to)

7. **TS-6: Mixed Human-Agent Threads** -- the core differentiator
8. **TS-3: Agent Cards** -- capability discovery compatible with A2A
9. **D-3: Tool-Use Visibility** -- transparent agent activity
10. **D-7: Structured Payloads** -- typed message blocks

### Phase 3: Rich Experience (what makes it feel magical)

11. **D-1: Interactive Agent UI** -- declarative components via A2UI model
12. **D-2: Hierarchical Memory** -- protocol-level context management
13. **D-4: Human-in-the-Loop Interrupts** -- approval workflows

### Phase 4: Scale and Trust (what makes enterprises adopt)

14. **TS-1: Federation** -- full multi-server routing
15. **TS-8: E2E Encryption** -- end-to-end for human messages, trust model for agents
16. **D-5: Multi-Agent Coordination** -- handoffs, task lifecycle, delegation chains
17. **D-8: Event Sourcing** -- complete audit trail
18. **D-10: Conversation Permissions** -- fine-grained scoping
19. **D-6: Anti-Prompt-Injection** -- protocol-level defenses
20. **D-9: MCP/A2A Compatibility** -- bridge to existing ecosystem

### Defer Indefinitely

- AF-2: Payment layer
- AF-4: Voice/video
- AF-3: Agent marketplace (let third parties build)

---

## Sources

### Protocols and Specifications
- [A2A Protocol Specification v0.3](https://a2a-protocol.org/v0.3.0/specification/)
- [AG-UI Protocol Documentation](https://docs.ag-ui.com/introduction)
- [A2UI Protocol](https://a2ui.org/)
- [AMP Protocol](https://agentmessaging.org/)
- [MCP Apps Extension](https://modelcontextprotocol.io/extensions/apps/overview)
- [MCP Specification](https://modelcontextprotocol.io/specification/2025-11-25)

### Security
- [OWASP Top 10 for Agentic Applications 2026](https://genai.owasp.org/resource/owasp-top-10-for-agentic-applications-for-2026/)
- [MCP Secure](https://mcp-secure.dev/)
- [Security Threat Modeling for AI-Agent Protocols](https://arxiv.org/html/2602.11327)

### Products and Frameworks
- [Agenium Messenger](https://dev.to/agenium_platform/we-built-an-ai-agent-messenger-and-its-live-4pii)
- [Letta / MemGPT Documentation](https://docs.letta.com/concepts/memgpt/)
- [Agent Protocol (AGI Inc)](https://github.com/agi-inc/agent-protocol)

### Architecture and Patterns
- [Matrix Protocol (Wikipedia)](https://en.wikipedia.org/wiki/Matrix_(protocol))
- [XMPP vs Matrix vs MQTT Comparison](https://www.rst.software/blog/xmpp-vs-matrix-vs-mqtt-which-instant-messaging-protocol-is-best-for-your-chat-application)
- [Agora Protocol Paper](https://arxiv.org/html/2410.11905v1)
- [Messaging Apps Trends 2026](https://www.daylox.com/blog/messaging-apps-evolution-2026/)

### Developer Experience
- [Google Developers Guide to AI Agent Protocols](https://developers.googleblog.com/developers-guide-to-ai-agent-protocols/)
- [A2A Deep Dive - WWT](https://www.wwt.com/blog/agent-2-agent-protocol-a2a-a-deep-dive)
