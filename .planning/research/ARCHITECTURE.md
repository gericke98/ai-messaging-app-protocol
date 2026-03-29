# Architecture Patterns: iOS Reference Client

**Domain:** Native iOS messaging client for AINM protocol
**Researched:** 2026-03-29
**Confidence:** HIGH (protocol is complete and server is implemented; iOS patterns are well-established)

## Recommended Architecture

The iOS client follows **MVVM + Repository** with a clear layered architecture. The critical insight: the server is the source of truth for messages and threads, but the local GRDB database is the source of truth for the UI. All server data flows through the database before reaching views. This "write-through" pattern gives us offline resilience, consistent UI state, and decoupled layers.

```
+-------------------------------------------------------------+
|                      SwiftUI Views                           |
|  ConversationListView | ThreadView | AgentCardView | ...    |
+-------------------------------------------------------------+
        |  @Observable binding           ^  GRDB observation
        v                                |
+-------------------------------------------------------------+
|                     ViewModels                               |
|  ConversationListVM | ThreadVM | StreamingVM | ...           |
+-------------------------------------------------------------+
        |  Repository calls              ^  AsyncStream results
        v                                |
+-------------------------------------------------------------+
|                    Repositories                              |
|  MessageRepository | ThreadRepository | MemoryRepository     |
|  AgentRepository   | IdentityRepository                     |
+-------------------------------------------------------------+
        |          |                     ^          ^
        v          v                     |          |
+----------------+  +--------------------+  +----------------+
| NetworkService |  | PersistenceService |  | CryptoService  |
| (WebSocket +   |  | (GRDB.swift)       |  | (CryptoKit)    |
|  HTTP client)  |  |                    |  |                |
+----------------+  +--------------------+  +----------------+
        |                    |                      |
        v                    v                      v
   Rust Server          SQLite file           iOS Keychain
   (wss://)             (local)               (Ed25519 keys)
```

### Component Boundaries

| Component | Responsibility | Communicates With |
|-----------|---------------|-------------------|
| **SwiftUI Views** | Render UI, capture user input. Zero business logic. | ViewModels (read @Observable state, call actions) |
| **ViewModels** | UI state, user action handling, coordinate repositories. One per screen/feature. | Repositories (call methods, observe streams) |
| **Repositories** | Single API for a domain. Orchestrate network + persistence. Expose `AsyncStream` for observation. | NetworkService, PersistenceService, CryptoService |
| **NetworkService** | WebSocket connection lifecycle, JSON-RPC framing, send/receive. Single persistent connection. | Rust server via `wss://` |
| **PersistenceService** | GRDB database operations, migrations, observation queries. | SQLite file on disk |
| **CryptoService** | Ed25519 key generation, message signing (JCS + sign), signature verification. Keychain access. | Apple CryptoKit, iOS Keychain |

### Data Flow

#### Sending a Message (Human -> Server)

```
User taps Send
  -> ThreadViewModel.sendMessage(text)
    -> CryptoService.signEnvelope(message)       // JCS canonicalize + Ed25519 sign
    -> MessageRepository.send(signedMessage)
      -> PersistenceService.insert(message, status: .sending)  // Optimistic local insert
      -> NetworkService.sendRPC("ainm.message.send", params)   // JSON-RPC request
      -> on success: PersistenceService.update(message, status: .sent)
      -> on failure: PersistenceService.update(message, status: .failed)
    -> ThreadViewModel state updates via GRDB observation (automatic)
```

#### Receiving a Message (Server -> Human)

```
WebSocket receives ainm.message.deliver notification
  -> NetworkService.incomingMessages AsyncStream yields
    -> MessageRepository.handleDelivery(notification)
      -> CryptoService.verifySignature(message)    // Verify Ed25519
      -> PersistenceService.insertOrUpdate(message, verified: true/false)
    -> GRDB observation fires
      -> ThreadViewModel.messages updated automatically
        -> SwiftUI re-renders message list
```

#### Streaming Agent Response (Server -> Human, real-time)

```
WebSocket receives ainm.stream.start
  -> NetworkService.streamEvents AsyncStream yields StreamStart
    -> StreamingRepository.handleStreamStart(event)
      -> PersistenceService.insertStreamPlaceholder(messageId, blocks)
      -> StreamingState[streamId] = StreamBuffer(blocks: [...])

WebSocket receives ainm.stream.delta (many times)
  -> StreamingRepository.handleDelta(event)
    -> StreamingState[streamId].appendDelta(blockIndex, text)
    -> StreamingViewModel.liveText updated (in-memory, NOT persisted per delta)
    -> SwiftUI re-renders incrementally

WebSocket receives ainm.stream.end
  -> StreamingRepository.handleStreamEnd(event)
    -> PersistenceService.replaceStreamPlaceholder(
         messageId, collapsedMessage, verified: CryptoService.verify(...)
       )
    -> StreamingState.remove(streamId)
    -> GRDB observation fires -> final message rendered
```

**Critical design decision:** Stream deltas are held in-memory only. The collapsed signed message at stream-end is what gets persisted to GRDB. This matches the protocol's design (deltas are ephemeral, only the collapsed message is signed and canonical).

#### Offline / Reconnection Flow

```
Connection drops (detected by URLSessionWebSocketTask delegate)
  -> NetworkService.connectionState = .disconnected
    -> All ViewModels observe this state -> show offline banner

User sends message while offline:
  -> PersistenceService.insert(message, status: .queued)
  -> Message appears in UI with "pending" indicator

Connection restored:
  -> NetworkService performs ainm.initialize handshake
  -> NetworkService.connectionState = .connected
  -> MessageRepository.flushQueuedMessages()
    -> For each queued message: send via WebSocket, update status
  -> Server may deliver missed messages via ainm.message.deliver
    -> Normal receive flow handles them
```

## Component Design Details

### 1. NetworkService (Singleton)

The single most important component. Manages the one WebSocket connection to the server.

```swift
@Observable
final class NetworkService {
    // Connection state
    enum ConnectionState { case disconnected, connecting, handshaking, connected }
    var connectionState: ConnectionState = .disconnected

    // Outbound
    func sendRequest(method: String, params: Encodable) async throws -> JSONRPCResponse
    func sendNotification(method: String, params: Encodable) async throws

    // Inbound streams (multiplexed from single WebSocket)
    var messageDeliveries: AsyncStream<MessageDeliverParams>
    var streamEvents: AsyncStream<StreamEvent>        // start/delta/end/error
    var presenceUpdates: AsyncStream<PresenceUpdate>
    var memoryNotifications: AsyncStream<MemoryEvent>
    var contextFrames: AsyncStream<ContextFrame>

    // Lifecycle
    func connect(to url: URL, identity: String, signature: String) async throws
    func disconnect() async
}
```

**Implementation pattern:** The WebSocket receive loop runs as a detached Task. It deserializes each incoming JSON-RPC message, inspects the `method` field, and routes to the appropriate `AsyncStream.Continuation.yield()`. Pending requests (messages with `id`) are tracked in a `[String: CheckedContinuation]` dictionary for request/response correlation.

**Why a single connection:** The AINM transport spec (Section 4.1) specifies one connection per client session. All JSON-RPC methods are multiplexed on this single WebSocket.

### 2. PersistenceService (GRDB)

Uses GRDB.swift with `DatabaseQueue` (single-connection, appropriate for a single-user iOS app). Provides both write operations and observation-based reads.

**Database schema (key tables):**

```
threads
  - id TEXT PRIMARY KEY          -- UUIDv7
  - title TEXT
  - created_at TEXT              -- RFC 3339
  - updated_at TEXT
  - last_message_preview TEXT
  - unread_count INTEGER DEFAULT 0
  - is_sub_thread INTEGER DEFAULT 0
  - parent_thread_id TEXT

messages
  - id TEXT PRIMARY KEY          -- UUIDv7 (from protocol)
  - thread_id TEXT NOT NULL      -- FK to threads
  - from_address TEXT NOT NULL   -- AINM address
  - timestamp TEXT NOT NULL      -- RFC 3339
  - type TEXT NOT NULL           -- "message", "action", "system"
  - content_json TEXT NOT NULL   -- JSON array of content blocks
  - signature TEXT
  - verified INTEGER             -- 0/1/NULL
  - status TEXT NOT NULL         -- "sending", "sent", "delivered", "failed", "queued"
  - stream_id TEXT               -- non-null during active streaming
  - INDEX idx_messages_thread ON messages(thread_id, timestamp)

participants
  - thread_id TEXT NOT NULL
  - address TEXT NOT NULL
  - type TEXT NOT NULL           -- "human" or "agent"
  - presence TEXT                -- "online", "offline", "available"
  - PRIMARY KEY (thread_id, address)

archival_facts
  - fact_id TEXT PRIMARY KEY
  - key TEXT NOT NULL
  - value_json TEXT NOT NULL
  - tags_json TEXT
  - stored_by TEXT NOT NULL
  - stored_at TEXT NOT NULL

identity
  - address TEXT PRIMARY KEY
  - display_name TEXT
  - type TEXT                    -- "human" or "agent"
  - public_key_base64 TEXT
  - server_url TEXT

agent_cards
  - address TEXT PRIMARY KEY
  - card_json TEXT NOT NULL      -- Full Agent Card JSON
  - etag TEXT                    -- For conditional GET caching
  - fetched_at TEXT
```

**Observation pattern (GRDB ValueObservation):**

```swift
// In MessageRepository:
func observeMessages(threadId: String) -> AsyncValueObservation<[Message]> {
    let observation = ValueObservation.tracking { db in
        try Message
            .filter(Column("thread_id") == threadId)
            .order(Column("timestamp"))
            .fetchAll(db)
    }
    return observation.values(in: database) // AsyncSequence
}
```

ViewModels consume these as `AsyncStream` / `AsyncSequence`, which triggers SwiftUI re-renders automatically when the database changes.

### 3. CryptoService

Wraps Apple CryptoKit for AINM's specific signing requirements.

```swift
final class CryptoService {
    // Key management
    func generateKeyPair() -> Curve25519.Signing.PrivateKey
    func storePrivateKey(_ key: Curve25519.Signing.PrivateKey, for address: String) // Keychain
    func loadPrivateKey(for address: String) -> Curve25519.Signing.PrivateKey?   // Keychain

    // Message signing (matches server's signing.rs exactly)
    func signEnvelope(_ envelope: inout [String: Any]) throws -> String
    // 1. Remove "signature" key
    // 2. JCS canonicalize (RFC 8785) -- sorted keys, no whitespace
    // 3. Sign canonical UTF-8 bytes with Ed25519
    // 4. Base64url encode (no padding)
    // 5. Set envelope["signature"]

    // Verification
    func verifySignature(publicKey: Data, envelope: [String: Any]) throws -> Bool

    // Init signing (transport.md Section 4.3)
    func signInit(identity: String) throws -> String
    // Signs: "identity|timestamp" with Ed25519
}
```

**JCS (RFC 8785) implementation note:** There is no established Swift library for JSON Canonicalization Scheme. We need to implement it -- it is straightforward (serialize JSON with sorted keys, no extra whitespace, specific number formatting). This is the same algorithm the Rust server uses via `serde_json_canonicalizer`. The implementation is ~100 lines and must produce byte-identical output to the server's version for signatures to verify.

### 4. Repository Layer

Each repository coordinates one domain across network + persistence:

| Repository | Network Methods | Persistence Tables | Key Responsibility |
|-----------|----------------|-------------------|-------------------|
| **MessageRepository** | `ainm.message.send`, handles `ainm.message.deliver` | `messages` | Send messages, persist deliveries, flush offline queue |
| **ThreadRepository** | `ainm.thread.create`, `ainm.thread.join`, `ainm.thread.leave` | `threads`, `participants` | Thread CRUD, participant management |
| **StreamingRepository** | Handles `ainm.stream.start/delta/end/error/cancel` | `messages` (for collapsed) | In-memory stream buffer, collapse to persistence |
| **MemoryRepository** | `ainm.memory.store/query/evict`, handles `ainm.context.frame` | `archival_facts` | Memory operations, context frame display |
| **AgentRepository** | HTTP GET `/.well-known/ainm/{name}/agent-card.json` | `agent_cards` | Agent Card discovery with ETag caching |
| **IdentityRepository** | HTTP GET identity resolution | `identity` | Identity resolution, key caching |

### 5. ViewModel Layer

| ViewModel | Screens | Key State |
|-----------|---------|-----------|
| **ConversationListVM** | Conversation list (home screen) | `[ThreadSummary]`, sorted by last activity |
| **ThreadVM** | Message thread view | `[Message]`, `[Participant]`, send action |
| **StreamingVM** | Embedded in ThreadVM | `[StreamId: LiveStreamState]` with in-memory delta text |
| **AgentCardVM** | Agent detail/discovery view | `AgentCard`, capabilities list |
| **MemoryVM** | Memory context display | `[ArchivalFact]`, context frames |
| **SettingsVM** | Identity/connection settings | `ConnectionState`, identity address |

## Patterns to Follow

### Pattern 1: Database as Single Source of Truth (Write-Through Cache)

**What:** All server data writes to GRDB first, then UI observes GRDB. Never hold message state only in ViewModel memory (except streaming deltas).

**When:** Every received message, thread update, participant change.

**Why:** Guarantees consistency. If the app crashes mid-operation, state is recoverable. GRDB observation handles UI updates automatically -- no manual state synchronization.

```swift
// GOOD: Write-through
func handleDelivery(_ notification: MessageDeliverParams) async throws {
    let message = Message(from: notification)
    try await database.write { db in
        try message.save(db)    // GRDB handles INSERT or UPDATE
    }
    // UI updates automatically via ValueObservation -- no explicit notification needed
}

// BAD: ViewModel-only state
func handleDelivery(_ notification: MessageDeliverParams) {
    messages.append(Message(from: notification))  // Lost on app restart
}
```

### Pattern 2: AsyncStream Demultiplexer for WebSocket

**What:** Single WebSocket receive loop demultiplexes by JSON-RPC method into typed AsyncStreams.

**When:** Always. The AINM protocol multiplexes all operations on one WebSocket.

```swift
private func receiveLoop() async {
    while let message = try? await webSocket.receive() {
        guard case .string(let text) = message,
              let json = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any],
              let method = json["method"] as? String else {
            // No "method" field = this is a JSON-RPC response (has "id" + "result"/"error")
            if let id = (json?["id"] as? String) ?? (json?["id"] as? Int).map(String.init) {
                pendingRequests[id]?.resume(returning: json!)
            }
            continue
        }

        switch method {
        case "ainm.message.deliver":
            messageDeliveryContinuation.yield(decode(json))
        case "ainm.stream.start", "ainm.stream.delta", "ainm.stream.end", "ainm.stream.error":
            streamEventContinuation.yield(decode(json))
        case "ainm.presence.update":
            presenceContinuation.yield(decode(json))
        case "ainm.context.frame":
            contextFrameContinuation.yield(decode(json))
        case "ainm.memory.stored", "ainm.memory.evicted", "ainm.memory.query.result":
            memoryContinuation.yield(decode(json))
        default:
            break // Unknown notification, ignore
        }
    }
}
```

### Pattern 3: Optimistic UI for Sending

**What:** Insert messages locally with `status: .sending` before server confirms. Update to `.sent` on success, `.failed` on error.

**When:** Every outbound message.

**Why:** Instant UI feedback. Messages appear immediately in the thread. Standard pattern in all modern messaging apps (iMessage, WhatsApp, Telegram).

### Pattern 4: Streaming Delta Buffer (In-Memory Only)

**What:** Accumulate stream deltas in-memory per `stream_id`. Only persist the final collapsed message.

**When:** All agent streaming responses.

**Why:** Protocol design -- deltas are ephemeral, unsigned. Persisting hundreds of deltas per message would thrash the database. The collapsed signed message at `ainm.stream.end` is the canonical record.

```swift
@Observable
class StreamBuffer {
    let streamId: String
    let messageId: String
    var blockTexts: [String]    // One entry per block in the layout
    var isComplete: Bool = false
}
```

### Pattern 5: Request/Response Correlation with CheckedContinuation

**What:** For JSON-RPC requests (messages with `id`), use a `[String: CheckedContinuation]` map to bridge the async gap between sending a request and receiving its response.

**When:** All outbound JSON-RPC requests (`ainm.message.send`, `ainm.thread.create`, etc.).

```swift
func sendRequest<T: Decodable>(method: String, params: Encodable) async throws -> T {
    let id = UUID().uuidString
    let rpcRequest = JSONRPCRequest(jsonrpc: "2.0", id: id, method: method, params: params)

    return try await withCheckedThrowingContinuation { continuation in
        pendingRequests[id] = continuation
        webSocket.send(.string(encode(rpcRequest))) { error in
            if let error {
                pendingRequests.removeValue(forKey: id)
                continuation.resume(throwing: error)
            }
        }
    }
}
```

## Anti-Patterns to Avoid

### Anti-Pattern 1: Parsing JSON-RPC in ViewModels

**What:** ViewModel directly deserializes raw WebSocket JSON.

**Why bad:** Couples UI layer to wire protocol. Makes testing impossible without mocking WebSocket.

**Instead:** NetworkService handles all JSON-RPC framing. Repositories handle all protocol-specific logic. ViewModels work with domain model types only.

### Anti-Pattern 2: Multiple WebSocket Connections

**What:** Opening separate WebSocket connections for different features (one for messages, one for streaming, etc.).

**Why bad:** Protocol violation -- AINM specifies one connection per session. Also wastes resources and complicates session state.

**Instead:** Single NetworkService singleton with one WebSocket, demultiplexed streams.

### Anti-Pattern 3: Persisting Stream Deltas

**What:** Writing every `ainm.stream.delta` to GRDB.

**Why bad:** Hundreds of database writes per agent response. Deltas are unsigned and ephemeral by protocol design. Causes UI flicker from rapid observation updates.

**Instead:** In-memory `StreamBuffer`. Persist only the final collapsed message from `ainm.stream.end`.

### Anti-Pattern 4: SwiftData for Messages

**What:** Using SwiftData instead of GRDB for message persistence.

**Why bad:** SwiftData lacks background write support (messages arrive on WebSocket task, not main thread). Performance degrades with large datasets. No raw SQL for complex queries (unread counts, search). Missing observation granularity.

**Instead:** GRDB.swift with `ValueObservation` provides exactly the observation + background-write + raw-SQL combination messaging apps need.

### Anti-Pattern 5: Re-implementing JSON-RPC Framing Ad Hoc

**What:** Hand-rolling JSON-RPC request/response/notification handling scattered across the codebase.

**Why bad:** Error-prone, misses edge cases (error codes, null IDs, notifications vs requests).

**Instead:** Build a small but complete `JSONRPCCodec` layer that handles all framing concerns once. ~200 lines that correctly handles requests, responses, notifications, and errors per the JSON-RPC 2.0 spec.

## Sync Strategy

### Online-First, Offline-Tolerant

This is NOT an offline-first app. The server is authoritative. But the app must handle:

1. **Brief disconnections** (tunnel, elevator, network switch): Queue outbound messages locally, flush on reconnect. Inbound messages delivered by server on reconnect.

2. **App backgrounding**: iOS suspends the app. On foreground, reconnect WebSocket, receive any queued deliveries from server.

3. **Cold start with cached data**: Show conversation list and recent messages from GRDB immediately while WebSocket connects in background.

**What we do NOT need for v1:**
- Conflict resolution (server is authoritative)
- CRDTs or operational transforms (messages are append-only)
- Background sync daemon (iOS kills background WebSocket connections anyway)
- Delta sync / incremental fetch (server delivers missed messages on reconnect)

### Reconnection Strategy

```
Disconnect detected
  -> Wait 1s, attempt reconnect
  -> If fail: exponential backoff (1s, 2s, 4s, 8s, max 30s)
  -> On success: full ainm.initialize handshake
  -> Server delivers any messages queued during offline period
  -> Flush local outbound queue
```

## Build Order (Suggested Phase Sequence)

The layers have hard dependencies. Build bottom-up:

### Phase 1: Foundation (no server needed)
1. **Xcode project setup** -- SPM, target config, iOS 18 minimum
2. **GRDB schema + PersistenceService** -- migrations, all tables, basic CRUD
3. **CryptoService** -- Ed25519 keygen, JCS canonicalization, signing, verification
4. **Domain models** -- Swift types for Message, Thread, Participant, ContentBlock, etc.
5. **Unit tests** -- Verify signing produces output compatible with Rust server's `signing.rs`

**Rationale:** Crypto compatibility is the riskiest integration point. Validating early that Swift's CryptoKit Ed25519 + our JCS implementation produces identical signatures to the Rust server prevents late-stage rework.

### Phase 2: Network Layer (needs running server)
6. **JSONRPCCodec** -- Encode/decode JSON-RPC 2.0 requests, responses, notifications
7. **NetworkService** -- WebSocket connection, init handshake, receive loop, demux
8. **Request/response correlation** -- CheckedContinuation map for RPC calls
9. **Integration test** -- Connect to demo server, complete handshake, send/receive one message

### Phase 3: Core Messaging (server + persistence + network integrated)
10. **MessageRepository** -- Send, receive, persist, optimistic UI
11. **ThreadRepository** -- Create thread, manage participants
12. **ThreadViewModel + ThreadView** -- Message list, send input, basic rendering
13. **ConversationListViewModel + ConversationListView** -- Thread list with last message preview
14. **ContentBlockRenderer** -- Render text, code, image, file blocks in SwiftUI

### Phase 4: Real-Time Features
15. **StreamingRepository + StreamBuffer** -- Handle stream lifecycle
16. **StreamingViewModel** -- Live text updates in thread view
17. **Interactive block renderers** -- Button, Form, Table, Approval, Action blocks
18. **Presence indicators** -- Online/offline/available status display

### Phase 5: Advanced Features
19. **MemoryRepository + MemoryVM** -- Context frames, archival facts display
20. **AgentRepository + AgentCardVM** -- Agent Card discovery with ETag caching
21. **IdentityRepository + SettingsVM** -- Identity management, connection settings
22. **Offline queue** -- Queue outbound messages, flush on reconnect

### Phase 6: Polish
23. **Navigation** -- NavigationStack with deep linking
24. **Error handling** -- Network errors, signing failures, UI feedback
25. **Accessibility** -- VoiceOver, Dynamic Type
26. **Performance** -- Profile GRDB queries, optimize list rendering

**Dependency graph:**
```
Phase 1 (Foundation) ----+
                         |
Phase 2 (Network) ------+---> Phase 3 (Core Messaging)
                                |
                         +------+-------+
                         |              |
                   Phase 4 (RT)    Phase 5 (Advanced)
                         |              |
                         +------+-------+
                                |
                          Phase 6 (Polish)
```

## Scalability Considerations

| Concern | At 10 conversations | At 100 conversations | At 1000+ conversations |
|---------|---------------------|----------------------|------------------------|
| Message list | No issues | SQLite indexed query, fine | Need pagination (GRDB `limit/offset` or cursor) |
| GRDB observation | Instant | Instant | Use `DatabaseRegionObservation` for targeted observation |
| WebSocket throughput | Trivial | Fine | One connection handles thousands of messages/sec |
| Streaming | 1 concurrent stream | Multiple OK | Bound stream buffers to cap memory |
| SQLite file size | KB | Low MB | Implement message archival/cleanup strategy |

## Key Integration Points with Rust Server

| Integration Point | Server Side | iOS Side | Verification Method |
|-------------------|-------------|----------|-------------------|
| WebSocket handshake | `ws_upgrade_handler` + `ainm.initialize` | `NetworkService.connect()` | Successful capability exchange |
| JSON-RPC framing | `protocol.rs` types | `JSONRPCCodec` | Round-trip encode/decode test |
| Ed25519 signing | `signing.rs` + `ring` | `CryptoService` + CryptoKit | Sign on iOS, verify on server (and vice versa) |
| JCS canonicalization | `serde_json_canonicalizer` | Custom Swift implementation | Byte-identical canonical output for same input |
| Message envelope | `ainm.message.send` params | `Message` model | Schema compliance (JSON Schema 2020-12) |
| Stream lifecycle | `streaming.rs` | `StreamingRepository` | Start/delta/end sequence correctly handled |
| Content blocks | JSON content array | `ContentBlock` enum + renderers | All 9 block types rendered (text, code, image, file, button, form, table, approval, action) |
| Agent Card | `agent_card_handler` with ETag | `AgentRepository` HTTP GET | Conditional GET, ETag caching |
| Thread creation | `ainm.thread.create` handler | `ThreadRepository` | Thread ID returned and persisted |
| Memory operations | `ainm.memory.store/query/evict` handlers | `MemoryRepository` | Facts stored and retrieved |
| Presence updates | `ainm.presence.update` notifications | `participants` table + UI | Status reflected in thread participant list |

## Sources

- [GRDB.swift (v7.10.0)](https://github.com/groue/GRDB.swift) -- SQLite toolkit, observation API
- [GRDBQuery](https://github.com/groue/GRDBQuery) -- SwiftUI companion for GRDB
- [Apple CryptoKit - Curve25519.Signing](https://developer.apple.com/documentation/cryptokit/curve25519/signing) -- Ed25519 in iOS
- [URLSessionWebSocketTask](https://developer.apple.com/documentation/foundation/urlsessionwebsockettask) -- Native WebSocket API
- [Iterating over web socket messages with async/await in Swift](https://www.donnywals.com/iterating-over-web-socket-messages-with-async-await-in-swift/) -- AsyncStream wrapping pattern
- [Modern MVVM + Repository Pattern in SwiftUI](https://medium.com/@gauravios/modern-mvvm-repository-pattern-in-swiftui-eca4f78fc2f5) -- Architecture reference
- [Offline-First Sync Patterns](https://developersvoice.com/blog/mobile/offline-first-sync-patterns/) -- Sync strategy reference
- AINM spec/transport.md -- Connection lifecycle, JSON-RPC framing, capability negotiation
- AINM spec/streaming.md -- Stream start/delta/end lifecycle
- AINM spec/security.md -- Ed25519 signing procedure, JCS canonicalization
- AINM spec/content.md -- Content block types and base schema
- AINM spec/interactive.md -- Interactive block types (button, form, table, approval, action)
- AINM spec/memory.md -- Three-tier memory model, context frames
- AINM spec/identity.md -- Address format, identity resolution
- AINM demo/src/server.rs -- Server WebSocket handler implementation (integration target)
- AINM demo/src/signing.rs -- Reference signing implementation (port to Swift)
- AINM demo/src/streaming.rs -- Reference streaming implementation
- AINM demo/src/protocol.rs -- JSON-RPC type definitions (match in Swift)
