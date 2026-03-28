# Project Research Summary

**Project:** AI-Native Messaging Protocol & App
**Domain:** Federated messaging protocol with AI-native primitives + reference server + iOS client
**Researched:** 2026-03-27
**Confidence:** HIGH

## Executive Summary

This project is a federated messaging protocol where humans and AI agents are equal participants in conversations. The research establishes that every successful federated messaging system (SMTP, XMPP, Matrix, ActivityPub) shares the same three-API architecture: Client-Server, Server-Server (Federation), and Identity/Discovery. Our protocol follows this universal pattern but innovates in three areas no existing protocol addresses: mixed human-agent threads, token-by-token streaming as a first-class protocol primitive, and declarative agent UI rendered natively by clients. The recommended stack is Rust/Axum for the reference server (validated by Matrix's Conduit homeserver at 32 MiB RAM), pure SwiftUI with GRDB.swift for the iOS client, and JSON-RPC 2.0 over WebSocket as the wire protocol.

The single most important strategic decision is to avoid the overspecification trap. AMP has 11 spec documents at v0.1.2 with no real adoption. Matrix spent a decade on state resolution and is still fixing critical bugs. The recommended approach is to define a protocol kernel (identity, message signing, streaming, basic content), implement it on a single server, and ship a usable iOS app within the first two phases. Federation is deliberately deferred to Phase 4 -- the protocol should prove its value as a superior messaging experience before tackling distributed systems complexity.

The top risks are: (1) prompt injection through federated messages -- every incoming message is untrusted input to an LLM, and attack success rates exceed 85% against state-of-the-art defenses; (2) the adoption chicken-and-egg problem -- people adopt products, not protocols, so the iOS app must be compelling as a standalone messaging client; and (3) iOS background execution constraints that will silently break real-time streaming if not designed into the protocol from day one. Mitigations: architectural boundary-level security (not prompt-level), pre-built useful agents for day-one utility, and a dual delivery mode (streaming foreground + store-and-forward background).

## Key Findings

### Recommended Stack

The stack splits into three domains -- protocol specification, reference server, and iOS client -- with strong consensus across sources for each. See [STACK.md](./STACK.md) for full rationale and alternatives considered.

**Core technologies:**
- **Rust 1.85+ / Axum 0.8**: Reference server -- memory safety without GC pauses, critical for real-time streaming p99 latency. Conduit validates this for federated messaging.
- **PostgreSQL 16+ / sqlx 0.8**: Event store -- concurrent writes needed for federation, JSONB for flexible message payloads, LISTEN/NOTIFY for real-time. SQLite locks on writes.
- **SwiftUI (iOS 18+) / GRDB.swift 7.10**: iOS client -- native declarative UI with the most mature Swift database toolkit. SwiftData rejected (no background tasks, poor large dataset perf).
- **WebSocket + JSON-RPC 2.0**: Wire protocol -- bidirectional, persistent, low-latency. JSON-RPC aligns with MCP and A2A. HTTP/SSE as fallback.
- **Ed25519 (ring + CryptoKit)**: Cryptographic signing -- fast, small signatures (64 bytes), misuse-resistant. Both server (ring) and iOS (CryptoKit) have native support.
- **AsyncAPI v3.1 + JSON Schema 2020-12**: Spec format -- purpose-built for event-driven protocols. Machine-readable schemas, code generation for docs and clients.
- **Docusaurus 3.x**: Spec website -- versioning, MDX for interactive examples, React components.

### Expected Features

See [FEATURES.md](./FEATURES.md) for full feature landscape with dependencies, complexity, and existing coverage analysis.

**Must have (table stakes):**
- Transport layer (WebSocket C2S + HTTP S2S) -- everything depends on this
- Email-like identity (`name@provider`) with Ed25519 key binding
- Cryptographic message signing -- OWASP lists unauthenticated agent communication as critical risk
- Streaming message delivery -- token-by-token is baseline UX since ChatGPT
- Basic rich content (text/markdown, code blocks, images, files)
- Conversation threading (flat-by-default with optional sub-threads)
- Mixed human-agent threads -- the core value proposition, no protocol solves this today
- Agent capability discovery (Agent Cards, compatible with A2A)
- End-to-end encryption -- table stakes for human-to-human, needs clear trust model for agent access

**Should have (competitive):**
- Interactive agent UI (declarative components via A2UI model)
- Hierarchical memory / context management (protocol-level, not app-level)
- Tool-use visibility and status events
- Human-in-the-loop interrupts and approval workflows
- Multi-agent coordination primitives (handoffs, delegation chains)
- Anti-prompt-injection at protocol level (trust annotations, content boundaries)
- MCP/A2A compatibility layer
- Event sourcing and audit trail

**Defer (v2+):**
- Voice/video messaging
- Payment/transaction layer
- Agent marketplace (let third parties build on discovery protocol)
- Message editing/deletion (conflicts with audit trail; use corrections instead)

### Architecture Approach

The architecture follows the universal federated pattern (client-server-federation split) but makes two critical departures from Matrix: (1) linear per-server event ordering instead of a DAG with state resolution, and (2) push-based federation with origin-server authority instead of full history replication. These two decisions eliminate the most painful complexity in Matrix (state resolution bugs, state resets, massive storage overhead) at the cost of reduced censorship resistance -- an acceptable trade-off for v1. The spec structure follows XMPP's extensibility model (small core + formal extensions) rather than Matrix's monolithic approach. See [ARCHITECTURE.md](./ARCHITECTURE.md) for full component design, data flow diagrams, and database schema.

**Major components:**
1. **Home Server (Rust/Axum)** -- Connection Manager, Auth & Identity, Message Router, Thread Manager, Stream Manager, Event Store (PostgreSQL), Federation Sender/Receiver, Push Gateway, Media Store
2. **iOS Client (SwiftUI)** -- MVVM + Repository pattern with @Observable, GRDB.swift local cache, Protocol SDK (shared library for crypto/serialization)
3. **Agent SDK (TypeScript)** -- Headless client library for agents to send/receive messages, declare capabilities, stream responses
4. **Protocol Specification** -- Markdown prose + JSON Schema definitions + AsyncAPI transport bindings, published via Docusaurus

### Critical Pitfalls

See [PITFALLS.md](./PITFALLS.md) for all 13 pitfalls with real-world examples and detection strategies.

1. **Overspecification trap** -- Designing a comprehensive spec before building anything. AMP has 11 docs at v0.1.2 with no adoption. Prevention: define a protocol kernel, implement immediately, ship working demo in 4-6 weeks.
2. **State resolution complexity** -- Matrix spent 8+ years on this and Project Hydra found high-severity bugs in 2025. Prevention: use origin-server authority for thread state, no DAG, no distributed consensus in v1.
3. **Prompt injection via federated messages** -- Every federated message is untrusted LLM input. Attack success >85% against state-of-the-art defenses. Prevention: architectural boundary-level security, trust tiers for message sources, Ed25519 signing from day one.
4. **Adoption chicken-and-egg** -- Protocols need implementations, implementations need users, users need a network. Prevention: ship the iOS app as the product, pre-build useful agents, host a server for early users.
5. **iOS background execution mismatch** -- iOS suspends WebSocket connections in background, throttles push to 2-3/hour. Prevention: dual delivery mode (streaming foreground + store-and-forward background) designed into the protocol, not patched onto the app.

## Implications for Roadmap

Based on combined research, the architecture has a clear dependency chain that dictates build order. Phases 3 and 4 can run in parallel (agent features and federation are independent concerns that converge later).

### Phase 1: Protocol Specification & Data Model
**Rationale:** Everything downstream depends on the spec. But the spec must be a kernel, not an encyclopedia. Define only what Phase 2 needs to implement.
**Delivers:** Core event schema (JSON Schema), thread model, identity model (`name@provider`), Ed25519 signing spec (canonical JSON per RFC 8785), Agent Card schema, AsyncAPI transport bindings, Docusaurus spec website, version negotiation design.
**Addresses:** TS-5 (Transport), TS-2 (Identity), TS-7 (Signing) from FEATURES.md -- the critical path that unblocks the most downstream work.
**Avoids:** Pitfall #1 (overspecification) -- spec only what you will implement in Phase 2. Pitfall #9 (versioning) -- version negotiation must be in the initial design.

### Phase 2: Single-Server Reference Implementation + Basic iOS Client
**Rationale:** Proves the protocol works before federation adds distributed systems complexity. The iOS app provides immediate user value and drives adoption.
**Delivers:** Working Rust/Axum server (event store, auth, threads, sync, WebSocket streaming), basic iOS app (login, thread list, send/receive messages, streaming agent responses), local-only messaging between humans and agents.
**Addresses:** TS-4 (Streaming), TS-9 (Rich Content), TS-10 (Threading), TS-6 (Mixed Threads) -- the core experience.
**Avoids:** Pitfall #2 (state resolution) -- single server, no federation complexity. Pitfall #4 (adoption) -- ship a usable product, not just a protocol.

### Phase 3: Agent Features & Rich Experience
**Rationale:** Once basic messaging works, add the agent-native features that differentiate this protocol. This is what makes it worth switching to from existing tools.
**Delivers:** Agent Registry with capability cards, rich content blocks (forms, tables, code, charts), tool-use visibility events, streaming UI polish, human-in-the-loop interrupts, TypeScript Agent SDK, hierarchical memory/context management.
**Addresses:** TS-3 (Agent Cards), D-1 (Interactive UI), D-2 (Memory), D-3 (Tool Visibility), D-4 (Interrupts), D-7 (Structured Payloads) from FEATURES.md.
**Avoids:** Pitfall #6 (rendering fragmentation) -- mandatory fallback field on all rich content, minimum rendering set defined in spec. Pitfall #7 (context explosion) -- protocol-level context management, not app-level.

### Phase 4: Federation
**Rationale:** Federation is the protocol's long-term value, but it is a distributed systems problem that should not block single-server viability. Phases 3 and 4 can run in parallel.
**Delivers:** Server-to-server API (HTTPS + mutual TLS), DNS SRV + well-known discovery, federation sender/receiver with event signing, selective backfill, multi-server integration tests (docker-compose).
**Addresses:** TS-1 (Federation), TS-8 (E2E Encryption design) from FEATURES.md.
**Avoids:** Pitfall #2 (state resolution) -- origin-server authority, push-based federation, no DAG. Pitfall #8 (federation spam) -- protocol-level trust tiers, graduated server reputation, cryptographic server identity. Pitfall #3 (prompt injection) -- trust tiers for federated message sources.

### Phase 5: Production Hardening & Ecosystem
**Rationale:** Polish and enterprise features after the core protocol and federation are proven.
**Delivers:** Push notifications (APNs), media store, presence system, multi-agent coordination primitives, conversation permissions, event sourcing audit trail, anti-prompt-injection protocol defenses, MCP/A2A compatibility layer, Protocol Spec v1.0.
**Addresses:** D-5 (Multi-Agent Coordination), D-6 (Anti-Prompt-Injection), D-8 (Event Sourcing), D-9 (MCP/A2A Compat), D-10 (Permissions) from FEATURES.md.
**Avoids:** Pitfall #5 (iOS background) -- dual delivery mode finalized. Pitfall #12 (leaking protocol details to users) -- user-friendly error mapping layer.

### Phase Ordering Rationale

- **Spec before code, but spec is a kernel, not a thesis.** Only spec what Phase 2 needs. Unimplemented sections are explicitly marked draft.
- **Single-server before federation.** Get the data model, event ordering, and streaming right without distributed systems complexity. This is Matrix's biggest lesson.
- **Agent features and federation are independent.** The architecture cleanly separates these concerns. Phase 3 and Phase 4 can run in parallel with different contributors/workstreams.
- **iOS app ships in Phase 2, not Phase 5.** The app is the adoption vehicle. People don't adopt protocols; they adopt products.
- **Security is not a phase -- it is embedded.** Ed25519 signing in Phase 1. Trust tiers in Phase 4. Anti-injection in Phase 5. Each phase builds on the previous security layer.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 1 (Spec):** AsyncAPI + Docusaurus integration needs testing; the workflow may need custom glue. Agent Card schema needs careful design for A2A compatibility.
- **Phase 4 (Federation):** Push-based federation with origin-server authority is simpler than Matrix's DAG, but needs its own conflict resolution strategy for concurrent membership changes. CRDT-based state for thread metadata should be evaluated.
- **Phase 5 (Memory/Context):** Hierarchical memory at the protocol level is novel -- no existing protocol does this. Letta/MemGPT is the best reference but is per-agent, not per-conversation. Needs design research.

Phases with standard patterns (skip additional research):
- **Phase 2 (Single-Server):** Rust/Axum + PostgreSQL + WebSocket is a well-documented pattern. Conduit and Element X iOS are direct reference implementations.
- **Phase 3 (Rich Content):** A2UI's declarative model is well-specified. Native SwiftUI rendering is standard iOS development.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Established tools with strong consensus. Rust/Axum validated by Conduit. SwiftUI/GRDB validated by Element X iOS. JSON-RPC aligned with MCP/A2A. |
| Features | HIGH | Grounded in analysis of 6+ existing protocols (A2A, AG-UI, A2UI, AMP, MCP, Matrix). Feature gap analysis is thorough. OWASP security framework provides threat model. |
| Architecture | HIGH | Based on 4 mature federated systems (SMTP, XMPP, Matrix, ActivityPub) with decades of combined production experience. Key architectural decisions (linear ordering, push-based federation) are explicitly simpler alternatives to proven-but-painful approaches. |
| Pitfalls | HIGH | Real-world examples from Matrix, XMPP, email, Mastodon. iOS constraints verified against Apple documentation. Prompt injection data from OWASP 2025/2026 and academic meta-analyses. |

**Overall confidence:** HIGH

### Gaps to Address

- **E2E encryption trust model for agents:** Agents need to process plaintext for LLM inference, creating inherent tension with E2E. Decision needed: E2E between humans with agents as trusted processors, or full E2E with agents having their own keys? Defer to Phase 4-5 design.
- **Context management protocol design:** No existing protocol has conversation-scoped hierarchical memory. Letta/MemGPT is per-agent, not per-conversation. The protocol-level context spec needs original design work in Phase 3.
- **Federation conflict resolution for concurrent state changes:** Origin-server authority works for most cases, but concurrent membership changes (two servers inviting different users simultaneously) need a defined resolution strategy. Evaluate CRDTs vs. last-writer-wins during Phase 4 planning.
- **KeychainAccess vs. alternatives:** STACK.md notes medium confidence on this library. Check if swift-keychain-wrapper is more current before Phase 2 iOS work.
- **AsyncAPI + Docusaurus integration:** Confirmed as untested. May need custom glue code. Validate early in Phase 1.
- **Cross-federation streaming:** No existing protocol does real-time token streaming across federated servers. Backpressure, error recovery, and partial delivery need design work during Phase 4.

## Sources

### Primary (HIGH confidence)
- [Matrix Specification v1.18](https://spec.matrix.org/latest/) -- Federation architecture, event model, state resolution
- [XMPP Core RFC 6120](https://xmpp.org/rfcs/rfc6120.html) -- Extensibility model, server-to-server federation
- [ActivityPub W3C Recommendation](https://www.w3.org/TR/activitypub/) -- Actor model, inbox/outbox, WebFinger discovery
- [A2A Protocol v0.3](https://a2a-protocol.org/v0.3.0/specification/) -- Agent Cards, task lifecycle, gRPC support
- [AG-UI Protocol](https://docs.ag-ui.com/introduction) -- Streaming event model, tool-call events, interrupts
- [OWASP Top 10 for Agentic Applications 2026](https://genai.owasp.org/resource/owasp-top-10-for-agentic-applications-for-2026/) -- Security threat model
- [Conduit (Rust Matrix homeserver)](https://conduit.rs/) -- Validates Rust for federated messaging
- [RFC 8785 - JSON Canonicalization Scheme](https://www.rfc-editor.org/rfc/rfc8785) -- Deterministic JSON for signing
- [Apple CryptoKit - Curve25519.Signing](https://developer.apple.com/documentation/cryptokit/curve25519/signing) -- Native Ed25519

### Secondary (MEDIUM confidence)
- [AMP Protocol](https://agentmessaging.org/) -- Agent messaging patterns, Ed25519 signing format
- [A2UI Protocol](https://a2ui.org/) -- Declarative UI-as-data model for agent interfaces
- [MCP Apps Extension](https://modelcontextprotocol.io/extensions/apps/overview) -- Sandboxed UI approach (iframe-based)
- [Letta/MemGPT](https://docs.letta.com/concepts/memgpt/) -- Hierarchical memory architecture
- [Element X iOS](https://github.com/element-hq/element-x-ios) -- SwiftUI messaging app reference implementation
- [GRDB.swift v7.10](https://github.com/groue/GRDB.swift) -- iOS database recommendation validation
- [Security Threat Modeling for AI-Agent Protocols](https://arxiv.org/html/2602.11327) -- Academic threat model

### Tertiary (LOW confidence)
- [AsyncAPI + Docusaurus integration](https://www.asyncapi.com/) -- Untested workflow, needs validation
- [KeychainAccess v4.x](https://github.com/kishikawakatsumi/KeychainAccess) -- Stable but needs currency check vs. alternatives

---
*Research completed: 2026-03-27*
*Ready for roadmap: yes*
