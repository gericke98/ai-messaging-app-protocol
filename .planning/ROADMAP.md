# Roadmap: AI-Native Messaging Protocol

## Milestones

- ✅ **v3.1 AI-Native Messaging Protocol v1 Specification & Demo** — Phases 1-6 (shipped 2026-03-29) — [Archive](milestones/v3.1-ROADMAP.md)
- ✅ **v3.2 Federation** — Phases 7-9 (shipped 2026-03-29) — [Archive](milestones/v3.2-ROADMAP.md)
- 🚧 **v3.3 iOS Reference Client** — Phases 10-13 (in progress)

## Phases

<details>
<summary>v3.1 AI-Native Messaging Protocol v1 Specification & Demo (Phases 1-6) - SHIPPED 2026-03-29</summary>

See [milestones/v3.1-ROADMAP.md](milestones/v3.1-ROADMAP.md)

</details>

<details>
<summary>v3.2 Federation (Phases 7-9) - SHIPPED 2026-03-29</summary>

See [milestones/v3.2-ROADMAP.md](milestones/v3.2-ROADMAP.md)

</details>

### v3.3 iOS Reference Client

- [x] **Phase 10: Foundation** - Crypto, networking, persistence infrastructure validated against Rust server (completed 2026-03-30)
- [x] **Phase 11: Core Messaging UI** - Conversation list, message threads, content blocks, offline access (completed 2026-03-30)
- [ ] **Phase 12: Streaming & Interactive Blocks** - Real-time streaming, interactive block suite, action responses
- [ ] **Phase 13: AI-Native Features & Platform** - Tool calls, memory, agent discovery, sub-threads, deep linking

## Phase Details

### Phase 10: Foundation
**Goal**: App connects to the AINM Rust server, completes the protocol handshake, and exchanges Ed25519-signed messages with verified JCS canonicalization — all infrastructure proven before any UI
**Depends on**: Phase 9 (v3.2 complete — working Rust server as integration target)
**Requirements**: NET-01, NET-02, NET-03, NET-04, NET-05, NET-06, PLT-01
**Success Criteria** (what must be TRUE):
  1. App connects to the Rust demo server via WebSocket and completes the ainm.initialize/ainm.initialized handshake — verified by receiving the server's capability list
  2. App signs a message with Ed25519 (Apple CryptoKit) using RFC 8785 JCS canonicalization, and the Rust server successfully verifies the signature — cross-platform crypto proven
  3. A message signed by the Rust server is received by the app and its Ed25519 signature is verified locally — bidirectional signing works
  4. User can enter server URL and display name during onboarding, and the app generates and stores an Ed25519 keypair in iOS Keychain that persists across app restarts
  5. Messages, threads, and participants received from the server are persisted to GRDB and survive app termination — the local database is the single source of truth for UI
**Plans**: 5 plans
Plans:
- [x] 10-01-PLAN.md -- Xcode project setup, protocol models, JCS canonicalizer, base64url
- [x] 10-02-PLAN.md -- Ed25519 KeyManager, MessageSigner, cross-platform test vectors
- [x] 10-03-PLAN.md -- GRDB persistence layer (AppDatabase, migrations, record types)
- [x] 10-04-PLAN.md -- WebSocket networking, AINMClient protocol handshake, AppState
- [x] 10-05-PLAN.md -- Onboarding and Connection Success UI, GitHub Actions CI

### Phase 11: Core Messaging UI
**Goal**: Users can browse conversations, read message threads with rich content blocks, send text messages, and access cached messages offline — a functional messaging app
**Depends on**: Phase 10
**Requirements**: MSG-01, MSG-02, MSG-03, MSG-04, MSG-05, MSG-06, MSG-07, MSG-08, MSG-09, PLT-02, PLT-03
**Success Criteria** (what must be TRUE):
  1. User sees a conversation list with thread titles, last message previews, and timestamps — populated from GRDB observation, updating in real-time as new messages arrive
  2. User taps a conversation and sees a paginated message thread (most recent 50 messages, load-more on scroll-up) with participant avatars distinguishing humans from agents
  3. User can compose and send a text message that appears in the thread, is signed with Ed25519, and is received by other participants via the server
  4. Messages containing multiple content blocks (text + code + image) render in sequence — CodeBlocks show syntax highlighting with copy button, ImageBlocks show with tap-to-fullscreen, FileBlocks show filename/size with download
  5. User can kill the app, relaunch without network, and browse previously loaded conversations and messages from the local GRDB cache
**Plans**: 4 plans
Plans:
- [x] 11-01-PLAN.md -- Data layer: ContentBlock enum, repositories, DB migration, navigation rewiring
- [x] 11-02-PLAN.md -- Conversation list screen with GRDB observation, shared UI components
- [x] 11-03-PLAN.md -- Content block renderers: text, code, image, file, syntax highlighter
- [x] 11-04-PLAN.md -- Message thread, composer, new thread creation, full messaging flow
**UI hint**: yes

### Phase 12: Streaming & Interactive Blocks
**Goal**: Agent responses stream token-by-token in real-time and users can interact with rich UI blocks (buttons, forms, tables, approvals) that send responses back through the protocol
**Depends on**: Phase 11
**Requirements**: STR-01, STR-02, STR-03, STR-04, INT-01, INT-02, INT-03, INT-04, INT-05
**Success Criteria** (what must be TRUE):
  1. When an agent streams a response, tokens appear one-by-one with a cursor animation at 30-60fps — no main thread starvation even at 100+ deltas/second
  2. User can tap "Stop Generating" during an active stream and the agent stops — the partial response is preserved in the thread
  3. User taps a ButtonBlock and the app sends a signed ActionBlock response as a new thread message; FormBlocks validate input client-side before submission; ApprovalBlocks cycle through pending/approved/rejected states
  4. TableBlocks render with horizontal scroll for wide data and row-level action buttons; unrecognized block types display their fallback text instead of crashing
  5. Typing/processing indicators show for both humans and agents, and parallel block streaming (two blocks receiving deltas simultaneously) renders correctly
**Plans**: TBD
**UI hint**: yes

### Phase 13: AI-Native Features & Platform
**Goal**: The app showcases AINM's differentiating features — tool call transparency, memory context, agent discovery, sub-threading, and platform integration — delivering a reference-quality iOS client
**Depends on**: Phase 12
**Requirements**: AI-01, AI-02, AI-03, AI-04, AI-05, PLT-04, PLT-05
**Success Criteria** (what must be TRUE):
  1. When an agent executes tool calls, the user sees collapsible indicators grouped by call_id showing the started/progress/completed lifecycle — tool execution is transparent, not hidden
  2. User can @-mention agents and humans in the composer with autocomplete, and mentions are highlighted in rendered messages
  3. User can browse an agent discovery view showing Agent Cards with skills, tools, and capabilities — and start a conversation with a discovered agent
  4. User can view active memory context frames for an agent in a thread, search archival memory facts, and delete specific facts — memory is transparent and user-controlled
  5. User can branch a sub-thread from any message (one level per spec), navigate between parent and sub-thread, and open a specific thread/message via deep link URL
**Plans**: TBD
**UI hint**: yes

## Progress

**Execution Order:**
Phases execute in numeric order: 10 -> 11 -> 12 -> 13

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 10. Foundation | v3.3 | 5/5 | Complete    | 2026-03-30 |
| 11. Core Messaging UI | v3.3 | 4/4 | Complete   | 2026-03-30 |
| 12. Streaming & Interactive Blocks | v3.3 | 0/0 | Not started | - |
| 13. AI-Native Features & Platform | v3.3 | 0/0 | Not started | - |
