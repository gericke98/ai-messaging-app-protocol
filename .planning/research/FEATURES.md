# Feature Landscape

**Domain:** iOS reference client for AI-native messaging protocol (AINM)
**Researched:** 2026-03-29
**Overall confidence:** HIGH (features derived primarily from existing protocol spec + established iOS messaging patterns)

## Table Stakes

Features users expect in any messaging app. Missing = the app feels broken, not just incomplete.

### Core Messaging

| Feature | Why Expected | Complexity | Protocol Dependency | Notes |
|---------|--------------|------------|---------------------|-------|
| **Conversation list** | Every messaging app has one. Users need to see and navigate all threads. | Med | `ainm.thread.create`, thread metadata (threading.md 3.3) | Must show thread title, last message preview, timestamp, unread count, participant avatars. Mixed human-agent threads should show agent avatars distinctly (badge or icon treatment). |
| **Message thread view** | The primary interaction surface. Chronological, scrollable message history. | Med | `ainm.message.deliver`, threading.md flat model | LazyVStack in SwiftUI. Must handle hundreds of messages without jank. Time-ordered by UUIDv7 timestamps. |
| **Text message rendering (Markdown)** | ChatGPT, Claude, and every AI chat app renders Markdown. Users expect bold, italic, links, lists, headers. | Med | content.md TextBlock with CommonMark | Need a CommonMark parser for SwiftUI. Options: swift-markdown (Apple), Down, or MarkdownUI. Must handle inline code, block quotes, links, lists. |
| **Send text messages** | Users must be able to type and send messages. | Low | `ainm.message.send` with TextBlock content | Standard message composer with send button. Must include Ed25519 signing before send. |
| **Code block rendering with syntax highlighting** | Agents produce code constantly. Unhighlighted code is unreadable. | Med | content.md CodeBlock (`language`, `code`, `filename`) | Needs a syntax highlighting library. Splash or Highlightr for SwiftUI. Must show language label and optional filename. Copy-to-clipboard button is expected. |
| **Image display** | Users share screenshots, agents generate diagrams. | Low | content.md ImageBlock (HTTPS URL, mime_type, alt, width/height) | AsyncImage or custom loader. Must respect width/height for pre-layout. Tap to fullscreen. HTTPS-only validation. |
| **File attachment display** | Users share documents, agents produce artifacts. | Low | content.md FileBlock (URL, filename, size_bytes, digest) | Show filename, size, mime type icon. Download on tap. SHA-256 digest verification after download. |
| **Multi-block messages** | Agents routinely send text + code + text in one message. Must render all blocks in order. | Low | content.md Section 6 (block ordering) | Already the protocol's native format. Render `content[]` array in order. Each block gets its own renderer by `type`. |
| **Streaming responses (token-by-token)** | ChatGPT and Claude set this expectation. Watching tokens appear is THE AI chat UX. | High | streaming.md: `ainm.stream.start`, `.delta`, `.end` | Block layout declared upfront, deltas arrive per block index. Must render text progressively with cursor/caret animation. Handle parallel block streaming (two blocks receiving deltas simultaneously). Collapse to signed message on stream-end. Stream Chat SwiftUI SDK demonstrates this pattern exists in production SwiftUI apps. |
| **Typing / processing indicators** | Every messaging app shows when someone is typing. For agents, "processing" is the equivalent. | Low | threading.md presence + `ainm.presence.update`/`.notify` | Protocol defines thread-scoped presence with states. Show animated dots or similar for human typing, distinct "thinking" indicator for agents (e.g., pulsing brain icon). |
| **Connection status** | Users must know if they are online, reconnecting, or offline. | Low | transport.md WebSocket lifecycle, reconnection (Section 4.5) | Banner or subtle indicator. "Connecting...", "Connected", "Reconnecting...". Must handle WiFi-to-cellular transitions gracefully via Network.framework. |
| **Pull-to-refresh / load earlier messages** | Users scroll up and expect to load history. | Low | `ainm.memory.query` with `tier: "conversation"` and time_range | Paginate conversation memory. Server handles the query, client renders results above existing messages. |

### AI-Native Essentials

| Feature | Why Expected | Complexity | Protocol Dependency | Notes |
|---------|--------------|------------|---------------------|-------|
| **Tool call visibility (collapsible indicators)** | The protocol's transparency principle. Users see what agents are doing. No other consumer app does this well -- but the spec demands it. | Med | tool-visibility.md: `tool.call.started`, `.progress`, `.completed` | Collapsed by default: single line showing tool name + status. Expandable to show input params, progress bar (determinate/indeterminate), result summary, duration. Group events by `call_id`. Must update in-place as lifecycle progresses. |
| **Interactive buttons** | Agents need to offer actions beyond text. Buttons are the simplest interactive element. | Med | interactive.md ButtonBlock (`action_id`, `label`, `style`, `disabled`) | Render with semantic styles (primary/secondary/danger). On tap, send ActionBlock message. Respect `disabled` state. |
| **Interactive forms** | Agents ask structured questions -- filling a form is better than free-text for things like configuration, preferences, booking. | High | interactive.md FormBlock (`fields[]` with text/number/select/checkbox/date types, validation) | Must render each field_type: text input, number stepper, dropdown select, checkbox toggle, date picker. Client-side validation (required, min/max, pattern). Submit sends ActionBlock with field values. |
| **Data tables** | Agents present structured results (search results, comparisons, metrics). | Med | interactive.md TableBlock (`columns[]`, `rows[][]`, optional `row_actions[]`) | Horizontal scroll for wide tables. Row actions render as buttons per row. Column headers with type hints. |
| **Approval workflows** | Agents request permission before taking irreversible actions. Critical for trust. | Med | interactive.md ApprovalBlock (state machine: pending -> approved/rejected/changes_requested) | Visual state machine: pending shows approve/reject/request-changes buttons. Transition sends ActionBlock. Show current state, who acted, when. Immutable after terminal state. |
| **Agent identity display** | Users must know WHO they are talking to -- especially whether it is a human or an agent. | Low | identity.md (type: human/agent), agent-card.md (name, description) | Agent messages get a badge/label. Profile tap shows agent card info: name, description, skills, tools. Human messages show standard avatar. |
| **Fallback rendering for unknown blocks** | The protocol guarantees graceful degradation. Client MUST render `fallback` for unrecognized types. | Low | content.md Section 5 (extension blocks, fallback rule) | If block `type` is unrecognized or `version` MAJOR is too high, render `fallback` string. Style it distinctly (e.g., gray background, "simplified view" label). |
| **@-mention support** | Users mention agents to invoke them. Agents mention users in responses. Spec requires it. | Med | content.md TextBlock `mentions[]` (address, offset, length) | Highlight mention text in messages. Tap navigates to profile. Composer needs autocomplete for participant addresses. |
| **Message signing (Ed25519)** | Non-negotiable security requirement. Every message is signed. | Med | security.md Section 4, Apple CryptoKit Curve25519.Signing | Sign messages before send. Verify signatures on received messages. Show verification status (checkmark for verified, warning for failed). Key stored in Keychain via `kSecClassGenericPassword`. |

### Platform Essentials

| Feature | Why Expected | Complexity | Protocol Dependency | Notes |
|---------|--------------|------------|---------------------|-------|
| **Offline message access** | Users open the app on the subway and expect to read past messages. | Med | Local persistence via GRDB.swift | Cache messages, threads, participants locally. Show cached data immediately, sync when online. Store-and-forward (transport.md 4.5) delivers missed messages on reconnect. |
| **Push notifications** | Users expect to know about new messages without having the app open. | Med | Apple Push Notification service (APNs) + server support | Server sends push when client is disconnected. Must handle notification tap -> open correct thread. Not fully defined in protocol spec yet -- will need server-side APNs integration. |
| **Dark mode** | iOS standard since iOS 13. Not supporting it feels broken. | Low | None (pure client) | SwiftUI handles most of this automatically with semantic colors. Must ensure code blocks, tool indicators, and interactive elements respect dark mode. |
| **Accessibility** | VoiceOver, Dynamic Type, reduced motion. Required for App Store quality. | Med | None (pure client) | All interactive elements need accessibility labels. Content block `alt` text (images) and `fallback` text feed into VoiceOver. Streaming text needs proper accessibility announcements. |
| **Deep linking** | Tap a notification or shared link -> open specific thread/message. | Med | NavigationStack + NavigationPath | URL scheme: `ainm://thread/{thread_id}` or `ainm://thread/{thread_id}/message/{message_id}`. |

## Differentiators

Features that set this app apart from ChatGPT, Claude, Slack bots, and other AI chat experiences. Not expected, but high-value.

| Feature | Value Proposition | Complexity | Protocol Dependency | Notes |
|---------|-------------------|------------|---------------------|-------|
| **Agent discovery browser** | Browse available agents, see their capabilities, skills, and tools BEFORE starting a conversation. No consumer app does this well. | Med | agent-card.md: Agent Card fetch, skills, tools, capabilities | Profile-style view per agent: name, description, skill tags, tool list, streaming/threading capabilities. Search/filter by skill tags. Start conversation from agent card. ETag caching for efficiency. |
| **Memory context display** | Show users what the agent "remembers" -- active context, conversation summary, relevant archival facts. Radical transparency into AI cognition. | High | memory.md: context frames, three-tier model (active/conversation/archival) | Drawer or panel showing: recent context frame contents, conversation summary, relevant archival facts. Let users see what agents know about them. Unique differentiator -- no existing app exposes this. |
| **Memory management UI** | Let users view, edit, and delete what agents remember about them. Data sovereignty over AI memory. | High | `ainm.memory.store`, `ainm.memory.query`, `ainm.memory.evict` | Settings-style interface: list archival facts per agent, delete individual facts, see when facts were stored. Empowers users. GDPR-friendly by design. |
| **Mixed human-agent thread creation** | Create a thread and invite both humans and agents as equal participants. The core differentiator of the entire protocol. | Med | `ainm.thread.create` with initial participants (human + agent addresses) | Thread creation UI: add participants by address, see type (human/agent) before creating. Agent participants get capability preview from Agent Card. |
| **Sub-thread branching** | Long-press a message to branch a focused side conversation. One level only per spec. | Med | threading.md Section 4 (sub-threads, parent_thread_id, parent_message_id) | Branch UI: select message, create sub-thread with inherited participants. Sub-thread appears as nested indicator in parent thread. Navigate between parent and sub-thread. |
| **Parallel block streaming visualization** | When an agent streams into multiple blocks simultaneously (e.g., explanation + code), show both blocks rendering in real-time. | High | streaming.md parallel streaming (`ainm.stream.delta` with different `block_index` values) | Most AI apps stream one block at a time. Parallel streaming is visually impressive and demonstrates agent sophistication. Layout must handle two blocks growing simultaneously without scroll jump. |
| **Tool call timeline** | Aggregate view of all tool calls in a thread -- like a developer console audit log. | Med | tool-visibility.md: all tool events in thread | Filterable list: tool name, status, duration, timestamp. Tap to see details. Useful for power users debugging agent behavior. |
| **Federated identity display** | Show which server a participant belongs to. `alice@server-a.com` and `bot@server-b.com` in the same thread -- make the federation visible and tangible. | Low | identity.md address format (name@provider) | Subtle provider label under participant name. Different servers get different accent colors or icons. Makes federation real to users. |
| **Verified message indicators** | Show cryptographic verification status on each message. Green checkmark = signature verified. | Low | security.md Ed25519 signing + verification | Verify signature on display. Show small checkmark icon. Tap for verification details (signer address, algorithm). Unique among messaging apps. |
| **Stream cancellation** | User can cancel a streaming response mid-generation. | Low | streaming.md `ainm.stream.cancel` (client -> server request) | "Stop generating" button during active stream. Sends cancel request. Server forwards to agent. Stream ends with status "cancelled". Already fully spec'd in protocol. |
| **Extension block registry** | Register custom block renderers for extension types (reverse-domain namespaced). Future-proofs for third-party content types. | High | content.md Section 5 (extension blocks) | Plugin architecture for custom SwiftUI views mapped to extension type strings. v2 feature but architecturally important to plan for now. |
| **Haptic feedback on interactions** | Subtle haptics on button taps, form submissions, approval actions. iOS-native polish that web-wrapper apps cannot match. | Low | None (pure client) | UIImpactFeedbackGenerator for button actions. UINotificationFeedbackGenerator for approvals (success/error). Small touch that differentiates native quality. |

## Anti-Features

Features to explicitly NOT build. Each has a clear rationale.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **Voice/video messaging** | Out of scope for v1 per PROJECT.md. Text and rich content only. Massive complexity (WebRTC, codec support, recording UI) for no protocol benefit. | Focus on text, code, interactive blocks, streaming. Voice/video is a v2+ consideration. |
| **Agent marketplace / store** | Out of scope per PROJECT.md. Discovery yes, marketplace no. A marketplace requires payment, curation, review -- all non-trivial and premature. | Build agent discovery browser showing Agent Cards. Users add agents by address manually or from search. No commerce layer. |
| **End-to-end encryption** | Protocol explicitly chose server-visible messages (needed for agent processing, search, context management). E2EE is a v2 feature per STACK.md. Signal Protocol is incompatible with multi-agent threads where agents must read content. | TLS 1.3 for transport. Ed25519 signing for integrity and authenticity. Not confidentiality from server. |
| **Custom themes / skins** | Premature customization. Focus on one polished visual identity. Theme engines add complexity to every view for marginal benefit. | Support light/dark mode only. Use SwiftUI semantic colors throughout. Consistent visual language builds brand identity. |
| **Chat bots via slash commands** | This is the old paradigm (Slack `/command`). AINM agents are full participants, not command responders. Slash commands undermine the "agents are equals" philosophy. | Agents are @-mentioned or participate naturally in threads. No slash command parsing needed. |
| **Read receipts** | Not defined in AINM v1 protocol. Adds privacy concerns. Users in AI-native messaging care about responses, not read status. | Show presence/typing indicators instead. Agent presence (online/processing/idle) is more useful than "read at 3:42 PM". |
| **Message reactions (emoji)** | Not defined in AINM v1 protocol. Adding client-only reactions creates protocol divergence. Premature feature that does not serve the AI-native use case. | Wait for protocol spec to define reactions if needed. Text replies serve the same purpose. Focus on interactive blocks for structured feedback. |
| **Offline message composition queue** | Over-engineering for v1. Composing messages offline and queuing them adds complexity (conflict resolution, signature timing, stale context). | Show "you are offline" banner. Disable send button. Let store-and-forward handle messages for offline recipients (the other side). |
| **iPad / Mac Catalyst** | Scope creep. iPhone reference client first. iPad layout (split view) is a different design problem requiring different navigation patterns. | Target iPhone only for v1. SwiftUI makes iPad adaptation easier later but do not design for it now. |
| **Self-hosting tools / server bundling** | Out of scope per PROJECT.md. The iOS app connects to existing AINM servers, it does not bundle one. | Document server connection settings. User enters server URL during onboarding. No embedded server. |
| **ChartBlock rendering** | Explicitly excluded from protocol spec (noted in PROJECT.md tech debt as deliberate exclusion). | Use TableBlock for data display. Charts can be added as extension blocks later if ChartBlock is spec'd. |

## Feature Dependencies

Dependencies between features, showing build order constraints.

```
WebSocket Connection Manager
  |-> ainm.initialize handshake
  |     |-> Capability negotiation (streaming, memory, etc.)
  |     |-> Session establishment
  |-> All message send/receive
  |-> Streaming event delivery
  |-> Presence updates

Ed25519 Key Management (Keychain)
  |-> Message signing (every outgoing message)
  |-> Signature verification (every incoming message)
  |-> Identity display (verified badges)

Identity Resolution
  |-> Participant display (name, type, provider)
  |-> Agent Card fetch
  |     |-> Agent discovery browser
  |     |-> Agent capability display in profiles
  |-> @-mention autocomplete

Content Block Rendering Engine
  |-> TextBlock (Markdown) renderer
  |-> CodeBlock (syntax highlight) renderer
  |-> ImageBlock renderer
  |-> FileBlock renderer
  |-> Fallback renderer (unknown types)
  |-> Extension block plugin architecture (future)

Interactive Block Engine (depends on Content Block Engine + ActionBlock sender)
  |-> ButtonBlock renderer + ActionBlock sender
  |-> FormBlock renderer + client validation + ActionBlock sender
  |-> TableBlock renderer + row action handling
  |-> ApprovalBlock renderer + state machine display

Streaming Engine (depends on WebSocket + Content Block Engine)
  |-> Stream start -> placeholder rendering
  |-> Delta accumulation -> progressive text display
  |-> Stream end -> collapse to permanent message
  |-> Stream cancel -> user-initiated stop
  |-> Parallel block streaming

Tool Visibility Engine (depends on Content Block Engine)
  |-> Collapsible indicator rendering
  |-> Lifecycle grouping by call_id
  |-> In-place status updates
  |-> Progress bar (determinate/indeterminate)

Memory Display (depends on WebSocket)
  |-> Context frame rendering
  |-> Archival memory query/display
  |-> Memory management UI (view/delete facts)

Local Persistence (GRDB.swift)
  |-> Offline message cache
  |-> Thread list cache
  |-> Participant cache
  |-> Agent Card cache (with ETag)

Threading (depends on WebSocket + Content Block Engine)
  |-> Thread creation (with mixed participants)
  |-> Thread join/leave
  |-> Sub-thread branching
  |-> Sub-thread navigation
```

## MVP Recommendation

Build in this order. Each tier produces a usable increment.

### Tier 1: Foundation (must have for any demo)
1. **WebSocket connection + ainm.initialize handshake** -- nothing works without this
2. **Ed25519 key generation + Keychain storage** -- every message needs signing
3. **Send and receive text messages** (TextBlock only, Markdown rendering)
4. **Conversation list + thread view** -- basic navigation
5. **Local persistence (GRDB)** -- cache messages for offline reading
6. **Connection status indicator** -- users need to know if they are connected

### Tier 2: Rich Content (makes it feel like a real app)
7. **Code block rendering with syntax highlighting**
8. **Image display** (AsyncImage + fullscreen)
9. **File display + download**
10. **Streaming responses** (token-by-token with cursor animation)
11. **Typing/processing indicators** (presence)
12. **Message signature verification indicators**

### Tier 3: AI-Native Features (the differentiators)
13. **Tool call visibility** (collapsible indicators, lifecycle grouping)
14. **Interactive buttons** (ButtonBlock + ActionBlock flow)
15. **Agent identity display** (type badge, profile with Agent Card data)
16. **@-mention support** (highlighting + composer autocomplete)
17. **Stream cancellation** (stop generating button)

### Tier 4: Full Interactive Suite
18. **Interactive forms** (all field types + validation)
19. **Data tables** (with row actions)
20. **Approval workflows** (state machine UI)
21. **Agent discovery browser** (search, filter, start conversation)
22. **Mixed human-agent thread creation**

### Tier 5: Memory and Polish
23. **Memory context display** (context frames, conversation summary)
24. **Memory management UI** (view/edit/delete archival facts)
25. **Sub-thread branching and navigation**
26. **Push notifications** (APNs integration)
27. **Deep linking** (notification tap -> thread)
28. **Accessibility audit** (VoiceOver, Dynamic Type)

**Defer indefinitely:**
- Extension block plugin architecture: premature for reference client v1
- iPad/Mac: scope creep
- Voice/video: out of scope
- Marketplace: out of scope

## Sources

### Protocol Specifications (PRIMARY -- HIGH confidence)
- AINM spec/content.md -- Content block types and rendering rules
- AINM spec/interactive.md -- ButtonBlock, FormBlock, TableBlock, ApprovalBlock, ActionBlock
- AINM spec/streaming.md -- Stream lifecycle (start/delta/end), parallel blocks, cancellation
- AINM spec/agent-card.md -- Agent Card discovery, capabilities, skills, tools
- AINM spec/tool-visibility.md -- Tool call events, collapsible rendering, lifecycle grouping
- AINM spec/memory.md -- Three-tier memory model, context frames, archival operations
- AINM spec/threading.md -- Flat conversations, sub-threads, presence, mentions
- AINM spec/transport.md -- WebSocket binding, JSON-RPC 2.0, handshake, reconnection
- AINM spec/identity.md -- Address format, identity resolution, key management
- AINM spec/security.md -- Ed25519 signing, JCS canonicalization

### Ecosystem Research (MEDIUM confidence)
- [Smashing Magazine -- Designing for Agentic AI: Practical UX Patterns](https://www.smashingmagazine.com/2026/02/designing-agentic-ai-practical-ux-patterns/) -- Agentic UX patterns for control, consent, accountability
- [Stream Chat SwiftUI SDK -- AI Integrations](https://getstream.io/chat/docs/sdk/ios/ai-integrations/overview/) -- StreamingMessageView, AITypingIndicatorView, token-by-token rendering in SwiftUI
- [Stream -- How to Add A2UI to a Stream Chat App](https://getstream.io/blog/a2ui-chat-integration/) -- A2UI integration patterns for iOS chat
- [A2UI.org](https://a2ui.org/) -- Google's declarative agent UI specification, SwiftUI renderer planned for v1.0
- [Apple Developer -- Curve25519.Signing](https://developer.apple.com/documentation/cryptokit/curve25519/signing) -- Ed25519 via CryptoKit
- [Apple Developer -- Storing CryptoKit Keys in the Keychain](https://developer.apple.com/documentation/cryptokit/storing-cryptokit-keys-in-the-keychain) -- Ed25519 private key storage as kSecClassGenericPassword
- [Stream Chat SwiftUI -- Typing Indicators](https://getstream.io/chat/docs/sdk/ios/swiftui/message-components/typing-indicators/) -- Typing indicator placement patterns (bottomOverlay default)
- [Apple Study on AI Agent Interaction Expectations](https://9to5mac.com/2026/02/12/apple-study-looks-into-how-people-expect-to-interact-with-ai-agents/) -- User expectations for agent UX on iOS
- [AI UX Patterns](https://www.aiuxpatterns.com/) -- Catalog of AI interaction design patterns
- [Groovyweb -- UI/UX Design Trends for AI-First Apps 2026](https://www.groovyweb.co/blog/ui-ux-design-trends-ai-apps-2026) -- Glassmorphism, streaming containers, confidence indicators
