# Phase 11: Core Messaging UI - Research

**Researched:** 2026-03-29
**Domain:** SwiftUI messaging UI, GRDB observation, content block rendering, iOS 18
**Confidence:** HIGH

## Summary

Phase 11 builds the core messaging experience on top of Phase 10's foundation (WebSocket client, GRDB persistence, Ed25519 crypto, onboarding). The work divides into four domains: (1) conversation list driven by GRDB ValueObservation, (2) paginated message thread with hybrid layout (chat bubbles for humans, full-width for agents), (3) content block rendering (text/markdown, code with syntax highlighting, image with fullscreen, file with download), and (4) message composition with Ed25519 signing.

The existing codebase provides `ThreadRecord`, `MessageRecord`, `ParticipantRecord` with GRDB associations, `AINMClient` actor with WebSocket and write-through cache, and `AppState` with `@Observable`. The `ContentBlock` model is text-only and must be expanded to a discriminated union (enum with associated values) supporting all four core block types from the protocol spec. Navigation must change from onboarding-as-root to conversation-list-as-root post-connection.

The main technical risks are: (1) LazyVStack scroll position maintenance when prepending older messages (a known SwiftUI pain point), (2) regex-based syntax highlighting without third-party dependencies (per D-08 decision), and (3) GRDB ValueObservation integration with `@Observable` ViewModels for live-updating UI.

**Primary recommendation:** Use GRDB `ValueObservation.tracking { }.values(in:)` async sequences consumed inside `@Observable` ViewModels via Task lifecycle, with LazyVStack + `scrollPosition(id:)` for the message thread and `defaultScrollAnchor(.bottom)` for initial scroll-to-bottom.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Hybrid message layout -- chat bubbles for human messages (right-aligned accent color), flat/wide layout for agent messages (left-aligned, full width for rich content like code blocks and tool calls).
- **D-02:** Agent badge: icon + type label next to agent name (e.g., cpu.fill + "Agent"). Agent Card data populates the badge. (MSG-09)
- **D-03:** Consecutive messages from same sender are grouped -- show avatar + name only on first message in sequence, tighter spacing for subsequent messages, timestamp on last message only.
- **D-04:** New thread creation via direct compose -- tap '+' to open composer with 'To:' field for @-mention autocomplete of participants (humans and agents). Thread created on first message send via `thread.create` protocol message.
- **D-05:** Standard messaging row: avatar (human or agent), thread title, last message preview (truncated), relative timestamp, unread badge. (MSG-01)
- **D-06:** Sort by most recent activity -- conversations with newest messages float to top. No pinning in v1.
- **D-07:** Empty state: centered SF Symbol icon, "No conversations yet" message, prominent "Start a Conversation" CTA button.
- **D-08:** CodeBlocks: native AttributedString + regex-based syntax highlighting for ~10 common languages. Language label top-left, copy button top-right. Monospace font, dark background. No third-party dependency. (MSG-05)
- **D-09:** ImageBlocks: aspect-fit scaling to full message width, capped at 300pt height. Tap opens fullscreen with pinch-to-zoom. Server-provided dimensions pre-set frame to avoid layout jumps. (MSG-06)
- **D-10:** Multi-block messages render as vertical stack, each block gets full available width. Natural top-to-bottom reading flow. (MSG-04)
- **D-11:** NavigationStack only -- conversation list as root, push to thread detail. No tab bar.
- **D-12:** Simple composer: auto-growing TextField at bottom, send button on right. Plain text input -- Markdown rendered in sent messages but not previewed while typing. Auto-grows to ~4 lines max.
- **D-13:** Connection status as subtle nav bar subtitle under "Conversations" title. Shows Connected / Reconnecting... / Offline. Disappears when connected for 5+ seconds.
- **D-14:** Light/dark mode follows system -- use SwiftUI semantic colors throughout. No in-app toggle for v1.

### Claude's Discretion
- FileBlock download UX design (filename, size display, download action) -- Claude picks best iOS pattern (MSG-07)
- Markdown rendering approach for text blocks (MSG-03) -- AttributedString or swift-markdown, Claude decides
- Pagination implementation for message thread (50 messages, load-more on scroll-up) -- per success criteria
- Offline mode UX details (PLT-02) -- how to indicate cached vs live data
- Unread badge logic -- how unread count is tracked and cleared

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| MSG-01 | User can view conversation list with thread title, last message preview, and timestamps | GRDB ValueObservation on ThreadRecord + last MessageRecord join; ConversationListViewModel |
| MSG-02 | User can view message thread with chronological messages and participant avatars | LazyVStack with pagination, MessageRecord observation, participant type for avatar selection |
| MSG-03 | User can send text messages with Markdown rendering | AttributedString(markdown:) for rendering; AINMClient.sendMessage with Ed25519 signing |
| MSG-04 | App renders multi-block messages (text + code + image in sequence) | ContentBlock enum with associated values; vertical VStack of block renderers |
| MSG-05 | App renders CodeBlocks with syntax highlighting, language label, and copy button | Regex-based AttributedString highlighting; UIPasteboard for copy |
| MSG-06 | App renders ImageBlocks with tap-to-fullscreen and size pre-layout | AsyncImage with frame from width/height fields; fullscreen overlay with MagnifyGesture |
| MSG-07 | App renders FileBlocks with filename, size, and download capability | UIActivityViewController via share sheet; URLSession download |
| MSG-08 | User can create threads with mixed human and agent participants | New thread composer sheet; thread.create JSON-RPC method |
| MSG-09 | App displays agent identity distinctly (type badge, profile shows Agent Card data) | ParticipantRecord.type field; SF Symbol cpu.fill + "Agent" label |
| PLT-02 | App provides offline access to cached messages | Write-through cache pattern already in place; GRDB reads work offline |
| PLT-03 | App supports light and dark mode via SwiftUI semantic colors | Semantic color tokens from UI-SPEC; no hardcoded hex |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

- **Architecture:** MVVM + Repository with `@Observable` (iOS 17+), NavigationStack + NavigationPath
- **Database:** GRDB.swift 7.10.x, write-through cache pattern (server data flows through GRDB before UI)
- **Concurrency:** Swift 6 strict concurrency -- `@MainActor` on state, `Sendable` records, `async/await`
- **Crypto:** Apple CryptoKit `Curve25519.Signing` for Ed25519 message signing
- **No third-party UI libraries:** All SwiftUI native controls, SF Symbols for icons, San Francisco font
- **Target:** iOS 18+ / Xcode 16+ / Swift 6.1+
- **Wire format:** JSON-RPC 2.0 over WebSocket, JSON Schema 2020-12 content validation
- **Content blocks stored as JSON text column** in GRDB -- decode into typed block variants for rendering

## Standard Stack

### Core (Already Installed from Phase 10)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| SwiftUI | iOS 18+ | UI framework | Native declarative UI, LazyVStack for message lists |
| GRDB.swift | 7.10.x | Local database + observation | ValueObservation drives live UI updates from SQLite |
| Swift Concurrency | Swift 6.1+ | Async networking + observation | async/await, Actor isolation, AsyncStream |
| Apple CryptoKit | iOS 13+ | Ed25519 signing | Curve25519.Signing for message signatures |
| KeychainAccess | 4.x | Secure key storage | Ed25519 private key storage |

### Supporting (No New Dependencies)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| Foundation AttributedString | iOS 15+ | Markdown text rendering | TextBlock content with CommonMark subset |
| AsyncImage | iOS 15+ | Image loading from URL | ImageBlock rendering with placeholder |
| UIPasteboard | iOS | Clipboard access | CodeBlock copy button |
| UIActivityViewController | iOS | Share sheet | FileBlock download/share |
| SFSafariViewController | iOS | In-app browser | Tappable links in TextBlock markdown |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| AttributedString(markdown:) | swift-markdown package | More control over AST but adds dependency; AttributedString sufficient for bold/italic/code/links |
| Regex-based syntax highlighting | HighlightSwift package | Better coverage but D-08 explicitly forbids third-party; regex covers ~10 languages adequately |
| GRDBQuery | Manual ValueObservation in VM | GRDBQuery adds another package; manual observation integrates cleanly with existing @Observable pattern |

## Architecture Patterns

### Recommended Project Structure
```
ios/AINMMessenger/AINMMessenger/
  App/
    AINMMessengerApp.swift      # Modified: route to ConversationListView post-auth
    AppState.swift              # Extended: add conversation/thread state
  Models/
    MessageEnvelope.swift       # Modified: expand ContentBlock to enum
    ContentBlock.swift          # NEW: discriminated union for text/code/image/file
    ThreadMetadata.swift        # Existing
    Participant.swift           # Existing
  Persistence/
    AppDatabase.swift           # Extended: add hasUnread column migration
    ThreadRecord.swift          # Extended: add hasUnread field
    MessageRecord.swift         # Existing (contentJson already stores JSON)
    ParticipantRecord.swift     # Existing
    ThreadRepository.swift      # NEW: GRDB queries for conversation list
    MessageRepository.swift     # NEW: GRDB queries for message thread
  Networking/
    AINMClient.swift            # Extended: thread.create, thread list sync
  UI/
    ConversationList/
      ConversationListView.swift      # NEW: Screen 1
      ConversationListViewModel.swift # NEW: GRDB observation
      ConversationRowView.swift       # NEW: single row component
    MessageThread/
      MessageThreadView.swift         # NEW: Screen 2
      MessageThreadViewModel.swift    # NEW: paginated observation
      MessageBubbleView.swift         # NEW: human bubble
      AgentMessageView.swift          # NEW: agent flat layout
      MessageGroupView.swift          # NEW: grouping logic
    ContentBlocks/
      TextBlockView.swift             # NEW: markdown rendering
      CodeBlockView.swift             # NEW: syntax highlighting
      ImageBlockView.swift            # NEW: async image + fullscreen
      FileBlockView.swift             # NEW: file download row
      ContentBlockRenderer.swift      # NEW: switch on block type
    Composer/
      ComposerView.swift              # NEW: Screen 2a
      NewThreadComposerView.swift     # NEW: Screen 3
    Shared/
      AvatarView.swift                # NEW: reusable avatar circle
      ConnectionStatusView.swift      # NEW: D-13 status subtitle
      FullscreenImageView.swift       # NEW: pinch-to-zoom overlay
    OnboardingView.swift              # Existing
    ConnectionSuccessView.swift       # Remove or repurpose
  Helpers/
    SyntaxHighlighter.swift           # NEW: regex-based highlighting
    RelativeTimestamp.swift            # NEW: "2m ago", "Yesterday"
    FileSizeFormatter.swift           # NEW: KB/MB/GB formatting
```

### Pattern 1: GRDB ValueObservation in @Observable ViewModel

**What:** Each screen has an `@Observable` ViewModel that starts a GRDB ValueObservation as an async sequence and updates published properties on change.

**When to use:** Every screen that displays database-driven content (conversation list, message thread).

**Example:**
```swift
// Source: GRDB docs + established Phase 10 patterns
@MainActor
@Observable
final class ConversationListViewModel {
    var threads: [ThreadWithPreview] = []
    private var observationTask: Task<Void, Never>?
    private let database: AppDatabase

    init(database: AppDatabase) {
        self.database = database
    }

    func startObserving() {
        observationTask = Task {
            let observation = ValueObservation.tracking { db in
                try ThreadRecord
                    .including(optional: ThreadRecord.messages
                        .order(Column("timestamp").desc)
                        .limit(1))
                    .order(sql: """
                        (SELECT MAX(m.timestamp) FROM message m
                         WHERE m.threadId = thread.id) DESC NULLS LAST
                        """)
                    .asRequest(of: ThreadWithPreview.self)
                    .fetchAll(db)
            }
            do {
                for try await threads in observation.values(in: database.dbPool) {
                    self.threads = threads
                }
            } catch {
                // Handle observation error
            }
        }
    }

    func stopObserving() {
        observationTask?.cancel()
        observationTask = nil
    }
}
```

### Pattern 2: ContentBlock Discriminated Union

**What:** Replace the flat `ContentBlock` struct with a Swift enum that has associated values for each block type's specific fields.

**When to use:** Decoding contentJson from MessageRecord, rendering content in message views.

**Example:**
```swift
enum ContentBlock: Codable, Sendable {
    case text(TextContent)
    case code(CodeContent)
    case image(ImageContent)
    case file(FileContent)
    case unknown(fallback: String)

    struct TextContent: Codable, Sendable {
        let version: String
        let source: String
        let text: String
        let mentions: [Mention]?
        let fallback: String
    }

    struct CodeContent: Codable, Sendable {
        let version: String
        let source: String
        let language: String
        let code: String
        let filename: String?
        let fallback: String
    }

    struct ImageContent: Codable, Sendable {
        let version: String
        let source: String
        let url: String
        let mimeType: String
        let alt: String
        let width: Int?
        let height: Int?
        let sizeBytes: Int?
        let fallback: String
    }

    struct FileContent: Codable, Sendable {
        let version: String
        let source: String
        let url: String
        let mimeType: String
        let filename: String
        let sizeBytes: Int
        let digest: String
        let fallback: String
    }
}
```

### Pattern 3: Message Grouping Algorithm

**What:** Group consecutive messages by sender for D-03 (avatar + name on first only, tight spacing within group).

**When to use:** MessageThreadViewModel processes flat message array into grouped structure for rendering.

**Example:**
```swift
struct MessageGroup: Identifiable {
    let id: String  // first message ID
    let senderAddress: String
    let senderType: String  // "human" or "agent"
    let messages: [MessageRecord]
    var isFromCurrentUser: Bool
}

func groupMessages(_ messages: [MessageRecord]) -> [MessageGroup] {
    var groups: [MessageGroup] = []
    var current: [MessageRecord] = []
    var currentSender: String?

    for message in messages {
        if message.fromAddress == currentSender {
            current.append(message)
        } else {
            if !current.isEmpty, let sender = currentSender {
                groups.append(MessageGroup(
                    id: current[0].id,
                    senderAddress: sender,
                    senderType: /* lookup from participants */,
                    messages: current,
                    isFromCurrentUser: /* compare with connectedIdentity */
                ))
            }
            current = [message]
            currentSender = message.fromAddress
        }
    }
    // Don't forget the last group
    if !current.isEmpty, let sender = currentSender {
        groups.append(/* ... */)
    }
    return groups
}
```

### Pattern 4: Paginated Message Loading

**What:** Load most recent 50 messages initially, load older 50 on scroll-to-top.

**When to use:** MessageThreadView scroll position management.

**Example:**
```swift
@MainActor
@Observable
final class MessageThreadViewModel {
    var messageGroups: [MessageGroup] = []
    var isLoadingMore = false
    var hasMoreMessages = true
    private var oldestTimestamp: String?
    private let threadId: String
    private let database: AppDatabase
    private let pageSize = 50

    func loadInitialMessages() async {
        let messages = try await database.dbPool.read { db in
            try MessageRecord
                .filter(Column("threadId") == threadId)
                .order(Column("timestamp").desc)
                .limit(pageSize)
                .fetchAll(db)
        }
        self.messageGroups = groupMessages(messages.reversed())
        self.oldestTimestamp = messages.last?.timestamp
        self.hasMoreMessages = messages.count == pageSize
    }

    func loadOlderMessages() async {
        guard !isLoadingMore, hasMoreMessages, let oldest = oldestTimestamp else { return }
        isLoadingMore = true
        let older = try await database.dbPool.read { db in
            try MessageRecord
                .filter(Column("threadId") == threadId)
                .filter(Column("timestamp") < oldest)
                .order(Column("timestamp").desc)
                .limit(pageSize)
                .fetchAll(db)
        }
        // Prepend and re-group
        hasMoreMessages = older.count == pageSize
        oldestTimestamp = older.last?.timestamp
        isLoadingMore = false
    }
}
```

### Anti-Patterns to Avoid
- **Fetching all messages at once:** LazyVStack does NOT recycle views (STATE.md blocker). Pagination is required from day 1 to avoid memory issues with large threads.
- **Observing entire message table:** ValueObservation should be scoped to the specific threadId, not the entire message table. Broad observation triggers unnecessary UI updates.
- **Using List instead of ScrollView + LazyVStack:** List has built-in features but poor control over scroll position and message bubble customization. ScrollView + LazyVStack gives full control.
- **Storing unread state only in memory:** Unread badge must survive app restart. Use a GRDB column (hasUnread on ThreadRecord per UI-SPEC).
- **Blocking main thread with JSON decoding:** ContentBlock JSON decoding from the contentJson column should happen in the ViewModel or a background context, not in the View body.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Markdown rendering | Custom CommonMark parser | `AttributedString(markdown:)` (Foundation) | Handles bold, italic, code spans, links, lists. Native iOS 15+. Covers MSG-03 needs |
| Image loading + caching | Custom download + NSCache | `AsyncImage` (SwiftUI native) | Handles progressive loading, placeholder, caching. Built into SwiftUI |
| Relative timestamps | Manual date math | `RelativeDateTimeFormatter` (Foundation) | "2 minutes ago", "Yesterday" -- locale-aware, handles edge cases |
| File size formatting | Manual KB/MB conversion | `ByteCountFormatter` (Foundation) | Locale-aware, handles all unit thresholds automatically |
| Keyboard avoidance | Manual frame adjustment | SwiftUI automatic keyboard avoidance | iOS 15+ SwiftUI handles this natively for views inside ScrollView |
| Pull-to-refresh | Custom gesture recognizer | `.refreshable` modifier (SwiftUI) | Native pull-to-refresh with async/await integration |
| Share sheet | Custom file handling | `UIActivityViewController` via UIViewControllerRepresentable | Standard iOS share sheet for FileBlock downloads |

**Key insight:** SwiftUI iOS 18 and Foundation provide native solutions for nearly every UI primitive this phase needs. The only custom implementation required is the regex-based syntax highlighter (D-08) and the message grouping algorithm.

## Common Pitfalls

### Pitfall 1: LazyVStack Scroll Position on Prepend
**What goes wrong:** When loading older messages and prepending them to the top of the list, the scroll position jumps to the top instead of staying on the message the user was reading.
**Why it happens:** SwiftUI's LazyVStack re-renders from the top when items are inserted at index 0. Unlike UIKit's UITableView, there is no built-in `contentOffset` preservation.
**How to avoid:** Use `scrollPosition(id:)` (iOS 17+) to bind scroll position to a message ID. Before prepending, capture the current top-visible message ID. After prepending, set the scroll position binding back to that ID. Alternatively, use `defaultScrollAnchor(.bottom)` for initial load and manual `ScrollViewReader.scrollTo()` for position restoration.
**Warning signs:** Messages "jump" when user scrolls up to load more.

### Pitfall 2: ValueObservation + Pagination Conflict
**What goes wrong:** ValueObservation detects new messages and re-fetches, but if the observation query uses LIMIT, it may not include all loaded pages, causing visible messages to disappear.
**Why it happens:** The observation tracks a query with `LIMIT 50`, but the user has scrolled up and loaded 150 messages. A new message triggers the observation, which returns only the latest 50.
**How to avoid:** Use ValueObservation only for detecting NEW messages (e.g., observe messages with timestamp > last known). For pagination, use one-shot `dbPool.read` queries. Merge new messages from observation with the existing paginated array.
**Warning signs:** Older messages vanish when a new message arrives.

### Pitfall 3: ContentBlock JSON Decoding with Unknown Types
**What goes wrong:** App crashes when receiving a content block with an unrecognized type (extension blocks like `com.example.weather-card`).
**Why it happens:** A strict enum decoder fails on unknown type discriminators.
**How to avoid:** Implement custom `init(from decoder:)` on ContentBlock enum. Use a `default` / `unknown` case that captures the fallback text. Per content spec Section 5: "Clients that do not recognize an extension block type MUST render the fallback field."
**Warning signs:** Crash logs showing `DecodingError.dataCorrupted` on content block type.

### Pitfall 4: Swift 6 Strict Concurrency in ViewModels
**What goes wrong:** Compiler errors about sending non-Sendable types across actor boundaries, or data races with observation callbacks.
**Why it happens:** `@Observable` classes on `@MainActor` interact with GRDB's actor-isolated methods. ValueObservation callbacks may cross isolation boundaries.
**How to avoid:** Mark ViewModels as `@MainActor @Observable`. Use `for try await` in a Task launched from the MainActor context. GRDB 7's ValueObservation `.values(in:)` returns a MainActor-friendly async sequence. Ensure all record types (`ThreadRecord`, `MessageRecord`) remain `Sendable`.
**Warning signs:** Compiler warnings about `Sendable` conformance, runtime `@MainActor` assertion failures.

### Pitfall 5: AttributedString Markdown Limitations
**What goes wrong:** Tables, headers (H1-H6), and images in markdown do not render in `AttributedString(markdown:)`.
**Why it happens:** Apple's Foundation AttributedString supports a limited CommonMark subset: bold, italic, strikethrough, code spans, links, and (since iOS 16.4) ordered/unordered lists. Headers, tables, images, and block quotes are NOT supported.
**How to avoid:** This is acceptable for Phase 11 -- TextBlock content is primarily paragraph text with inline formatting. Document the limitation. If a message contains a markdown table, it will render as plain text, which is the correct degraded behavior.
**Warning signs:** Headers rendering as plain text without visual distinction.

### Pitfall 6: AsyncImage Memory with Many Images
**What goes wrong:** ScrollView with many ImageBlocks causes memory pressure as AsyncImage holds loaded images in memory.
**Why it happens:** LazyVStack does not recycle views or release image data when scrolled off-screen (unlike UIKit's cell reuse).
**How to avoid:** The 50-message pagination limit naturally caps the number of concurrent images. For Phase 11, this is sufficient. If memory becomes an issue, consider a custom image cache with size limits, or downsampling large images for display.
**Warning signs:** Memory warnings in Xcode when scrolling through image-heavy threads.

## Code Examples

### GRDB ValueObservation for Conversation List
```swift
// Source: GRDB 7.10 docs, adapted for project patterns
// Observe threads with their latest message for conversation list
let observation = ValueObservation.tracking { db in
    try ThreadRecord
        .annotated(with:
            ThreadRecord.messages.max(Column("timestamp")).forKey("lastMessageTimestamp"),
            ThreadRecord.messages
                .filter(Column("type") == "message")
                .select(Column("contentJson"))
                .order(Column("timestamp").desc)
                .limit(1)
                .forKey("lastMessageContent")
        )
        .order(literal: """
            (SELECT MAX(m.timestamp) FROM message m WHERE m.threadId = thread.id) DESC NULLS LAST
        """)
        .asRequest(of: ConversationRow.self)
        .fetchAll(db)
}
```

### ContentBlock Custom Decoder
```swift
// Source: Protocol spec content.md + content.schema.json
extension ContentBlock {
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)

        switch type {
        case "text":
            self = .text(try TextContent(from: decoder))
        case "code":
            self = .code(try CodeContent(from: decoder))
        case "image":
            self = .image(try ImageContent(from: decoder))
        case "file":
            self = .file(try FileContent(from: decoder))
        default:
            // Extension or unknown block -- render fallback
            let fallback = try container.decode(String.self, forKey: .fallback)
            self = .unknown(fallback: fallback)
        }
    }

    private enum CodingKeys: String, CodingKey {
        case type, fallback
    }
}
```

### Regex-Based Syntax Highlighter
```swift
// Source: D-08 decision + Swift regex patterns
struct SyntaxHighlighter {
    struct Rule {
        let pattern: String
        let color: Color
    }

    // Shared rules across languages
    static let stringRule = Rule(
        pattern: #"\"[^\"\\]*(?:\\.[^\"\\]*)*\""#,
        color: Color(red: 0.84, green: 0.36, blue: 0.36) // reddish
    )
    static let commentRule = Rule(
        pattern: #"//[^\n]*|/\*[\s\S]*?\*/"#,
        color: .secondary
    )
    static let numberRule = Rule(
        pattern: #"\b\d+(\.\d+)?\b"#,
        color: Color(red: 0.69, green: 0.56, blue: 0.99) // purple
    )

    // Per-language keyword lists
    static let swiftKeywords = [
        "func", "var", "let", "if", "else", "return", "struct",
        "class", "enum", "import", "guard", "switch", "case",
        "for", "while", "try", "catch", "throw", "async", "await"
    ]

    static func highlight(_ code: String, language: String) -> AttributedString {
        var attributed = AttributedString(code)
        attributed.font = .system(.body, design: .monospaced)
        attributed.foregroundColor = .primary

        let rules = rulesForLanguage(language)
        let nsString = code as NSString

        for rule in rules {
            guard let regex = try? NSRegularExpression(pattern: rule.pattern) else { continue }
            let matches = regex.matches(in: code, range: NSRange(location: 0, length: nsString.length))
            for match in matches {
                if let range = Range(match.range, in: code),
                   let attrRange = Range(range, in: attributed) {
                    attributed[attrRange].foregroundColor = rule.color
                }
            }
        }
        return attributed
    }
}
```

### Scroll Position Management for Message Thread
```swift
// Source: SwiftUI iOS 17+ scrollPosition API
struct MessageThreadView: View {
    @Bindable var viewModel: MessageThreadViewModel
    @State private var scrollPosition: ScrollPosition = .init(idType: String.self)

    var body: some View {
        ScrollView {
            LazyVStack(spacing: 0) {
                if viewModel.isLoadingMore {
                    ProgressView("Loading older messages...")
                        .padding()
                }

                ForEach(viewModel.messageGroups) { group in
                    MessageGroupView(group: group)
                }
            }
        }
        .scrollPosition($scrollPosition)
        .defaultScrollAnchor(.bottom)
        .onScrollGeometryChange(for: Bool.self) { geometry in
            geometry.contentOffset.y <= 20  // near top
        } action: { _, isNearTop in
            if isNearTop {
                Task { await viewModel.loadOlderMessages() }
            }
        }
    }
}
```

### Thread Creation via Protocol
```swift
// Source: threading.md Section 3.4 + transport.md
func createThread(
    title: String?,
    participants: [String],
    firstMessageText: String
) async throws {
    let threadId = UUID().uuidString  // UUIDv7 in production
    let timestamp = ISO8601DateFormatter.ainm.string(from: Date())

    // 1. Send thread.create
    let createEnvelope = MessageEnvelope(
        id: UUID().uuidString,
        from: connectedIdentity,
        threadId: threadId,
        timestamp: timestamp,
        type: .threadCreate,
        content: [],
        signature: nil,
        parentId: nil
    )
    try await client.sendMessage(envelope: createEnvelope)

    // 2. Send first message
    let messageEnvelope = MessageEnvelope(
        id: UUID().uuidString,
        from: connectedIdentity,
        threadId: threadId,
        timestamp: ISO8601DateFormatter.ainm.string(from: Date()),
        type: .message,
        content: [ContentBlock.text(ContentBlock.TextContent(
            version: "1.0",
            source: "user",
            text: firstMessageText,
            mentions: nil,
            fallback: firstMessageText
        ))],
        signature: nil,
        parentId: nil
    )
    try await client.sendMessage(envelope: messageEnvelope)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| ObservableObject + @Published | @Observable (Observation framework) | iOS 17 / WWDC 2023 | Less boilerplate, finer-grained updates, better performance |
| Combine for GRDB observation | async/await ValueObservation.values(in:) | GRDB 6.0+ / 2023 | Simpler code, native Swift concurrency integration |
| UIKit UITableView for chat | SwiftUI LazyVStack + ScrollView | iOS 14+ (mature iOS 17+) | Declarative but lacks cell recycling; requires pagination discipline |
| NavigationView | NavigationStack + NavigationPath | iOS 16+ / WWDC 2022 | Type-safe programmatic navigation, better deep linking |
| Custom keyboard avoidance | SwiftUI automatic | iOS 15+ | No manual frame calculations needed |

**Deprecated/outdated:**
- `ObservableObject` / `@StateObject` / `@Published`: Still works but `@Observable` is preferred for new code. Phase 10 already uses `@Observable`.
- `List` for message views: Too opinionated for custom message layouts. Use `ScrollView + LazyVStack`.
- Combine publishers for GRDB: `ValueObservation.publisher(in:)` still works but async sequences are idiomatic in Swift 6.

## Open Questions

1. **GRDB Annotation Queries for Conversation List**
   - What we know: GRDB supports `annotated(with:)` for aggregate queries and `including()` for associations.
   - What's unclear: The exact query syntax for fetching threads sorted by latest message timestamp with a preview of the last message content. May need a raw SQL subquery or a custom `FetchableRecord`.
   - Recommendation: Start with a simpler approach (fetch threads, then fetch last message per thread separately) and optimize to a single query if performance is acceptable.

2. **ScrollView Position Stability on iOS 18**
   - What we know: `scrollPosition(id:)` and `defaultScrollAnchor` exist. iOS 18 improved scroll behavior.
   - What's unclear: Whether prepending items to a LazyVStack with `scrollPosition` bound to a message ID truly maintains position without visual jumps on iOS 18.
   - Recommendation: Implement with `scrollPosition(id:)` + capture/restore pattern. If jumps occur, fall back to `ScrollViewReader.scrollTo()` after a brief delay. Test empirically on device.

3. **AINMClient Thread Sync**
   - What we know: `AINMClient` handles `ainm.message.deliver` notifications and writes to GRDB. Thread list sync requires an initial fetch of existing threads.
   - What's unclear: Whether the server sends a thread list on connect, or whether the client must request it. The current `handleIncomingMessage` only handles message delivery.
   - Recommendation: Add `ainm.thread.list` request method to AINMClient. If the server does not support it yet, seed conversation list purely from locally cached GRDB data (offline-first approach per PLT-02).

## Environment Availability

Step 2.6: SKIPPED (no external dependencies identified). Phase 11 is purely iOS code/UI changes using already-installed dependencies from Phase 10.

## Sources

### Primary (HIGH confidence)
- Protocol spec `spec/content.md` -- Content block types, fields, rendering rules
- Protocol spec `spec/threading.md` -- Thread model, creation, participant management
- Schema `schemas/content.schema.json` -- JSON Schema for all block types
- Existing codebase (Phase 10) -- AppState, AppDatabase, MessageRecord, AINMClient patterns
- UI-SPEC `11-UI-SPEC.md` -- Visual contract for all screens and components

### Secondary (MEDIUM confidence)
- [GRDB ValueObservation docs](https://swiftpackageindex.com/groue/grdb.swift/v7.10.0/documentation/grdb/valueobservation) -- async values pattern
- [Apple ScrollPosition docs](https://developer.apple.com/documentation/swiftui/scrollposition) -- scroll position management
- [Apple AttributedString Markdown](https://developer.apple.com/documentation/foundation/instantiating-attributed-strings-with-markdown-syntax) -- CommonMark rendering
- [Apple Developer Forums: scroll position on prepend](https://developer.apple.com/forums/thread/731271) -- known limitations
- [GRDB GitHub README](https://github.com/groue/GRDB.swift) -- ValueObservation patterns

### Tertiary (LOW confidence)
- [Medium: regex syntax highlighting in SwiftUI](https://medium.com/@orhanerday/building-a-swiftui-code-block-view-with-syntax-highlighting-d3d737a90a65) -- approach validation for D-08
- [Perisync: SwiftUI pagination](https://perisync.com/blogs/pagination-in-swiftui-scrollview-lazyvstack-mvvm) -- general pagination patterns

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already installed and proven in Phase 10; no new dependencies
- Architecture: HIGH -- MVVM + Repository with @Observable established in Phase 10; extension is straightforward
- Content block rendering: HIGH -- protocol spec is complete and schemas are defined; implementation is mechanical
- Scroll position management: MEDIUM -- known SwiftUI limitation; iOS 18 improvements may help but needs empirical testing
- Syntax highlighting: MEDIUM -- regex approach is sound but coverage quality depends on regex patterns per language
- Pitfalls: HIGH -- documented from real SwiftUI messaging app development patterns

**Research date:** 2026-03-29
**Valid until:** 2026-04-28 (30 days -- stable iOS/SwiftUI stack, no breaking changes expected)
