# Phase 12: Streaming & Interactive Blocks - Research

**Researched:** 2026-03-29
**Domain:** iOS real-time streaming UI, interactive content blocks, SwiftUI performance
**Confidence:** HIGH

## Summary

Phase 12 adds two major capabilities to the AINM iOS client: (1) real-time token-by-token streaming of agent responses with cursor animation, cancellation, and parallel block support, and (2) five new interactive content block types (ButtonBlock, FormBlock, TableBlock, ApprovalBlock, ActionBlock) that enable structured human-agent interaction within threads.

The streaming subsystem requires a dedicated `StreamingManager` actor that receives `ainm.stream.start`, `ainm.stream.delta`, and `ainm.stream.end` JSON-RPC notifications over the existing WebSocket connection, accumulates deltas per block index in an in-memory buffer, and publishes updates to an `@Observable` view model at a throttled rate (every 16-33ms) to maintain 30-60fps without main thread starvation. The key architectural challenge is the ephemeral-to-permanent transition: streaming deltas are transient WebSocket events while the collapsed permanent message arrives via `ainm.message.deliver` after stream-end.

The interactive block subsystem extends the existing `ContentBlock` enum with five new cases, adds new SwiftUI views for each block type, and implements the ActionBlock response flow where user interactions (button taps, form submissions, approval selections, table row actions) generate signed message envelopes sent back through the protocol. The `ContentBlockRenderer` dispatcher gains new cases. Client-side form validation uses the regex patterns from the spec. The ApprovalBlock implements a simple state machine (pending -> approved/rejected/changes_requested).

**Primary recommendation:** Structure implementation in four plans: (1) streaming data layer + AINMClient protocol handling, (2) streaming UI + cancel flow, (3) interactive block models + renderers, (4) ActionBlock response flow + typing/presence indicators.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| STR-01 | Token-by-token streaming with cursor animation at 30-60fps | StreamingManager actor with throttled UI updates, DisplayLink-driven cursor animation |
| STR-02 | Typing/processing indicators for humans and agents | `ainm.presence.notify` handler + per-participant indicator UI in thread view |
| STR-03 | Cancel active streaming response | `ainm.stream.cancel` JSON-RPC request via AINMClient, "Stop Generating" button |
| STR-04 | Parallel block streaming | Per-block-index delta buffers in StreamingManager, independent rendering per block |
| INT-01 | ButtonBlocks with semantic styles + ActionBlock on tap | ButtonBlockView with primary/secondary/danger styles, confirmation dialog, ActionBlock message sending |
| INT-02 | FormBlocks with field types + client-side validation | FormBlockView with 8 field types, regex validation, required field checks |
| INT-03 | TableBlocks with horizontal scroll + row actions | TableBlockView with ScrollView(.horizontal), sortable columns, row action buttons |
| INT-04 | ApprovalBlocks with state machine | ApprovalBlockView with pending/approved/rejected/changes_requested states |
| INT-05 | Fallback text for unrecognized block types | Already implemented via `.unknown(fallback:)` case -- extend to new interactive types |
</phase_requirements>

## Standard Stack

### Core (already in project)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| SwiftUI | iOS 18+ | All UI rendering | Project constraint from Phase 10 |
| Swift Concurrency | Swift 6.1+ | Async streaming, actor isolation | Project constraint -- strict concurrency |
| GRDB.swift | 7.10.x | Local persistence, observation | Already integrated in Phase 10 |
| Apple CryptoKit | iOS 18+ | Ed25519 signing for ActionBlock messages | Already integrated in Phase 10 |
| URLSessionWebSocketTask | iOS 18+ | WebSocket transport | Already integrated in Phase 10 |

### No New Dependencies Required

Phase 12 requires zero new package dependencies. All streaming and interactive block functionality is implementable with the existing stack. Specifically:

- **Streaming UI throttling:** Use `DispatchSource.makeTimerSource` or `Task.sleep` for delta coalescing -- no external timer library needed
- **Form validation:** ECMAScript regex patterns from the spec are compatible with Swift's `NSRegularExpression` -- no regex library needed
- **Cursor animation:** SwiftUI's built-in animation system with `withAnimation` and `Animation.easeInOut.repeatForever` handles the blinking cursor
- **Table horizontal scroll:** Native `ScrollView(.horizontal)` -- no custom scroll view library needed

## Architecture Patterns

### Recommended Project Structure Additions

```
ios/AINMMessenger/AINMMessenger/
  Models/
    ContentBlock.swift          # EXTEND: add button, form, table, approval, action cases
    StreamEvent.swift           # NEW: stream.start, stream.delta, stream.end models
    MessageEnvelope.swift       # EXTEND: add .action MessageType case
  Networking/
    AINMClient.swift            # EXTEND: handle stream notifications, send cancel/action
    StreamingManager.swift      # NEW: actor managing active streams, delta accumulation
  UI/
    ContentBlocks/
      ContentBlockRenderer.swift  # EXTEND: new interactive block dispatch
      ButtonBlockView.swift       # NEW
      FormBlockView.swift         # NEW
      TableBlockView.swift        # NEW
      ApprovalBlockView.swift     # NEW
      ActionBlockView.swift       # NEW (read-only display of action records)
    MessageThread/
      StreamingMessageView.swift  # NEW: renders in-progress streaming messages
      TypingIndicatorView.swift   # NEW: dots animation for typing/processing
      StopGeneratingButton.swift  # NEW
    Shared/
      CursorView.swift            # NEW: blinking cursor animation
  Persistence/
    AppDatabase.swift             # EXTEND: v3 migration for metadata column
    MessageRecord.swift           # EXTEND: optional metadata JSON column
```

### Pattern 1: StreamingManager Actor (Core Architecture)

**What:** A dedicated Swift actor that manages the lifecycle of active streams. It receives raw JSON-RPC notifications from `AINMClient`, maintains per-stream state (block layout, accumulated deltas per block index), and publishes observable state for the UI.

**When to use:** Every streaming event flows through this actor.

**Design:**

```swift
actor StreamingManager {
    struct ActiveStream {
        let streamId: String
        let messageId: String
        let from: String
        let threadId: String
        let timestamp: String
        let blockLayout: [BlockLayoutEntry]
        var accumulatedContent: [String]  // indexed by block position
        var language: [String?]           // language hint per block
    }

    struct BlockLayoutEntry: Sendable {
        let type: String    // "text" or "code"
        let version: String
        let source: String
    }

    private var activeStreams: [String: ActiveStream] = [:]  // streamId -> stream

    func handleStreamStart(_ params: StreamStartParams) -> ActiveStream { ... }
    func handleDelta(_ params: StreamDeltaParams) -> (streamId: String, blockIndex: Int, newContent: String)? { ... }
    func handleStreamEnd(_ params: StreamEndParams) -> ActiveStream? { ... }
    func handleStreamError(_ params: StreamErrorParams) -> ActiveStream? { ... }
    func cancelStream(_ streamId: String) -> String? { ... }  // returns streamId if found
}
```

### Pattern 2: Throttled UI Updates for Streaming

**What:** Deltas arrive at 100+ per second but SwiftUI rerenders should happen at most every 16-33ms (30-60fps). The view model coalesces deltas using a timer-based flush pattern.

**When to use:** Any streaming text display.

**Design:**

```swift
@MainActor
@Observable
final class StreamingViewModel {
    var activeStreams: [String: StreamingMessageState] = [:]  // threadId:streamId -> state

    struct StreamingMessageState: Identifiable {
        let id: String  // streamId
        let messageId: String
        let from: String
        let threadId: String
        var blocks: [StreamingBlockState]
        var isCancelling: Bool = false
    }

    struct StreamingBlockState {
        let type: String
        var content: String = ""
        var language: String?
    }

    // Called from StreamingManager at throttled intervals
    func updateStreamContent(streamId: String, blockIndex: Int, content: String) { ... }
    func streamStarted(_ state: StreamingMessageState) { ... }
    func streamEnded(streamId: String) { ... }
}
```

The key insight: the `StreamingManager` actor accumulates ALL deltas immediately (for correctness), but only pushes UI updates at throttled intervals using a Task with `Task.sleep(for: .milliseconds(33))` in a loop.

### Pattern 3: Ephemeral-to-Permanent Transition

**What:** When `ainm.stream.end` arrives, the streaming placeholder must seamlessly transition to the permanent signed message that arrives via `ainm.message.deliver`.

**When to use:** Every stream completion.

**Flow:**
1. `ainm.stream.end` received -> remove stream from `StreamingViewModel.activeStreams`
2. The collapsed `ainm.message.deliver` arrives -> persisted to GRDB via existing write-through pattern
3. `MessageThreadViewModel` observes GRDB and picks up the new message
4. Because the `message_id` was pre-assigned at `stream.start`, the UI can match the streaming placeholder to the permanent message

**Critical detail:** The `message_id` from `stream.start` is pre-assigned. The streaming placeholder should use this ID. When the permanent message with the same ID arrives in GRDB, the observation automatically replaces the placeholder.

### Pattern 4: Interactive Block ActionBlock Response Flow

**What:** When a user interacts with a ButtonBlock, FormBlock, TableBlock, or ApprovalBlock, the client constructs a new MessageEnvelope with type `.action` containing an ActionBlock, signs it with Ed25519, and sends it via `ainm.message.send`.

**When to use:** Every interactive block user interaction.

**Flow:**
1. User taps button / submits form / clicks row action / selects approval option
2. If confirmation required (ButtonBlock), show alert first
3. Construct ActionBlock with `ref_message_id`, `ref_block_index`, `action_id`, and `data`
4. Wrap in MessageEnvelope with `type: .action`
5. Send via existing `AINMClient.sendMessage()` (already handles signing + persistence)

### Anti-Patterns to Avoid

- **Rendering every delta individually:** At 100+ deltas/sec, each SwiftUI rerender costs ~1ms. Rerendering 100 times/sec will starve the main thread. MUST coalesce deltas and update UI at 30-60fps max.
- **Storing streaming deltas in GRDB:** Streaming events are ephemeral by spec design. Only the collapsed permanent message is persisted. Writing 100+ DB rows/sec would kill performance.
- **Using `@State` for streaming content in deeply nested views:** State changes propagate up the view hierarchy. Use `@Observable` at the ViewModel level with targeted updates.
- **Rebuilding the entire message list on every delta:** Only the streaming message view should update. Use stable `Identifiable` IDs and SwiftUI's diffing to isolate rerenders.
- **Signing streaming deltas:** The spec explicitly states deltas are ephemeral and NOT individually signed. Only the collapsed message is signed.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| ECMAScript regex validation | Custom regex engine | `NSRegularExpression` with spec patterns | Swift's regex engine handles ECMAScript patterns; just strip implicit `^`/`$` anchors (spec says they're implicit) |
| Blinking cursor animation | Custom CADisplayLink timer | `withAnimation(.easeInOut(duration: 0.5).repeatForever(autoreverses: true))` on opacity | SwiftUI animation system handles this cleanly |
| Table sorting | Custom sort algorithm | `Array.sorted(by:)` with column key path | Client-side only per spec; Swift's built-in sorting suffices |
| Date picker for form fields | Custom calendar UI | `DatePicker` SwiftUI component | Native component matches platform conventions |
| Confirmation dialogs | Custom modal view | `.alert()` or `.confirmationDialog()` modifier | Native SwiftUI pattern for ButtonBlock confirmation |

## Common Pitfalls

### Pitfall 1: Main Thread Starvation from High-Frequency Delta Updates

**What goes wrong:** Agent streams 100+ deltas/second. Each delta triggers a SwiftUI state change. The main thread spends all its time re-rendering and the UI becomes unresponsive (frozen scroll, unresponsive buttons).

**Why it happens:** Naive implementation sets `@Observable` property on every delta. SwiftUI's observation tracking triggers view body evaluation on every property change.

**How to avoid:** Implement a delta coalescing buffer in the `StreamingManager` actor. Flush accumulated content to the `@Observable` ViewModel at most every 16-33ms. The ViewModel updates its published state once per flush, not once per delta.

**Warning signs:** Xcode Instruments shows main thread usage > 60% during streaming. UI scroll becomes jerky. "Stop Generating" button takes > 200ms to respond.

### Pitfall 2: Race Condition Between Stream End and Collapsed Message

**What goes wrong:** `ainm.stream.end` arrives, the streaming placeholder is removed, but the collapsed `ainm.message.deliver` hasn't arrived yet. There's a visual gap where the message disappears.

**Why it happens:** Stream-end and collapsed message are separate WebSocket events. Network jitter or processing delay creates a gap.

**How to avoid:** On `stream.end`, do NOT immediately remove the streaming placeholder. Instead, transition it to a "finalizing" state that still shows the accumulated content. When the collapsed message arrives via GRDB observation (matching by `message_id`), the transition is seamless.

**Warning signs:** Message "flashes" (disappears then reappears) at end of streaming.

### Pitfall 3: ContentBlock Enum Explosion with Codable

**What goes wrong:** Adding 5 new cases to the `ContentBlock` enum breaks the existing `init(from:)` decoder or creates a massive switch statement that's hard to maintain.

**Why it happens:** The current `ContentBlock` already handles 4 types + unknown with a manual `Codable` implementation. Adding 5 more requires careful extension.

**How to avoid:** The existing pattern of type-discriminated decoding (`switch typeString`) is correct and should be extended consistently. Each new type (button, form, table, approval, action) gets its own inner struct conforming to `Codable, Sendable`, and the `init(from:)` adds new `case` branches. The `.unknown(fallback:)` case already handles unrecognized types gracefully -- this is the INT-05 requirement already partially satisfied.

### Pitfall 4: Form Validation Regex Mismatch

**What goes wrong:** The spec says form field `validation` patterns use ECMAScript regex syntax. Swift's `NSRegularExpression` uses ICU regex syntax. Some patterns may behave differently.

**Why it happens:** ECMAScript and ICU regex have subtle differences (e.g., `\d` in ECMAScript matches only ASCII digits, ICU matches Unicode digits).

**How to avoid:** For the v1 client, the difference is negligible for typical form validation patterns (email, version strings, etc.). Document the limitation. If strict ECMAScript compliance is needed later, `JavaScriptCore` can evaluate regexes natively, but this is overkill for v1.

### Pitfall 5: Memory Leak from Uncleaned Active Streams

**What goes wrong:** If a WebSocket disconnects mid-stream, the `StreamingManager` retains the `ActiveStream` state indefinitely.

**Why it happens:** No stream-end or stream-error event arrives because the connection dropped.

**How to avoid:** Implement a cleanup method on `StreamingManager` called from `AINMClient.handleDisconnect()`. Clear all active streams and mark any streaming placeholders as "unverified" per the spec's security model (streaming.md Section 9.3).

### Pitfall 6: Approval State Stale After ActionBlock Error

**What goes wrong:** User taps "Approve", client sends ActionBlock, server returns error `-32071` (already resolved by someone else). The client still shows the approval as pending.

**Why it happens:** The client didn't handle the error response to update the local state.

**How to avoid:** On receiving `-32071`, parse the `data` field for `current_status` and update the local ApprovalBlock rendering to show the resolved state. Show a toast/alert explaining someone else already resolved it.

## Code Examples

### Stream Start/Delta/End Model Definitions

```swift
// Source: spec/streaming.md Sections 3.1, 3.2, 3.3
struct StreamStartParams: Codable, Sendable {
    let streamId: String
    let messageId: String
    let from: String
    let threadId: String
    let timestamp: String
    let blocks: [BlockLayout]

    struct BlockLayout: Codable, Sendable {
        let type: String
        let version: String
        let source: String
    }

    enum CodingKeys: String, CodingKey {
        case streamId = "stream_id"
        case messageId = "message_id"
        case from
        case threadId = "thread_id"
        case timestamp
        case blocks
    }
}

struct StreamDeltaParams: Codable, Sendable {
    let streamId: String
    let blockIndex: Int
    let delta: String
    let language: String?

    enum CodingKeys: String, CodingKey {
        case streamId = "stream_id"
        case blockIndex = "block_index"
        case delta
        case language
    }
}

struct StreamEndParams: Codable, Sendable {
    let streamId: String
    let status: String  // "completed" or "cancelled"

    enum CodingKeys: String, CodingKey {
        case streamId = "stream_id"
        case status
    }
}
```

### ContentBlock Enum Extension for Interactive Types

```swift
// Source: spec/interactive.md Sections 4-8
enum ContentBlock: Codable, Sendable {
    // Existing cases
    case text(TextContent)
    case code(CodeContent)
    case image(ImageContent)
    case file(FileContent)
    // New interactive cases (Phase 12)
    case button(ButtonContent)
    case form(FormContent)
    case table(TableContent)
    case approval(ApprovalContent)
    case action(ActionContent)
    // Fallback
    case unknown(fallback: String)

    struct ButtonContent: Codable, Sendable {
        let version: String
        let source: String
        let label: String
        let actionId: String
        let style: String?        // "primary", "secondary", "danger"
        let disabled: Bool?
        let confirmation: String?
        let fallback: String

        enum CodingKeys: String, CodingKey {
            case version, source, label, style, disabled, confirmation, fallback
            case actionId = "action_id"
        }
    }

    struct ApprovalContent: Codable, Sendable {
        let version: String
        let source: String
        let actionId: String
        let title: String
        let description: String?
        let status: String           // "pending", "approved", "rejected", "changes_requested"
        let options: [ApprovalOption]
        let fallback: String

        struct ApprovalOption: Codable, Sendable {
            let value: String
            let label: String
            let style: String?
        }

        enum CodingKeys: String, CodingKey {
            case version, source, title, description, status, options, fallback
            case actionId = "action_id"
        }
    }

    struct ActionContent: Codable, Sendable {
        let version: String
        let source: String
        let refMessageId: String
        let refBlockIndex: Int
        let actionId: String
        let data: AnyCodableValue  // flexible JSON object
        let fallback: String

        enum CodingKeys: String, CodingKey {
            case version, source, fallback, data
            case refMessageId = "ref_message_id"
            case refBlockIndex = "ref_block_index"
            case actionId = "action_id"
        }
    }
    // FormContent and TableContent follow similar patterns
}
```

### Stream Cancel Request

```swift
// Source: spec/streaming.md Section 5
// In AINMClient actor:
func cancelStream(streamId: String) async throws {
    guard case .connected = state else { throw ClientError.notConnected }

    let requestId = nextRequestId
    nextRequestId += 1

    let request: [String: Any] = [
        "jsonrpc": "2.0",
        "id": requestId,
        "method": "ainm.stream.cancel",
        "params": ["stream_id": streamId]
    ]
    let data = try JSONSerialization.data(withJSONObject: request)
    try await webSocket.send(data)

    // Wait for response confirming cancellation
    let responseData = try await waitForResponse(id: requestId)
    // Deltas may still arrive between cancel request and stream.end -- handle gracefully
}
```

### Typing Indicator from Presence Notification

```swift
// Source: spec/transport.md Section 4.4, threading.md Section 8
struct PresenceNotifyParams: Codable, Sendable {
    let identity: String
    let status: String    // "online", "processing", "away", etc.
    let threadId: String?
    let detail: String?

    enum CodingKeys: String, CodingKey {
        case identity, status, detail
        case threadId = "thread_id"
    }
}

// In AINMClient.handleIncomingMessage:
case "ainm.presence.notify":
    if let params = json["params"] as? [String: Any] {
        await handlePresenceNotify(params)
    }
```

### ActionBlock Message Construction

```swift
// Source: spec/interactive.md Section 8
func sendActionBlock(
    refMessageId: String,
    refBlockIndex: Int,
    actionId: String,
    data: [String: Any],
    fallbackText: String,
    threadId: String,
    identity: String
) async throws {
    let actionBlock = ContentBlock.action(ContentBlock.ActionContent(
        version: "1.0",
        source: "user",
        refMessageId: refMessageId,
        refBlockIndex: refBlockIndex,
        actionId: actionId,
        data: AnyCodableValue(data),
        fallback: fallbackText
    ))

    let envelope = MessageEnvelope(
        id: UUID().uuidString,
        from: identity,
        threadId: threadId,
        timestamp: ISO8601DateFormatter.ainm.string(from: Date()),
        type: .action,    // New MessageType case
        content: [actionBlock],
        signature: nil,
        parentId: nil
    )
    try await sendMessage(envelope: envelope)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `ObservableObject` + `@Published` | `@Observable` macro (iOS 17+) | WWDC 2023 | Less boilerplate, finer-grained observation -- critical for streaming perf since only accessed properties trigger updates |
| `Combine` publishers for streaming | `AsyncStream` / `AsyncSequence` | Swift 5.5+ | Native concurrency, no framework dependency, simpler cancellation |
| `List` for message threads | `LazyVStack` inside `ScrollView` | iOS 14+ (refined iOS 18) | Better control over cell lifecycle, required for streaming message injection |
| Manual `@MainActor` dispatch | Strict concurrency with `Sendable` actors | Swift 6.0+ | Compiler-enforced thread safety; StreamingManager as actor prevents data races |

## Open Questions

1. **Presence persistence across disconnects**
   - What we know: `ainm.presence.notify` is ephemeral (WebSocket notification). The spec does not define persistence of presence state.
   - What's unclear: Should the client cache last-known presence state in GRDB, or treat it as purely in-memory?
   - Recommendation: In-memory only for Phase 12. Presence is transient by nature. Store in the StreamingViewModel or a dedicated PresenceManager. Revisit if offline presence display becomes a requirement.

2. **Parallel streaming capability negotiation update**
   - What we know: The current `ClientCapabilities` struct sets `parallelStreaming: false`. Phase 12 will implement parallel streaming.
   - What's unclear: Should this be toggled to `true` as part of this phase?
   - Recommendation: Yes -- update `ClientCapabilities` to `parallelStreaming: true` and update `contentTypes` to include all new block types.

3. **Stream-end to collapsed message timing**
   - What we know: The spec says collapsed message "SHOULD be delivered immediately after stream.end" but "MAY" have a brief delay.
   - What's unclear: What timeout should the client use before giving up on the collapsed message?
   - Recommendation: Keep streaming placeholder visible for up to 5 seconds after stream.end. If collapsed message doesn't arrive, mark as "unverified" with visual indicator. The placeholder already has the accumulated content so the user experience degrades gracefully.

## Project Constraints (from CLAUDE.md)

### Technology Stack (Locked)

- **iOS 18+ / SwiftUI / Swift 6.1 strict concurrency** -- all new code must be `Sendable`-compliant with actor isolation
- **GRDB.swift** for persistence -- no SwiftData, no Core Data
- **Apple CryptoKit** for Ed25519 signing -- no third-party crypto
- **URLSessionWebSocketTask** for WebSocket -- no third-party WS library
- **@Observable** (not `ObservableObject`) for state management
- **MVVM + Repository** pattern
- **Write-through cache pattern** -- all server data flows through GRDB before UI
- **JSON-RPC 2.0** framing for all protocol messages
- **Ed25519 + RFC 8785 JCS** for message signing
- **nonisolated(unsafe)** for static formatters (established pattern from Phase 10)

### Conventions from Prior Phases

- Content blocks stored as JSON text column in `message.contentJson`
- `LazyVStack` (not `List`) for message threads with paginated loading
- GRDB observation via `ValueObservation` for real-time UI updates
- `MessageRecord` uses memberwise + envelope initializers
- `AINMClient` is an actor, not a class
- `ISO8601DateFormatter.ainm` for all timestamp formatting

## Sources

### Primary (HIGH confidence)
- `spec/streaming.md` -- Complete streaming lifecycle, delta format, cancel flow, parallel streaming, collapse semantics, error codes, security model
- `spec/interactive.md` -- All 5 block type schemas, ActionBlock format, declarative component model, form field types, approval state machine, error codes
- `spec/transport.md` -- `ainm.presence.update` and `ainm.presence.notify` method definitions and parameters
- `spec/threading.md` -- Presence semantics (Section 8), human vs agent presence states
- Existing codebase: `ContentBlock.swift`, `AINMClient.swift`, `WebSocketConnection.swift`, `MessageThreadViewModel.swift`, `ContentBlockRenderer.swift`, `MessageRecord.swift`, `AppDatabase.swift`

### Secondary (MEDIUM confidence)
- SwiftUI `@Observable` performance characteristics based on WWDC 2023/2024 sessions
- `NSRegularExpression` ECMAScript compatibility based on Apple documentation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies, extending existing proven stack
- Architecture: HIGH - streaming pattern follows established real-time messaging patterns; interactive blocks are straightforward SwiftUI views
- Pitfalls: HIGH - main thread starvation and race conditions are well-documented iOS performance concerns; specific mitigations identified from spec analysis

**Research date:** 2026-03-29
**Valid until:** 2026-04-28 (stable -- spec is v0.1.0, iOS 18 APIs are stable)
