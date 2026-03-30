# Phase 11: Core Messaging UI - Context

**Gathered:** 2026-03-29
**Status:** Ready for planning

<domain>
## Phase Boundary

Users can browse conversations, read message threads with rich content blocks (text, code, image, file), send text messages, and access cached messages offline — a functional messaging app. Requirements: MSG-01 through MSG-09, PLT-02, PLT-03.

</domain>

<decisions>
## Implementation Decisions

### Message Thread Design
- **D-01:** Hybrid message layout — chat bubbles for human messages (right-aligned accent color), flat/wide layout for agent messages (left-aligned, full width for rich content like code blocks and tool calls).
- **D-02:** Agent badge: icon + type label next to agent name (e.g., `🤖 ResearchBot │Agent│`). Human messages show name with person icon. Agent Card data populates the badge. (MSG-09)
- **D-03:** Consecutive messages from same sender are grouped — show avatar + name only on first message in sequence, tighter spacing for subsequent messages, timestamp on last message only.
- **D-04:** New thread creation via direct compose — tap '+' to open composer with 'To:' field for @-mention autocomplete of participants (humans and agents). Thread created on first message send via `thread.create` protocol message.

### Conversation List Layout
- **D-05:** Standard messaging row: avatar (human 👤 or agent 🤖), thread title, last message preview (truncated), relative timestamp, unread badge. (MSG-01)
- **D-06:** Sort by most recent activity — conversations with newest messages float to top. No pinning in v1.
- **D-07:** Empty state: centered SF Symbol icon, "No conversations yet" message, prominent "Start a Conversation" CTA button.

### Content Block Rendering
- **D-08:** CodeBlocks: native AttributedString + regex-based syntax highlighting for ~10 common languages. Language label top-left, copy button top-right. Monospace font, dark background. No third-party dependency. (MSG-05)
- **D-09:** ImageBlocks: aspect-fit scaling to full message width, capped at 300pt height. Tap opens fullscreen with pinch-to-zoom. Server-provided dimensions pre-set frame to avoid layout jumps. (MSG-06)
- **D-10:** Multi-block messages render as vertical stack, each block gets full available width. Natural top-to-bottom reading flow. (MSG-04)

### Composer & Navigation
- **D-11:** NavigationStack only — conversation list as root, push to thread detail. No tab bar. Agent discovery and settings accessible from nav bar buttons.
- **D-12:** Simple composer: auto-growing TextField at bottom, send button on right. Plain text input — Markdown rendered in sent messages but not previewed while typing. Auto-grows to ~4 lines max.
- **D-13:** Connection status as subtle nav bar subtitle under "Conversations" title. Shows 🟢 Connected / 🟡 Reconnecting... / 🔴 Offline. Disappears when connected for 5+ seconds. (Phase 10 D-02 deferred item)
- **D-14:** Light/dark mode follows system — use SwiftUI semantic colors throughout. No in-app toggle for v1. (PLT-03)

### Claude's Discretion
- FileBlock download UX design (filename, size display, download action) — Claude picks best iOS pattern (MSG-07)
- Markdown rendering approach for text blocks (MSG-03) — AttributedString or swift-markdown, Claude decides
- Pagination implementation for message thread (50 messages, load-more on scroll-up) — per success criteria
- Offline mode UX details (PLT-02) — how to indicate cached vs live data
- Unread badge logic — how unread count is tracked and cleared

### Folded Todos
None.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Protocol Specification (Message & Content)
- `spec/content.md` — Content block types (TextBlock, CodeBlock, ImageBlock, FileBlock), multi-block messages, rendering rules
- `spec/threading.md` — Thread creation, message ordering, sub-threading model, participant management
- `spec/transport.md` — WebSocket transport, JSON-RPC 2.0 framing, message sending/receiving operations
- `spec/identity.md` — Email-like identity model, participant types (human vs agent)
- `spec/agent-card.md` — Agent Card structure for agent display and discovery (MSG-09)

### Schemas
- `schemas/content.schema.json` — JSON Schema for all content block types (TextBlock, CodeBlock, ImageBlock, FileBlock)
- `schemas/threading.schema.json` — JSON Schema for thread and message structures
- `schemas/common.schema.json` — Shared type definitions (MessageEnvelope, ContentBlockBase)

### AsyncAPI
- `asyncapi.yaml` — Transport bindings, channel definitions for message send/receive operations

### Existing iOS Codebase (Phase 10 foundation)
- `ios/AINMMessenger/AINMMessenger/App/AppState.swift` — @MainActor @Observable root state, connection lifecycle
- `ios/AINMMessenger/AINMMessenger/Persistence/AppDatabase.swift` — GRDB DatabasePool, migration system
- `ios/AINMMessenger/AINMMessenger/Persistence/ThreadRecord.swift` — Thread GRDB record with associations
- `ios/AINMMessenger/AINMMessenger/Persistence/MessageRecord.swift` — Message GRDB record, contentJson as JSON text
- `ios/AINMMessenger/AINMMessenger/Persistence/ParticipantRecord.swift` — Participant GRDB record
- `ios/AINMMessenger/AINMMessenger/Models/MessageEnvelope.swift` — Protocol message model, ContentBlock (text-only, needs expansion)
- `ios/AINMMessenger/AINMMessenger/Networking/AINMClient.swift` — WebSocket client with JSON-RPC framing
- `ios/AINMMessenger/AINMMessenger/UI/OnboardingView.swift` — Established SwiftUI patterns (@Bindable, semantic colors, 44pt touch targets)

### Technology Stack
- `CLAUDE.md` §iOS Client Stack — SwiftUI, GRDB.swift, Apple CryptoKit, MVVM + Repository, @Observable

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AppState` (@MainActor @Observable) — extend with conversation/thread state properties
- `AppDatabase` with GRDB migrations — add new migrations for any schema changes (indexes, computed columns)
- `ThreadRecord`, `MessageRecord`, `ParticipantRecord` — GRDB records with associations already defined
- `MessageEnvelope` + `ContentBlock` models — expand ContentBlock for code, image, file block types
- `OnboardingView` — established SwiftUI patterns to follow (spacing, button styles, error handling)
- `AINMClient` — extend with message send/receive, thread operations

### Established Patterns
- Write-through cache: all server data flows through GRDB before UI (v3.3 init decision)
- MVVM + Repository: @Observable ViewModels, Repository pattern for data access
- Swift 6 strict concurrency: @MainActor on state, Sendable records, async/await throughout
- GRDB DatabasePool: concurrent reads, serialized writes, WAL mode
- JSON-RPC 2.0 framing for all protocol operations

### Integration Points
- `AINMClient.sendMessage()` — already exists for sending MessageEnvelope
- GRDB observation (`ValueObservation`) for live-updating conversation list and thread views
- NavigationStack from `AINMMessengerApp.swift` — currently routes onboarding → connection success, needs conversation list as post-auth root
- ContentBlock stored as JSON text column — decode into typed block variants for rendering

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

*Phase: 11-core-messaging-ui*
*Context gathered: 2026-03-29*
