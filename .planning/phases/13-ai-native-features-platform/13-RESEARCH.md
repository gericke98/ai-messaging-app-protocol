# Phase 13: AI-Native Features & Platform - Research

**Researched:** 2026-03-29
**Domain:** iOS SwiftUI client -- tool visibility, @-mentions, agent discovery, memory context, sub-threading, deep linking
**Confidence:** HIGH

## Summary

Phase 13 implements the features that differentiate AINM from conventional messaging protocols. The existing iOS client (Phases 10-12) provides a solid foundation: the app already has a working MVVM+Repository architecture with @Observable state, GRDB persistence with migrations, a WebSocket-based AINMClient actor, streaming support with callbacks, and ContentBlock rendering infrastructure. Phase 13 adds five feature domains on top of this: (1) tool call visibility with collapsible lifecycle indicators, (2) @-mention autocomplete in the composer and highlighted rendering, (3) agent discovery via Agent Cards, (4) memory context transparency (context frames + archival facts), and (5) sub-threading with deep linking.

All five feature domains are fully specified in the AINM protocol specs (tool-visibility.md, threading.md Section 9, agent-card.md, memory.md, threading.md Section 4). The iOS client already has the data structures for most of these -- MessageEnvelope supports `parentId`, ThreadMetadata supports `parentThreadId`/`parentMessageId`, ContentBlock.TextContent already includes `mentions`, and ThreadRecord already has `parentThreadId`/`parentMessageId` columns. The primary work is: (a) new model types for tool events, Agent Cards, memory facts, and context frames; (b) new GRDB tables and migrations; (c) new JSON-RPC method handling in AINMClient; (d) new SwiftUI views for each feature; and (e) wiring deep link URL handling via NavigationStack.

**Primary recommendation:** Organize into 4-5 plans: tool call visibility (models + views), @-mentions (composer autocomplete + rendering), agent discovery (networking + views), memory context (models + networking + views), and sub-threading + deep links (threading UI + URL handling). Each is a self-contained vertical slice.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| AI-01 | Tool call visibility -- collapsible indicators with lifecycle grouping | Fully specified in tool-visibility.md. Three message types (tool.call.started/progress/completed), grouped by call_id. Rendering guidance in Section 5. Need new MessageType cases, tool event parsing in AINMClient, ToolCallIndicatorView with expand/collapse, and lifecycle grouping logic. |
| AI-02 | @-mentions with highlighting and composer autocomplete | Spec in content.md Section 4.1 and threading.md Section 9. Mention struct already exists in ContentBlock.TextContent. Need: (a) MentionHighlightRenderer for TextBlockView to color mention spans, (b) composer autocomplete overlay that filters participants by typed prefix, (c) mention insertion into text with offset/length tracking. |
| AI-03 | Agent discovery view with Agent Cards | Fully specified in agent-card.md. Need: AgentCard model (Codable), HTTP fetch via identity resolution chain, AgentDiscoveryView with search/filter, AgentCardDetailView showing skills/tools/capabilities, and "Start Conversation" action. |
| AI-04 | Memory context -- active context frames, conversation history | Spec in memory.md. Need: ContextFrame model, MemoryFact model, handling ainm.context.frame notifications in AINMClient, MemoryContextView showing active context per agent, and ainm.memory.query support for conversation tier. |
| AI-05 | Archival memory -- view, search, delete facts | Spec in memory.md Sections 5.1-5.3. Need: ArchivalMemoryView with search, fact deletion via ainm.memory.evict, GRDB table for local fact caching, and SwiftUI list with swipe-to-delete. |
| PLT-04 | Sub-threading -- branch from message, navigate parent/child | Spec in threading.md Section 4. ThreadMetadata and ThreadRecord already support parent fields. Need: "Branch" action on messages, sub-thread creation via ainm.thread.create with parent fields, sub-thread indicator on parent messages, and navigation between parent/sub-thread. |
| PLT-05 | Deep linking to threads and messages | Need URL scheme (ainm://thread/{id} and ainm://thread/{id}/message/{id}), onOpenURL handler in AINMMessengerApp, NavigationPath-based programmatic navigation, and scroll-to-message in MessageThreadView. |
</phase_requirements>

## Standard Stack

### Core (Already in Project)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| SwiftUI | iOS 18+ | UI framework | All views use SwiftUI, @Observable pattern established |
| Swift Concurrency | 6.1+ | Async/networking | Actor-isolated AINMClient, async/await throughout |
| GRDB.swift | 7.10.x | Local database | Established persistence layer with migrations |
| Apple CryptoKit | iOS 13+ | Cryptography | Ed25519 signing already implemented |
| URLSession | iOS 13+ | HTTP networking | Needed for Agent Card fetching (new in this phase) |

### No New Dependencies Required

Phase 13 does not require any new Swift packages. All features build on existing infrastructure:
- Tool visibility: extends existing MessageEnvelope/ContentBlock models
- @-mentions: extends existing TextBlockView with AttributedString
- Agent discovery: URLSession for HTTP GET (already available)
- Memory: extends existing AINMClient JSON-RPC handling
- Deep linking: native SwiftUI onOpenURL + NavigationStack

## Architecture Patterns

### Recommended Project Structure (New Files)
```
ios/AINMMessenger/AINMMessenger/
  Models/
    ToolEvent.swift              # ToolCallStarted/Progress/Completed structs
    AgentCard.swift              # A2A-compatible Agent Card + AINM extensions
    MemoryModels.swift           # ContextFrame, MemoryFact, MemoryQueryResult
  Networking/
    AgentCardService.swift       # HTTP fetch + ETag caching for Agent Cards
    MemoryClient.swift           # ainm.memory.store/query/evict methods (or extend AINMClient)
  Persistence/
    AgentCardRecord.swift        # GRDB record for cached Agent Cards
    MemoryFactRecord.swift       # GRDB record for locally cached archival facts
  UI/
    ToolVisibility/
      ToolCallIndicatorView.swift   # Collapsible tool event row
      ToolCallDetailView.swift      # Expanded view with input/result/duration
    Mentions/
      MentionAutocompleteView.swift # Overlay popup for @-mention picker
      MentionTextRenderer.swift     # AttributedString builder for mention highlighting
    AgentDiscovery/
      AgentDiscoveryView.swift      # Browsable list of discovered agents
      AgentCardDetailView.swift     # Full Agent Card profile view
    Memory/
      MemoryContextView.swift       # Active context frames per agent
      ArchivalMemoryView.swift      # Searchable archival facts with delete
    SubThreading/
      SubThreadIndicatorView.swift  # Badge on parent messages showing sub-thread count
      SubThreadBannerView.swift     # Navigation bar banner in sub-threads
  App/
    DeepLinkHandler.swift           # URL scheme parsing + NavigationPath routing
```

### Pattern 1: Tool Event Lifecycle Grouping (Client-Side)
**What:** Group tool events by `call_id` into a single visual unit, updating in place as events arrive.
**When to use:** Always -- spec Section 5.6 mandates lifecycle grouping.
**Example:**
```swift
// ToolCallState holds the current lifecycle state for a single call_id
struct ToolCallState: Identifiable {
    let id: String  // call_id
    var name: String
    var status: ToolCallStatus  // .running, .completed, .error
    var input: [String: AnyCodableValue]?
    var progress: ToolCallProgress?
    var resultSummary: String?
    var durationMs: Int?
    var errorMessage: String?
    var isExpanded: Bool = false

    enum ToolCallStatus {
        case running
        case completed
        case error
    }
}
```

### Pattern 2: @-Mention Autocomplete in Composer
**What:** When user types `@` in the composer, show a filtered popup of thread participants.
**When to use:** AI-02 requirement.
**Approach:**
- Monitor text changes in the composer TextField
- Detect `@` character followed by partial name
- Show overlay with matching participants from current thread
- On selection, insert the display text and track mention metadata (address, offset, length)
- Store mentions array alongside composed text for inclusion in the TextBlock

### Pattern 3: Deep Link URL Handling
**What:** Handle URLs like `ainm://thread/{thread_id}` and `ainm://thread/{thread_id}/message/{message_id}`.
**When to use:** PLT-05 requirement.
**Approach:**
```swift
// In AINMMessengerApp.swift
.onOpenURL { url in
    if let route = DeepLinkHandler.parse(url) {
        switch route {
        case .thread(let threadId):
            appState.selectedThreadId = threadId
        case .message(let threadId, let messageId):
            appState.selectedThreadId = threadId
            appState.scrollToMessageId = messageId
        }
    }
}
```

### Pattern 4: Memory Operations as Private JSON-RPC
**What:** Memory operations (store/query/evict) are private client-server exchanges, NOT thread messages.
**When to use:** AI-04, AI-05 requirements.
**Key insight from spec:** Memory operations are NOT visible to other participants. They use the existing WebSocket session but are NOT delivered to threads. The AINMClient should have dedicated methods that send JSON-RPC requests and handle responses internally, separate from the thread message flow.

### Anti-Patterns to Avoid
- **Treating tool events as separate messages in the UI:** Tool events with the same `call_id` MUST be grouped into a single visual unit. Do NOT render each as a separate message bubble.
- **Sending memory queries as thread messages:** Memory operations are private. Do NOT route them through the thread message delivery path.
- **Deeply nested sub-threads:** The spec limits to ONE level. The UI must enforce this by hiding the "Branch" action on messages already in sub-threads.
- **Blocking the main thread on Agent Card fetches:** Agent Cards are fetched over HTTP. Always fetch async and cache with ETag.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Mention text highlighting | Custom string parser | `AttributedString` with custom attributes | SwiftUI natively renders AttributedString; manual offset calculation is error-prone |
| JSON pretty-printing (tool input display) | Custom JSON formatter | `JSONSerialization.data(withJSONObject:, options: .prettyPrinted)` | Built-in, handles all edge cases |
| ETag-based HTTP caching | Custom cache layer | URLSession with `URLCache` + manual `If-None-Match` header | Standard HTTP caching pattern |
| Deep link URL parsing | Custom string splitting | `URLComponents` from Foundation | Handles edge cases (encoding, query params) |
| Duration formatting | Custom ms-to-string | Computed property with simple thresholds per spec Section 5.4 | Simple enough to inline, but follow the spec's exact rules |

## Common Pitfalls

### Pitfall 1: Tool Event Ordering and Buffering
**What goes wrong:** ToolCallProgress or ToolCallCompleted arrives before ToolCallStarted due to message reordering.
**Why it happens:** WebSocket messages can arrive out of order during high-throughput tool use.
**How to avoid:** Buffer tool events for up to 10 seconds if the ToolCallStarted hasn't arrived yet (spec Section 6.4). Render from `content[]` fallback if started never arrives.
**Warning signs:** Tool indicators appearing as "unknown tool" or rendering incorrectly.

### Pitfall 2: Mention Offset Drift During Editing
**What goes wrong:** User edits text before a mention, shifting all subsequent mention offsets.
**Why it happens:** Mention offsets are character-based and break when text is inserted/deleted before them.
**How to avoid:** Use a token-based representation internally (e.g., placeholder characters or attributed ranges) and recalculate offsets only at send time. Alternatively, store mentions as ranges in an AttributedString and extract offsets when building the TextBlock.
**Warning signs:** Mentions highlighting the wrong text span after edits.

### Pitfall 3: Agent Card Fetch Cascading on Discovery View
**What goes wrong:** Opening the agent discovery view triggers dozens of HTTP requests simultaneously.
**Why it happens:** Each agent's card requires identity resolution THEN Agent Card fetch (two HTTP requests per agent).
**How to avoid:** Implement request throttling, lazy loading (fetch on scroll), and aggressive ETag caching. Cache Agent Cards in GRDB with their ETag for offline access.
**Warning signs:** Network saturation, slow discovery view load.

### Pitfall 4: Memory Context Frame Overwriting Active State
**What goes wrong:** A context frame notification arrives and replaces the user's current view of the conversation.
**Why it happens:** Context frames are server-push notifications that can arrive at any time (spec Section 4.3).
**How to avoid:** Context frames update a separate data layer, NOT the current message list directly. Only surface context frame data in a dedicated "Context" panel or on-demand view. Never auto-replace the user's scroll position or visible messages.
**Warning signs:** Messages jumping or disappearing when context frames arrive.

### Pitfall 5: Sub-Thread Navigation Breaking Back Button
**What goes wrong:** User navigates from parent thread to sub-thread and back button doesn't return correctly.
**Why it happens:** NavigationStack path gets confused with thread-within-thread navigation.
**How to avoid:** Use a single NavigationStack with a clear path model that distinguishes root threads from sub-threads. Include parent thread context in the sub-thread navigation destination. Use NavigationPath carefully.
**Warning signs:** Orphaned navigation states, back button going to wrong screen.

### Pitfall 6: Orphaned Tool Call Indicators (No Completed Event)
**What goes wrong:** A tool call shows "running..." forever because the agent disconnected before sending ToolCallCompleted.
**Why it happens:** Spec Section 6.3 acknowledges this can happen.
**How to avoid:** Implement a client-side timeout (spec recommends 5 minutes). After timeout, display "timed out" or "unknown status" instead of "running".
**Warning signs:** Permanent spinning indicators in old conversations.

## Code Examples

### Tool Event Message Type Extension
```swift
// Extend MessageEnvelope.MessageType with tool event types
extension MessageEnvelope.MessageType {
    // Add to existing enum:
    // case toolCallStarted = "tool.call.started"
    // case toolCallProgress = "tool.call.progress"
    // case toolCallCompleted = "tool.call.completed"
}
```

### Tool Event Model
```swift
// Source: tool-visibility.md Section 3
struct ToolCallEvent: Codable, Sendable {
    let callId: String
    let name: String?          // Present on started and completed
    let description: String?    // Present on started
    let input: [String: AnyCodableValue]?  // Present on started
    let progress: ToolProgress? // Present on progress
    let resultSummary: String?  // Present on completed
    let durationMs: Int?        // Present on completed
    let status: String?         // "success" or "error", present on completed
    let errorMessage: String?   // Present on completed with error

    struct ToolProgress: Codable, Sendable {
        let current: Int?
        let total: Int?
        let detail: String?
    }

    enum CodingKeys: String, CodingKey {
        case callId = "call_id"
        case name, description, input, progress
        case resultSummary = "result_summary"
        case durationMs = "duration_ms"
        case status
        case errorMessage = "error_message"
    }
}
```

### Agent Card Model
```swift
// Source: agent-card.md Section 5-6
struct AgentCard: Codable, Sendable {
    let name: String
    let description: String
    let url: String
    let version: String
    let protocolVersion: String
    let capabilities: A2ACapabilities
    let skills: [Skill]
    let ainm: AINMExtension?

    struct A2ACapabilities: Codable, Sendable {
        let streaming: Bool
        let pushNotifications: Bool
    }

    struct Skill: Codable, Sendable, Identifiable {
        let id: String
        let name: String
        let description: String
        let tags: [String]?
    }

    struct AINMExtension: Codable, Sendable {
        let protocolVersion: String
        let contentTypes: [String]
        let streaming: StreamingCapability
        let threading: ThreadingCapability
        let memory: MemoryCapability
        let tools: [Tool]
    }

    struct StreamingCapability: Codable, Sendable {
        let supported: Bool
        let parallelBlocks: Bool
        let preferredChunkSize: String
    }

    struct ThreadingCapability: Codable, Sendable {
        let subThreads: Bool
        let maxConcurrentThreads: Int?
    }

    struct MemoryCapability: Codable, Sendable {
        let archivalSupported: Bool
        let contextFramePreferred: Bool
    }

    struct Tool: Codable, Sendable, Identifiable {
        var id: String { name }
        let name: String
        let description: String
        let inputSchema: AnyCodableValue
    }
}
```

### Context Frame Model
```swift
// Source: memory.md Section 4.1
struct ContextFrame: Codable, Sendable {
    let threadId: String
    let frame: Frame

    struct Frame: Codable, Sendable {
        let activeContext: ActiveContext
        let conversationSummary: String?
        let archivalFacts: [ArchivalFact]
        let participants: [FrameParticipant]

        enum CodingKeys: String, CodingKey {
            case activeContext = "active_context"
            case conversationSummary = "conversation_summary"
            case archivalFacts = "archival_facts"
            case participants
        }
    }

    struct ActiveContext: Codable, Sendable {
        let recentMessages: [MessageSummary]
        let windowStart: String
        let messageCount: Int

        enum CodingKeys: String, CodingKey {
            case recentMessages = "recent_messages"
            case windowStart = "window_start"
            case messageCount = "message_count"
        }
    }

    struct MessageSummary: Codable, Sendable, Identifiable {
        let id: String
        let from: String
        let timestamp: String
        let summary: String
    }

    struct ArchivalFact: Codable, Sendable, Identifiable {
        let factId: String
        let key: String
        let value: AnyCodableValue
        let tags: [String]?
        let storedBy: String
        let storedAt: String

        var id: String { factId }

        enum CodingKeys: String, CodingKey {
            case factId = "fact_id"
            case key, value, tags
            case storedBy = "stored_by"
            case storedAt = "stored_at"
        }
    }

    struct FrameParticipant: Codable, Sendable {
        let identity: String
        let type: String
        let presence: String
    }

    enum CodingKeys: String, CodingKey {
        case threadId = "thread_id"
        case frame
    }
}
```

### Duration Formatting (Per Spec Section 5.4)
```swift
// Source: tool-visibility.md Section 5.4
extension Int {
    var formattedDuration: String {
        if self < 1000 {
            return "\(self)ms"
        } else if self < 60_000 {
            let seconds = Double(self) / 1000.0
            return String(format: "%.1fs", seconds)
        } else {
            let minutes = self / 60_000
            let seconds = (self % 60_000) / 1000
            return "\(minutes)m \(seconds)s"
        }
    }
}
```

### Mention Highlighting with AttributedString
```swift
// Build an AttributedString from TextContent with mention highlights
func highlightMentions(text: String, mentions: [ContentBlock.Mention]?) -> AttributedString {
    var attributed = AttributedString(text)
    guard let mentions = mentions else { return attributed }

    for mention in mentions.sorted(by: { $0.offset > $1.offset }) {
        let startIndex = attributed.index(attributed.startIndex, offsetByCharacters: mention.offset)
        let endIndex = attributed.index(startIndex, offsetByCharacters: mention.length)
        let range = startIndex..<endIndex
        attributed[range].foregroundColor = .accentColor
        attributed[range].font = .body.bold()
        // Store address as custom attribute for tap handling
    }
    return attributed
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Hide tool calls from users | Transparent tool visibility (AINM) | 2025-2026 | Users see exactly what agents do -- core AINM differentiator |
| Dump full history to agents | Structured context frames with budget | 2025 (Letta/MemGPT pattern) | Prevents context explosion, agents get curated context |
| Slash commands for bots | @-mention with autocomplete | 2024-2025 | Agents are participants, not command responders |
| No agent metadata | Agent Cards (A2A-compatible) | 2025-2026 | Capability discovery before interaction |

## Database Migrations Required

Phase 13 needs new GRDB migrations added to AppDatabase.migrate():

```swift
// v3: Tool call state tracking
migrator.registerMigration("v3_create_tool_calls") { db in
    try db.create(table: "toolCall") { t in
        t.primaryKey("callId", .text).notNull()
        t.column("threadId", .text).notNull()
            .references("thread", onDelete: .cascade)
        t.column("name", .text).notNull()
        t.column("status", .text).notNull()       // "running", "completed", "error"
        t.column("input", .text)                    // JSON
        t.column("resultSummary", .text)
        t.column("durationMs", .integer)
        t.column("errorMessage", .text)
        t.column("startedAt", .text).notNull()
        t.column("completedAt", .text)
    }
}

// v4: Agent Card cache
migrator.registerMigration("v4_create_agent_cards") { db in
    try db.create(table: "agentCard") { t in
        t.primaryKey("url", .text).notNull()        // agent_card_url
        t.column("identity", .text).notNull()       // AINM address
        t.column("cardJson", .text).notNull()       // Full JSON
        t.column("etag", .text)                     // HTTP ETag for caching
        t.column("fetchedAt", .text).notNull()      // Last fetch timestamp
    }
}

// v5: Archival memory facts (local cache)
migrator.registerMigration("v5_create_memory_facts") { db in
    try db.create(table: "memoryFact") { t in
        t.primaryKey("factId", .text).notNull()
        t.column("key", .text).notNull()
        t.column("value", .text).notNull()          // JSON
        t.column("tags", .text)                     // JSON array
        t.column("storedBy", .text).notNull()
        t.column("storedAt", .text).notNull()
    }
    try db.create(index: "idx_memoryFact_key", on: "memoryFact", columns: ["key"])
    try db.create(index: "idx_memoryFact_storedBy", on: "memoryFact", columns: ["storedBy"])
}
```

## Open Questions

1. **Agent Discovery Source**
   - What we know: Agent Cards are fetched via identity resolution chain (identity.md -> agent_card_url). The client needs AINM addresses to start resolution.
   - What's unclear: How does the client discover available agents in the first place? The spec defines fetching a KNOWN agent's card, not browsing a directory.
   - Recommendation: For v1, the discovery view shows agents the user has interacted with (from participant records in GRDB) plus a "Connect to agent" input where the user types an AINM address. A full directory/registry is a v2 feature.

2. **Deep Link URL Scheme Registration**
   - What we know: SwiftUI supports `.onOpenURL` for universal links and custom URL schemes.
   - What's unclear: Whether to use a custom scheme (`ainm://`) or universal links (`https://domain.com/thread/...`).
   - Recommendation: Use a custom URL scheme `ainm://` for v1 (simpler, no server-side AASA file needed). Format: `ainm://thread/{thread_id}` and `ainm://thread/{thread_id}/message/{message_id}`. Register in Info.plist.

3. **Memory Context Frame Display Trigger**
   - What we know: Context frames are server-push notifications (ainm.context.frame). They arrive on thread join, @-mention, or server refresh.
   - What's unclear: How prominent the memory UI should be. Always visible? On-demand panel?
   - Recommendation: Show a small "Context" pill/button in the thread toolbar. Tapping opens a sheet with active context, conversation summary, and archival facts. Non-intrusive by default.

## Environment Availability

Step 2.6: SKIPPED (no external dependencies identified). Phase 13 is purely iOS client code changes using existing SwiftUI, GRDB, and Foundation frameworks already in the project.

## Project Constraints (from CLAUDE.md)

- **Protocol-first:** All features implement protocol spec behavior (tool-visibility.md, threading.md, memory.md, agent-card.md)
- **MVVM + Repository pattern:** New views follow established ViewModel pattern (see ConversationListViewModel, MessageThreadViewModel, StreamingViewModel)
- **@Observable for state:** New ViewModels use @Observable macro (iOS 17+), consistent with existing AppState
- **GRDB.swift for persistence:** All cached data goes through GRDB with DatabaseMigrator
- **Actor isolation for networking:** AINMClient is an actor; all new networking methods must respect actor boundaries
- **SwiftUI iOS 18+:** Target platform, use latest SwiftUI APIs
- **No new dependencies:** Use Foundation URLSession for HTTP, SwiftUI for UI, GRDB for persistence
- **Ed25519 signing:** All outgoing messages signed per existing MessageSigner pattern
- **JSON-RPC 2.0 framing:** All server communication uses JSON-RPC via existing WebSocket infrastructure

## Sources

### Primary (HIGH confidence)
- tool-visibility.md -- Full tool event lifecycle, rendering guidance, error handling
- agent-card.md -- Agent Card schema, discovery flow, caching, validation
- memory.md -- Three-tier memory model, context frames, ainm.memory.* methods
- threading.md Section 4 -- Sub-threading, one-level nesting, context inheritance
- threading.md Section 9 -- @-mention structure, behavior, rendering
- content.md Section 4.1 -- TextBlock mentions array structure

### Secondary (MEDIUM confidence)
- Existing codebase analysis -- AppState.swift, AINMClient.swift, ContentBlock.swift, AppDatabase.swift patterns verified by reading source

### Tertiary (LOW confidence)
- None -- all research based on authoritative protocol specs and existing codebase

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies, builds on established codebase patterns
- Architecture: HIGH - follows existing MVVM+Repository, @Observable, actor patterns
- Pitfalls: HIGH - derived from protocol spec edge cases and SwiftUI known issues

**Research date:** 2026-03-29
**Valid until:** 2026-04-28 (stable -- all specs are project-internal and versioned)
