# Project Research Summary

**Project:** AI-Native Messaging Protocol — iOS Reference Client (v3.3)
**Domain:** Native iOS messaging client for a federated, AI-native protocol (AINM)
**Researched:** 2026-03-29
**Confidence:** HIGH

## Executive Summary

This project delivers a native iOS reference client for the AINM protocol — a federated messaging protocol where humans and AI agents are equal participants. The server stack (Rust/Axum, WebSocket, Ed25519, JSON-RPC 2.0) is complete and battle-tested through v3.2. The research scope is exclusively the iOS client layer, which must speak the existing protocol exactly and expose its AI-native features (streaming responses, tool call visibility, interactive blocks, memory transparency) through a polished SwiftUI app targeting iOS 18+.

The recommended approach is MVVM + Repository architecture with GRDB.swift as the local database and the server as the canonical source of truth. All server data flows through GRDB before reaching SwiftUI views — this write-through pattern provides offline resilience, consistent UI state, and decoupled layers without requiring offline-first complexity. A single WebSocket connection is demultiplexed into typed AsyncStreams per protocol notification type. Swift Concurrency (`async/await`, `Actor`, `AsyncStream`) handles all async coordination; no Combine or RxSwift is needed. Eight external SPM dependencies cover non-trivial gaps; everything else is built-in or custom-implemented in under 100 lines.

The critical risk is RFC 8785 JSON Canonicalization Scheme (JCS) interoperability between Swift and Rust. Signatures are only valid if both sides produce identical canonical bytes, and Swift has no standard JCS library — a custom implementation is required and must be built and cross-platform-verified in Phase 1 before anything else, or signature failures will surface intermittently in production with Unicode content. Secondary risks are WebSocket connection lifecycle management (iOS aggressively kills background sockets) and SwiftUI message list performance (LazyVStack does not recycle views — pagination must be designed from day one or the app will crash at scale).

## Key Findings

### Recommended Stack

The iOS client is built on SwiftUI (iOS 18+) with Swift 6.1 strict concurrency. Eight external SPM dependencies cover the non-trivial gaps: GRDB.swift (7.10.0) for local persistence, GRDBQuery (0.11.0) for reactive SwiftUI observation, KeychainAccess (4.2.2) for Ed25519 key storage, MarkdownUI (2.4.1) for rich text rendering, HighlightSwift (1.1.0) for code block syntax highlighting, Nuke (12.9.0) for image caching, and swift-collections/swift-algorithms for efficient data structures. Everything else — WebSocket (URLSessionWebSocketTask), cryptography (Apple CryptoKit), structured logging (OSLog), and navigation (NavigationStack) — is built-in. Two components require custom implementation with no library equivalent: RFC 8785 JCS canonicalization (~100 lines, no Swift library exists) and UUIDv7 generation (~30 lines, Foundation's UUID() produces v4 not v7).

**Core technologies:**
- SwiftUI iOS 18+ / Swift 6.1: Declarative UI with strict concurrency — eliminates data races at compile time for a real-time messaging app with concurrent WebSocket reads, database writes, and UI updates
- GRDB.swift 7.10.0: Production SQLite with ValueObservation, migrations, and background write support — SwiftData is not viable (no background task support, performance degrades with 10K+ messages)
- URLSessionWebSocketTask (built-in): Native WebSocket with zero dependency — sufficient for JSON-RPC 2.0 over WebSocket, no Starscream needed
- Apple CryptoKit (built-in): Hardware-accelerated Ed25519 via Curve25519.Signing — compatible with Rust's `ring` crate (both use 32-byte raw key representation)
- Custom JCS (RFC 8785): Required for cross-platform signing — must use UTF-16 lexicographic key sorting, not JSONEncoder(.sortedKeys) which uses UTF-8 byte order
- MarkdownUI 2.4.1: GFM rendering for agent responses — in maintenance mode but stable and battle-tested (3.7K+ stars)
- Nuke 12.9.0: Async image loading with disk/memory caching — AsyncImage is insufficient (no disk caching, no progressive loading)
- Custom UUIDv7: ~30 lines to generate time-ordered message IDs matching the server's `uuid` crate format

### Expected Features

**Must have (table stakes — Tier 1-2 from FEATURES.md):**
- WebSocket connection + ainm.initialize handshake with capability negotiation
- Ed25519 key generation, Keychain storage, message signing on send, signature verification on receive
- Conversation list and thread view with LazyVStack (paginated from day one — no exceptions)
- Text message rendering with Markdown (MarkdownUI for GFM)
- Streaming responses token-by-token (ainm.stream.start/delta/end) — the defining AI chat UX
- Code block rendering with syntax highlighting (HighlightSwift)
- Image display with caching and fullscreen tap (Nuke)
- File display with download and SHA-256 digest verification
- Multi-block message rendering (content[] array rendered in order)
- Tool call visibility with collapsible indicators and lifecycle grouping by call_id
- Interactive blocks: ButtonBlock, FormBlock, TableBlock, ApprovalBlock + ActionBlock send flow
- Agent identity display (type badge, Agent Card profile on tap)
- @-mention highlighting and composer autocomplete
- Local persistence for offline reading (GRDB write-through cache)
- Connection status indicator, typing/processing indicators (presence), fallback block rendering, stream cancellation

**Should have (differentiators — Tier 3-4 from FEATURES.md):**
- Agent discovery browser (search Agent Cards by skill before starting a conversation)
- Memory context display (show what agents remember — radical transparency, no competing app does this)
- Memory management UI (view/edit/delete archival facts per agent)
- Mixed human-agent thread creation (adding both humans and agents as equal participants)
- Sub-thread branching (long-press a message to branch a focused side conversation)
- Parallel block streaming visualization (two blocks rendering simultaneously in real time)
- Verified message indicators (Ed25519 checkmark per message with tap-for-details)
- Federated identity display (show which server each participant belongs to)
- Stream cancellation (stop generating button during active stream)

**Defer (v2+):**
- End-to-end encryption (protocol chose server-visible by design; incompatible with multi-agent threads where agents must read content)
- Voice/video messaging (out of scope per PROJECT.md; WebRTC complexity without protocol benefit)
- iPad/Mac Catalyst (iPhone-first; SwiftUI adaptation is easier later — do not design for it now)
- Extension block plugin registry (architecturally important to plan for; premature to build for v1)
- Push notifications (APNs requires server-side protocol extensions not yet specified)
- Read receipts, emoji reactions (not defined in AINM v1 protocol)
- Offline message composition queue (complexity around conflict resolution and stale context; show "offline" banner instead)

### Architecture Approach

The client follows a strict layered architecture: SwiftUI Views bind to ViewModels via `@Observable`, ViewModels coordinate Repositories, Repositories orchestrate NetworkService (WebSocket), PersistenceService (GRDB), and CryptoService (CryptoKit). The critical design invariant is "database as single source of truth": all server data is written to GRDB before any UI state updates. GRDB observation — not ViewModel mutation — is what drives SwiftUI re-renders. The only exception is streaming deltas, which are held in an in-memory `StreamBuffer` per stream_id and only collapsed to GRDB on `ainm.stream.end`. This matches the protocol's design: deltas are ephemeral and unsigned; only the final collapsed message is signed and canonical.

**Major components:**
1. **NetworkService** (singleton actor) — single WebSocket connection, JSON-RPC 2.0 framing via a JSONRPCCodec layer, receive loop that demultiplexes into typed AsyncStreams by method name, request/response correlation via `[String: CheckedContinuation]` map
2. **PersistenceService** (GRDB DatabaseQueue) — schema migrations, all CRUD operations, ValueObservation delivered on background queue to prevent main-thread fetches on large result sets
3. **CryptoService** — Ed25519 key generation/storage/signing/verification, custom RFC 8785 JCS canonicalization, init-signing for handshake (`"identity|timestamp"` format)
4. **Repository Layer** (MessageRepository, ThreadRepository, StreamingRepository, MemoryRepository, AgentRepository, IdentityRepository) — domain orchestrators combining network + persistence; expose AsyncStream for ViewModel observation
5. **ViewModel Layer** (ConversationListVM, ThreadVM, StreamingVM, AgentCardVM, MemoryVM, SettingsVM) — UI state, user actions, observe GRDB AsyncSequences, expose @Observable state to views
6. **SwiftUI Views** — zero business logic, pure rendering and user input capture; streaming text views are leaf nodes managing their own content to isolate delta updates from parent list re-renders

### Critical Pitfalls

1. **RFC 8785 JCS mismatch between Swift and Rust** — Swift has no JCS library. `JSONEncoder(.sortedKeys)` uses UTF-8 byte order; RFC 8785 requires UTF-16 lexicographic order for key sorting. These differ for characters above U+FFFF, causing intermittent signature failures with emoji and CJK content. Build a custom JCS implementation from the spec (~100 lines), validate against official test vectors at cyberphone/json-canonicalization, and run a cross-platform CI test (sign on iOS, verify on Rust server, and vice versa). Phase 1, Task 1 — everything else depends on this.

2. **WebSocket connection lifecycle on iOS** — iOS suspends background apps within 30 seconds, killing TCP connections. WiFi-to-cellular transitions silently invalidate the socket with no error callback. Fix: explicit state machine (`disconnected` → `connecting` → `handshaking` → `connected`), NWPathMonitor for proactive network-change reconnection, exponential backoff with jitter, and full ainm.initialize handshake replay on every reconnection. Never treat TCP open as protocol-ready. Never cache capabilities across sessions.

3. **SwiftUI message list memory collapse** — LazyVStack retains all created views in memory (not UITableView-style recycling). Memory exhaustion and crash occurs around 5,000 messages on iPhone 11 Pro Max. Pagination must be designed from day one: load the most recent 50 messages initially, trigger load-more on scroll-up, maintain a view window of ~200 maximum. `defaultScrollAnchor(.bottom)` with large datasets causes a blank window on initial load. Retrofitting pagination into a "load all" design is a full architectural rewrite.

4. **Streaming delta rendering starvation of the main thread** — At 50-100+ deltas/second from fast models, each delta triggering a SwiftUI state update starves the main thread. Fix: process WebSocket messages on a background actor, accumulate deltas in a buffer, flush to @MainActor at most every 16-33ms (30-60fps). Make streaming text views leaf nodes that manage their own `Text` content independently from the parent message list. For parallel block streaming, batch all block updates for a stream_id in a single UI transaction.

5. **Ed25519 private key loss or inaccessibility** — Ed25519 keys have no native Keychain `SecKey` representation; they must be stored as `kSecClassGenericPassword` raw bytes. Common mistakes: using `kSecAttrAccessibleWhenUnlocked` (makes the key unavailable when the device is locked, breaking background signing), not setting `kSecAttrSynchronizable: false` (iCloud Keychain syncs the key across devices, creating identity conflicts), key deletion on app uninstall with no recovery path. Use `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly` explicitly. Design a key recovery mechanism before shipping — without it, key loss equals permanent identity loss.

## Implications for Roadmap

Based on the dependency graph in ARCHITECTURE.md and the pitfall-to-phase mapping in PITFALLS.md, the following six-phase structure is recommended. The layering is strict: each phase depends on the one before it. Phases 1-3 are foundational; Phases 4-6 build differentiation on top.

### Phase 1: Foundation (Security + Network Core)

**Rationale:** Crypto compatibility and WebSocket lifecycle are the two highest-risk items in the project and block everything else. Validating these before any UI prevents late-stage rewrites. Five of seven critical pitfalls (JCS mismatch, WebSocket lifecycle, Ed25519 key storage, GRDB observation setup, ainm.initialize handshake) all map to Phase 1 failures.

**Delivers:** A working connection to the Rust demo server that can complete the ainm.initialize handshake and exchange signed, verified messages. All crypto and persistence infrastructure proven with tests before any UI is built.

**Addresses:** Ed25519 key generation and Keychain storage, JCS canonicalization, message signing and verification, WebSocket state machine, ainm.initialize / ainm.initialized handshake, GRDB schema and migrations, domain model types, JSONRPCCodec

**Avoids:** JCS mismatch (Pitfall 1), WebSocket lifecycle mismanagement (Pitfall 2), Ed25519 key storage mistakes (Pitfall 5), ainm.initialize handshake violations (Pitfall 7)

**Key deliverables:** Xcode project + SPM dependencies, GRDB schema + PersistenceService, custom JCS + RFC 8785 test vectors, CryptoService (keygen/sign/verify with cross-platform test), JSONRPCCodec (requests/responses/notifications), NetworkService (WebSocket + state machine + demux + handshake), integration test against demo server

### Phase 2: Core Messaging UI

**Rationale:** With network and crypto proven, wire up the basic messaging loop end-to-end. The message list pagination architecture must be established here — it cannot be retrofitted later without a full rewrite. GRDB observation scope and delivery queue must be co-designed with the message list.

**Delivers:** A usable messaging app: conversation list, thread view, send and receive text messages with Markdown rendering, code blocks, images, local persistence, offline reading, connection status indicator, and signature verification indicators.

**Addresses:** Conversation list with last-message preview, thread view (paginated LazyVStack, 50-message initial load), TextBlock rendering (MarkdownUI), CodeBlock rendering (HighlightSwift), ImageBlock rendering (Nuke), FileBlock display, fallback block rendering, optimistic send (status: sending/sent/failed), GRDB write-through cache, offline read from cache

**Avoids:** Message list performance collapse (Pitfall 3), GRDB observation overload (Pitfall 6)

**Key deliverables:** MessageRepository + ThreadRepository, ConversationListVM + ConversationListView (GRDB observation on background queue), ThreadVM + ThreadView (paginated, windowed, GRDB observation), ContentBlockRenderer (all 9 block types including fallback), signature verification checkmarks

### Phase 3: Streaming + Real-Time Features

**Rationale:** Streaming is the defining AI chat UX. The streaming pipeline — WebSocket delta events to throttled UI updates to collapsed persistence — has distinct complexity and must be isolated in a dedicated phase with explicit performance criteria.

**Delivers:** Token-by-token streaming agent responses with cursor animation, parallel block streaming visualization, stream cancellation, typing and processing indicators, presence display.

**Addresses:** ainm.stream.start/delta/end/cancel/error, parallel block streaming with different block_index values, presence updates (ainm.presence.update), pre-stream "thinking" indicator

**Avoids:** Streaming UI thread starvation (Pitfall 4), streaming collapse and persistence gap

**Key deliverables:** StreamingRepository + StreamBuffer (in-memory, background actor), throttled delta dispatch to @MainActor (16-33ms flush), StreamingVM with leaf-node architecture, stream lifecycle UI (placeholder, progressive text, cursor animation, collapse to permanent message), parallel block rendering, stream cancel button, presence indicator components

### Phase 4: AI-Native Interactive Features

**Rationale:** Interactive blocks and tool call visibility are the protocol's core differentiators over other AI chat products. They build on the content block renderer from Phase 2 and the message delivery pipeline from Phase 3. ActionBlocks are new thread messages (not mutations of the original), so the full send flow from Phase 2 is a prerequisite.

**Delivers:** Full interactive block suite, tool call visibility with lifecycle grouping, agent identity display, @-mention support, agent type badges.

**Addresses:** ButtonBlock rendering + ActionBlock send flow, FormBlock (all field types, client-side validation), TableBlock (horizontal scroll, row actions), ApprovalBlock (state machine: pending/approved/rejected/changes_requested), tool call events grouped by call_id (collapsible, progress bar, determinate/indeterminate), agent identity badge + Agent Card profile view, @-mention highlighting + composer autocomplete

**Avoids:** Interactive block action flow mistakes (form state lost on scroll, missing post-action disabled state, missing loading state on approval tap)

**Key deliverables:** All interactive block view components, ActionBlock message creation + CryptoService sign + send, Tool call visibility engine (lifecycle grouping, in-place status updates), Agent Card HTTP GET with ETag caching, @-mention rendering + autocomplete

### Phase 5: Advanced Features (Memory + Discovery + Threading)

**Rationale:** Memory transparency, agent discovery, and sub-thread branching are high-value differentiators that require the full messaging, streaming, and interactive infrastructure to be in place. These are also more speculative in UX — they should be built with room for design iteration after the core is validated.

**Delivers:** Memory context display (what agents remember), memory management UI (view/delete archival facts), agent discovery browser, mixed human-agent thread creation, sub-thread branching and navigation.

**Addresses:** ainm.memory.store/query/evict, context frames (ainm.context.frame notifications), Agent Card discovery with ETag caching, ainm.thread.create with mixed human + agent participants, sub-thread creation (parent_thread_id, parent_message_id), sub-thread navigation

**Key deliverables:** MemoryRepository + MemoryVM + MemoryView, AgentRepository + AgentDiscoveryVM + AgentDiscoveryView, thread creation UI with participant type display, sub-thread branching UI (long-press message), sub-thread indicator in parent thread

### Phase 6: Polish + Hardening

**Rationale:** This phase converts a functional app into a reference-quality app that can serve as the canonical iOS implementation of the AINM protocol. Deep linking, accessibility, performance hardening under production loads, and error UX polish are deferred here because they require the full feature set to be in place to audit properly.

**Delivers:** Deep linking, accessibility (VoiceOver, Dynamic Type), performance profiling under 2,000-message load, push notifications (APNs), offline outbound message queue hardening, error message mapping, and complete "Looks Done But Isn't" checklist verification.

**Addresses:** NavigationStack deep linking (`ainm://thread/{id}`, `ainm://thread/{id}/message/{id}`), VoiceOver labels for all interactive elements, Dynamic Type scaling, APNs integration + notification tap handler, offline message queue (store-and-forward with deduplication), key recovery mechanism design, error code to human-readable message mapping

**Key deliverables:** NavigationPath URL scheme handler, accessibility audit, Instruments profiling pass (60fps + <150MB memory budget for 2,000-message conversation), APNs server-side integration design, PITFALLS.md "Looks Done But Isn't" checklist as acceptance criteria, app store quality review

### Phase Ordering Rationale

- Phase 1 before everything: crypto interop and the WebSocket state machine are architectural foundations with catastrophic rewrite costs if deferred. PITFALLS.md explicitly marks JCS mismatch and WebSocket lifecycle as "never acceptable as technical debt shortcuts."
- Phase 2 before streaming (Phase 3): the ContentBlockRenderer and GRDB observation patterns established in Phase 2 are required by the streaming pipeline and interactive block renderers.
- Phase 3 before interactive (Phase 4): the ActionBlock send flow and message delivery pipeline from Phase 3 are the foundation for interactive block responses and tool call persistence.
- Phase 4 before advanced (Phase 5): agent discovery, memory display, and mixed thread creation all require complete interactive block rendering and identity resolution.
- Phase 6 last: polish and hardening are safe to defer; architectural mistakes are not. APNs in particular requires server-side protocol design work that is not yet specified.

### Research Flags

Phases likely needing per-task research during planning:

- **Phase 1 (JCS custom implementation):** No Swift JCS library exists. The UTF-16 key sorting edge cases (characters above U+FFFF, surrogate pairs) and ECMAScript number formatting (RYU algorithm) need careful implementation and validation. Research the exact cross-platform test harness setup before coding.
- **Phase 1 (URLSessionWebSocketTask behavior):** WiFi-to-cellular transition behavior and the exact timing of connection teardown during backgrounding are poorly documented. Recommend testing with Network Link Conditioner before committing to the reconnection strategy in NetworkService.
- **Phase 6 (APNs integration):** APNs requires server-side AINM protocol extensions (push token registration, server-to-APNs delivery, notification payload format) not yet defined in the spec. This needs protocol-level design before iOS implementation begins.

Phases with well-documented patterns (skip research-phase):

- **Phase 2 (GRDB + MVVM + Repository):** GRDB 7.10.0 is mature with extensive documentation. The MVVM + Repository pattern is well-established for SwiftUI. The observation and pagination strategy is fully designed in ARCHITECTURE.md.
- **Phase 3 (streaming throttling):** The streaming pipeline architecture is fully specified in AINM streaming.md. The throttling pattern (background actor, 16-33ms flush) is a standard Swift Concurrency pattern with clear implementation guidance.
- **Phase 4 (interactive blocks):** The interactive.md spec is complete. All block types (ButtonBlock, FormBlock, TableBlock, ApprovalBlock, ActionBlock) have defined schemas. SwiftUI rendering follows standard patterns.
- **Phase 5 (Agent Card discovery):** Agent Card HTTP GET with ETag caching is fully specified in agent-card.md. Standard URLSession conditional GET pattern.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All 8 SPM library versions verified on GitHub as of 2026-03-29. Built-in Apple APIs confirmed against WWDC 2024/2025 sessions. Custom JCS and UUIDv7 are well-specified (RFC 8785, RFC 9562). No speculative choices. |
| Features | HIGH | Derived directly from the complete AINM protocol spec (content.md, interactive.md, streaming.md, etc.). Feature dependency graph is fully mapped. MVP tiers are grounded in both protocol requirements and iOS messaging app conventions. |
| Architecture | HIGH | GRDB + MVVM + Repository is proven for production iOS apps (Monzo, PointFree recommendations). Data flow patterns are directly derived from protocol design (one WebSocket, demultiplexed streams, write-through cache). Component boundaries are concrete and match the protocol's module structure. |
| Pitfalls | HIGH | Critical pitfalls are grounded in Apple's documented platform constraints (Keychain accessibility attributes, URLSessionWebSocketTask single-shot receive, LazyVStack memory retention) and RFC 8785 spec analysis. Performance numbers (5,000-message crash threshold, 16ms frame budget) are from Apple's own SwiftUI documentation and community profiling data. |

**Overall confidence:** HIGH

### Gaps to Address

- **JCS Swift implementation correctness:** The custom RFC 8785 implementation is the single highest-risk technical unknown. The gap closes in Phase 1 by running the official cyberphone/json-canonicalization test vectors and a cross-platform signing integration test (sign on iOS, verify on Rust server, and vice versa). If the implementation is wrong, the feedback is immediate and fixable before any other feature is built.

- **APNs protocol design:** Push notifications require server-side AINM protocol extensions for push token registration and server-to-APNs delivery that are not yet specified. Deferred to Phase 6; flag for protocol-level design work before iOS implementation begins.

- **KeychainAccess 4.2.2 Swift 6 compliance:** The library was released in 2020 and has not been updated for Swift 6 strict concurrency. If Sendable warnings are blockers, the fallback is a thin custom wrapper around the Security framework (~100 lines). Assess in Phase 1 when CryptoService is built; do not block on this.

- **MarkdownUI maintenance mode risk:** MarkdownUI 2.4.1 is stable but the author is working on a successor (Textual, currently v0.3.1, too immature for production). If Textual reaches 1.0 during development, evaluate migration. Low probability of breaking impact within the v1 timeframe.

- **Offline outbound queue deduplication:** The client-side outbound queue for messages sent while offline needs an idempotency key strategy to prevent double-send on retry after reconnection. The server-side store-and-forward (transport.md 4.5) handles delivery to offline recipients; the client side needs its own deduplication. Design this in Phase 2 alongside the message send flow.

- **HighlightSwift performance at scale:** HighlightSwift uses JavaScriptCore (highlight.js) under the hood. Performance with many simultaneous code blocks in a single conversation is untested. Monitor during Phase 2; if sluggish, fall back to static coloring.

## Sources

### Primary (HIGH confidence)
- AINM protocol spec: content.md, interactive.md, streaming.md, threading.md, transport.md, security.md, identity.md, memory.md, agent-card.md, tool-visibility.md — direct protocol requirements and contract definitions
- AINM demo server: demo/src/signing.rs, demo/src/protocol.rs, demo/src/streaming.rs — reference implementations (integration targets for iOS)
- Apple Developer Documentation: CryptoKit Curve25519.Signing, Storing CryptoKit Keys in Keychain, URLSessionWebSocketTask, NWPathMonitor, Creating Performant Scrollable Stacks — platform constraints and APIs
- GRDB.swift v7.10.0 (GitHub, Feb 15 2026), GRDBQuery v0.11.0 (GitHub) — database API, observation patterns, SwiftUI integration
- Nuke v12.9.0 (GitHub), swift-collections v1.4.1 (GitHub, Mar 2026) — image loading, data structures
- RFC 8785 (JSON Canonicalization Scheme), RFC 9562 (UUIDv7) — implementation specifications for custom components

### Secondary (MEDIUM confidence)
- [PointFree SharingGRDB](https://www.pointfree.co/blog/posts/168-sharinggrdb-a-swiftdata-alternative) — GRDB as SwiftData alternative, production iOS patterns including background write architecture
- [Stream Chat SwiftUI SDK — AI Integrations](https://getstream.io/chat/docs/sdk/ios/ai-integrations/overview/) — StreamingMessageView and AITypingIndicatorView patterns in production SwiftUI
- [Donny Wals — WebSocket AsyncStream](https://www.donnywals.com/iterating-over-web-socket-messages-with-async-await-in-swift/) — URLSessionWebSocketTask wrapped in AsyncStream pattern
- [Smashing Magazine — Designing for Agentic AI](https://www.smashingmagazine.com/2026/02/designing-agentic-ai-practical-ux-patterns/) — Agentic UX patterns for control, consent, and tool visibility
- [A2UI.org](https://a2ui.org/) — Google's declarative agent UI spec; corroborates AINM interactive block design choices
- [Approachable Concurrency in Swift 6.2](https://www.avanderlee.com/concurrency/approachable-concurrency-in-swift-6-2-a-clear-guide/) — Swift actor isolation patterns

### Tertiary (LOW confidence — informational, needs validation during implementation)
- [Apple Study on AI Agent Interaction](https://9to5mac.com/2026/02/12/apple-study-looks-into-how-people-expect-to-interact-with-ai-agents/) — User expectations for agent UX on iOS (consumer research, not technical spec)
- [RFC 8785 cyberphone/json-canonicalization test vectors](https://github.com/cyberphone/json-canonicalization) — reference test suite for JCS validation (test data, not implementation)
- [HighlightSwift](https://github.com/appstefan/HighlightSwift) — JavaScriptCore performance at scale is untested; monitor during Phase 2 before committing

---
*Research completed: 2026-03-29*
*Ready for roadmap: yes*
