# Technology Stack: iOS Reference Client

**Project:** AI-Native Messaging Protocol -- v3.3 iOS Reference Client
**Researched:** 2026-03-29
**Scope:** NEW Swift packages and patterns for the iOS client. Does not re-research existing server stack (Axum, Tokio, ring, etc.)

## Existing Server Stack (DO NOT CHANGE)

The Rust server is complete and validated through v3.2. The iOS client connects to it.

| Technology | Version | Relevant to iOS Client |
|------------|---------|----------------------|
| Axum + WebSocket | 0.8.x | Server endpoint the client connects to |
| JSON-RPC 2.0 (hand-rolled) | N/A | Wire protocol the client must speak |
| Ed25519 via ring | 0.17.x | Signatures the client must produce/verify |
| RFC 8785 JCS via serde_json_canonicalizer | 0.3.x | Canonical JSON format the client must match for signing |
| UUIDv7 | 1.x | Message ID format the client must generate |
| RFC 3339 timestamps via chrono | 0.4.x | Timestamp format the client must use |

## iOS Client Stack

### Core Platform

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| SwiftUI | iOS 18+ / Xcode 16.3+ | UI framework | Declarative UI with mature `LazyVStack` for message lists, `ScrollViewReader` for auto-scroll, native animations. iOS 18 brings stability improvements critical for complex list views. The messaging domain (variable-height cells, streaming content, interactive blocks) is an ideal SwiftUI use case. |
| Swift 6.1+ | Xcode 16.3+ | Language | Strict concurrency checking enabled by default. `@MainActor` isolation, `Sendable` enforcement, and structured concurrency (`async/await`, `Actor`, `AsyncStream`) eliminate data races at compile time. Critical for a real-time messaging app with concurrent WebSocket reads, database writes, and UI updates. |
| iOS 18.0 minimum | N/A | Deployment target | Gives access to `@Observable` macro (iOS 17+), `ScrollPosition` API (iOS 18+), improved `NavigationStack` stability, and the latest SwiftUI fixes. By the time v3.3 ships, iOS 18 adoption will be 80%+. |

**Confidence:** HIGH -- Apple's own recommended patterns. WWDC 2024/2025 sessions confirm these as the canonical approach.

### Local Database

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| GRDB.swift | 7.10.0 | SQLite database toolkit | Production-grade SQLite wrapper with migrations, `ValueObservation` for reactive UI updates, compile-time SQL checking, and direct SQL when needed. SwiftData is NOT ready -- no background task support, poor large dataset performance, view-coupled design incompatible with background sync. GRDB is the consensus choice for production iOS apps needing reliable local persistence. Used by Monzo, and recommended by PointFree as their SwiftData alternative (SharingGRDB). |
| GRDBQuery | 0.11.0 | SwiftUI database observation | The official SwiftUI companion for GRDB. Provides `@Query` property wrapper that lets views directly observe database queries -- views auto-update when underlying data changes. Eliminates manual observation boilerplate. |

**Why GRDB over alternatives:**

| Alternative | Why Not |
|-------------|---------|
| SwiftData | No background task support. Performance degrades with 10K+ messages. View-coupled architecture prevents background sync. |
| Core Data | Legacy API. More boilerplate than GRDB for equivalent functionality. Poor Swift integration. |
| Realm | Proprietary (MongoDB). Includes sync features we do not need. Heavier than SQLite. |
| Raw SQLite (via sqlite3) | Too low-level. No migrations, no observation, no type safety. |

**GRDB + SwiftUI Integration Pattern:**

```swift
// Define a Queryable request
struct ConversationMessages: Queryable {
    static var defaultValue: [Message] { [] }

    var conversationId: String

    func publisher(in dbQueue: DatabaseQueue) -> AnyPublisher<[Message], Error> {
        ValueObservation
            .tracking { db in
                try Message
                    .filter(Column("conversationId") == conversationId)
                    .order(Column("timestamp").asc)
                    .fetchAll(db)
            }
            .publisher(in: dbQueue)
            .eraseToAnyPublisher()
    }
}

// In SwiftUI view
struct MessageListView: View {
    @Query(ConversationMessages(conversationId: "abc"))
    var messages: [Message]

    var body: some View {
        LazyVStack { ... }
    }
}
```

**Confidence:** HIGH -- GRDB 7.10.0 verified on GitHub (Feb 15, 2026). GRDBQuery 0.11.0 verified (Mar 15, 2025). Mature, actively maintained.

### Networking (WebSocket)

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| URLSessionWebSocketTask | iOS 13+ (built-in) | WebSocket client | Native Apple API. Zero dependency. Integrates with URLSession (auth, certificates, background). Sufficient for JSON-RPC 2.0 over WebSocket. No third-party library needed -- Starscream and SocketIO add complexity without benefit for our use case. |

**WebSocket Architecture:**

```swift
actor WebSocketConnection {
    private var task: URLSessionWebSocketTask?
    private let session: URLSession
    private let url: URL

    // Incoming messages as AsyncStream
    var messages: AsyncStream<JSONRPCMessage> {
        AsyncStream { continuation in
            receiveLoop(continuation: continuation)
        }
    }

    private func receiveLoop(continuation: AsyncStream<JSONRPCMessage>.Continuation) {
        task?.receive { [weak self] result in
            switch result {
            case .success(.string(let text)):
                if let message = try? JSONDecoder().decode(JSONRPCMessage.self, from: Data(text.utf8)) {
                    continuation.yield(message)
                }
                self?.receiveLoop(continuation: continuation) // Continue listening
            case .failure(let error):
                continuation.finish()
                self?.scheduleReconnect()
            default: break
            }
        }
    }
}
```

**Reconnection pattern:** Exponential backoff (1s, 2s, 4s, 8s, max 30s) with `NWPathMonitor` to detect network changes and trigger immediate reconnect on network restoration. Use `sendPing(pongReceiveHandler:)` every 30 seconds as keepalive.

**Why NOT alternatives:**

| Alternative | Why Not |
|-------------|---------|
| Starscream | Third-party dependency for something URLSession handles natively. Adds maintenance burden. |
| Network.framework (NWConnection) | Lower-level than needed. Use only if URLSession proves insufficient for custom close codes or network transitions. Keep as fallback option, not primary. |
| gRPC-Swift 2 | Heavy dependency. WebSocket + JSON-RPC is simpler, debuggable, and matches the protocol spec exactly. |

**Confidence:** HIGH -- URLSessionWebSocketTask is Apple's recommended approach. Well-documented since iOS 13.

### Cryptography

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Apple CryptoKit | iOS 13+ (built-in) | Ed25519 signing and verification | Native `Curve25519.Signing` provides hardware-accelerated Ed25519. Zero dependency. Exactly matches the protocol's signing requirements. `PrivateKey()` generates keys, `signature(for:)` signs data, `PublicKey.isValidSignature(_:for:)` verifies. SHA-512 is built into the Ed25519 implementation. |

**Signing flow (must match Rust server's ring implementation):**

```swift
import CryptoKit

// Generate identity keypair
let privateKey = Curve25519.Signing.PrivateKey()
let publicKey = privateKey.publicKey

// Sign a message (after JCS canonicalization)
let canonicalJSON: Data = canonicalize(message) // RFC 8785
let signature = try privateKey.signature(for: canonicalJSON)

// Verify a signature
let isValid = otherPublicKey.isValidSignature(signature, for: canonicalJSON)

// Export keys (base64 for wire format, raw for Keychain)
let pubKeyBase64 = publicKey.rawRepresentation.base64EncodedString()
```

**Critical interop requirement:** The `rawRepresentation` of CryptoKit's Ed25519 keys is the raw 32-byte key material, which matches `ring`'s Ed25519 key format. Base64-encoded public keys from the server can be imported directly:

```swift
let serverPubKeyData = Data(base64Encoded: serverPubKeyBase64)!
let serverPubKey = try Curve25519.Signing.PublicKey(rawRepresentation: serverPubKeyData)
```

**Confidence:** HIGH -- Apple documentation confirms Ed25519 via Curve25519.Signing. Raw key format compatibility with ring verified.

### JSON Canonicalization (RFC 8785 JCS)

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Custom implementation | N/A | RFC 8785 JSON Canonicalization Scheme | **No Swift library exists for RFC 8785.** The reference implementations at cyberphone/json-canonicalization cover Java, Go, Python, Ruby, PHP, and Elixir -- but not Swift. Must implement from spec. RFC 8785 is small (~500 lines of implementation): sort object keys lexicographically, serialize numbers per ECMAScript rules, no whitespace. Test vectors available at the reference repo. |

**Implementation approach:**

```swift
/// RFC 8785 JSON Canonicalization Scheme
func canonicalize(_ jsonObject: Any) -> Data {
    // 1. Parse JSON into Swift native types
    // 2. Recursively sort all object keys (Unicode code point order)
    // 3. Serialize numbers per ECMAScript Number.toString() rules
    // 4. No whitespace between tokens
    // 5. UTF-8 encode the result
}
```

**Validation:** Use the test vectors from `cyberphone/json-canonicalization/testdata/` to verify byte-identical output with the Rust `serde_json_canonicalizer` crate. This is the critical interop point -- if canonicalization differs, signatures will not verify across client and server.

**Confidence:** MEDIUM -- RFC 8785 is well-specified and small, but a custom implementation needs thorough testing against the reference test suite. This is a Phase 1 blocker: get it right before anything else.

### Key Storage

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| KeychainAccess | 4.2.2 | Secure Ed25519 private key storage | Simple Swift wrapper around iOS Keychain. Stores private key raw bytes with `.whenUnlockedThisDeviceOnly` accessibility. API: `keychain[data: "ed25519_private_key"] = privateKey.rawRepresentation`. The raw Security framework API is verbose and error-prone -- KeychainAccess provides a clean Swift interface. |

**Why KeychainAccess 4.2.2 and not raw Keychain API:**

The Security framework's `SecItemAdd`/`SecItemCopyMatching` API requires constructing `CFDictionary` with string keys, casting between `CFData`/`Data`, and manual error handling. KeychainAccess wraps this in a type-safe Swift API. The library is 4.2.2 (last release dropped iOS 8 support), stable, and widely used (8K+ stars).

**Concern:** KeychainAccess 4.2.2 was released in 2020. It works fine, but has not been updated for strict Swift 6 concurrency. If Swift 6 Sendable warnings become blockers, fall back to a thin custom wrapper around the Security framework (~100 lines).

**What gets stored in Keychain:**

| Key | Value | Accessibility |
|-----|-------|--------------|
| `ainm.identity.privateKey` | Ed25519 private key (32 bytes) | `.whenUnlockedThisDeviceOnly` |
| `ainm.identity.userId` | User's `name@provider` identifier | `.whenUnlockedThisDeviceOnly` |
| `ainm.server.authToken` | Session token from server (if applicable) | `.whenUnlockedThisDeviceOnly` |

**Confidence:** MEDIUM -- KeychainAccess is stable but old (2020). Swift 6 concurrency compliance is uncertain. Monitor during Phase 1; if issues arise, replace with ~100 lines of direct Security framework code.

### Markdown Rendering

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| MarkdownUI | 2.4.1 | Rich markdown rendering in SwiftUI | Agent messages frequently contain markdown (headers, code blocks, lists, tables, links). Native SwiftUI `Text` only supports bold/italic -- no headers, tables, code blocks, or images. MarkdownUI renders GitHub Flavored Markdown with customizable themes. In maintenance mode (author developing `Textual` successor at v0.3.1), but 2.4.1 is stable and battle-tested with 3.7K+ stars. |

**Why MarkdownUI over alternatives:**

| Alternative | Why Not |
|-------------|---------|
| Native `AttributedString` + `Text` | Only supports inline markdown (bold, italic, links). No block elements (headers, code blocks, tables, lists). Insufficient for agent response rendering. |
| Textual (gonzalezreal) | Successor to MarkdownUI but only at v0.3.1 (Jan 2025). Too immature for production. |
| WKWebView + HTML | Performance overhead of embedding a web view per message. Breaks native SwiftUI composition. |

**Usage pattern:**

```swift
import MarkdownUI

struct AgentMessageView: View {
    let content: String

    var body: some View {
        Markdown(content)
            .markdownTheme(.ainmAgent) // Custom theme matching app design
            .markdownCodeSyntaxHighlighter(
                .highlightJS(theme: .atomOneDark)
            )
    }
}
```

**Confidence:** MEDIUM -- MarkdownUI is in maintenance mode, not dead. It handles GFM well. If Textual reaches 1.0 during development, evaluate migration. Low risk since MarkdownUI still works.

### Code Syntax Highlighting

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| HighlightSwift | 1.1.0+ | Code block syntax highlighting | Wraps highlight.js for Swift/SwiftUI. Supports 50+ languages, 30+ themes, automatic language detection. Agents send code blocks constantly -- syntax highlighting is table stakes for readability. Integrates directly with MarkdownUI's code syntax highlighter protocol. |

**Confidence:** MEDIUM -- HighlightSwift uses JavaScriptCore under the hood (via highlight.js). Performance should be monitored with many code blocks in a single conversation. If sluggish, fall back to static coloring.

### Image Loading

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Nuke | 12.9.0 | Async image loading and caching | Protocol supports `ImageBlock` content. Nuke provides progressive loading, disk/memory caching, image processing pipeline, and `LazyImage` SwiftUI view. 12.9.0 is stable; 13.0 beta adopts `@globalActor` for Swift 6 data race safety but is not yet stable. |

**Why Nuke over alternatives:**

| Alternative | Why Not |
|-------------|---------|
| AsyncImage (native) | No disk caching, no progressive loading, no image processing. Insufficient for a messaging app with many images. |
| Kingfisher | Viable alternative, but Nuke is lighter and has better Swift Concurrency support. |
| SDWebImage | Objective-C legacy. Nuke is Swift-native. |

**Confidence:** HIGH -- Nuke 12.9.0 is mature (11 years, 127 releases). Well-maintained.

### Data Structures

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| swift-collections | 1.4.1 | OrderedDictionary, Deque | `OrderedDictionary` for preserving message/participant insertion order while maintaining O(1) lookup. `Deque` for efficient message buffer (append to end, remove from front for pagination). Apple's official package. |

**Confidence:** HIGH -- Apple-maintained, 1.4.1 verified (Mar 2026).

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| swift-algorithms | 1.2.0 | Sequence/collection algorithms | Chunking messages by date, unique filtering, windowed operations on message lists |
| OSLog | Built-in | Structured logging | Preferred over `print()` for debug/release logging. Integrates with Console.app and Instruments. No third-party logging dependency needed. |

## Architecture Patterns

### MVVM + Repository

| Pattern | Choice | Rationale |
|---------|--------|-----------|
| Architecture | **MVVM + Repository** | MVVM is natural for SwiftUI's `@Observable`. Repository pattern isolates data access (WebSocket + GRDB). TCA is overkill for a v1 client. |
| State management | **@Observable macro** | Simpler than `ObservableObject`. Tracks individual property changes (not whole object). Apple's recommended approach from WWDC 2024. |
| Navigation | **NavigationStack + NavigationPath** | Type-safe, programmatic navigation via route enums. Deep linking = constructing the path array from a URL. |
| Dependency injection | **SwiftUI @Environment + Protocols** | `@Environment(\.database)` for GRDB, `@Environment(\.webSocket)` for connection. Protocols for testability. |
| Concurrency | **Actor isolation** | `WebSocketConnection` as an `actor`. Database operations via GRDB's built-in serial queue. UI updates on `@MainActor`. |

### Layer Diagram

```
+--------------------------------------------------+
|                  SwiftUI Views                    |
|  (MessageListView, ConversationListView, etc.)   |
+--------------------------------------------------+
           |                           |
     @Observable                  @Query (GRDBQuery)
           |                           |
+--------------------+    +------------------------+
|    ViewModels      |    |  Database Observation   |
| (ConversationVM,   |    |  (ValueObservation)     |
|  AgentDiscoveryVM) |    +------------------------+
+--------------------+               |
           |                         |
+--------------------------------------------------+
|              Repository Layer                     |
| MessageRepository, ConversationRepository,        |
| AgentRepository, IdentityRepository               |
+--------------------------------------------------+
      |              |              |
+----------+  +----------+  +-------------+
| WebSocket|  |   GRDB   |  |  Keychain   |
| Service  |  | Database |  |  (Keys)     |
+----------+  +----------+  +-------------+
      |
  URLSessionWebSocketTask
      |
  Rust Server (JSON-RPC 2.0 over WebSocket)
```

## Integration Points with Existing Rust Server

### Wire Protocol Compatibility

The iOS client must implement the exact same JSON-RPC 2.0 message format as the demo CLI clients:

| Concern | Server Format | iOS Implementation |
|---------|--------------|-------------------|
| Message envelope | JSON-RPC 2.0 (`jsonrpc`, `method`, `params`, `id`) | Codable structs matching the format |
| Message IDs | UUIDv7 (time-ordered) | `import Foundation; UUID()` does NOT produce UUIDv7. Need custom UUIDv7 generation or a library. |
| Timestamps | RFC 3339 via chrono | `ISO8601DateFormatter` with `.withInternetDateTime` |
| Signatures | Ed25519 over JCS-canonicalized JSON, base64-encoded | CryptoKit + custom JCS + base64 |
| Public keys | Base64-encoded 32-byte raw Ed25519 keys | `Curve25519.Signing.PublicKey(rawRepresentation:)` |
| Content blocks | JSON objects with `type` discriminator | Codable enum with custom decoder |

### UUIDv7 Generation

**Problem:** Foundation's `UUID()` generates UUIDv4 (random). The protocol uses UUIDv7 (time-ordered, important for message ordering).

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Custom UUIDv7 | N/A | Time-ordered message IDs | UUIDv7 is simple to implement: 48-bit Unix timestamp (ms), version nibble, 12-bit random, variant bits, 62-bit random. ~30 lines of Swift. No library needed. Must produce the same format as Rust's `uuid` crate with `v7` feature. |

**Confidence:** HIGH -- UUIDv7 (RFC 9562) is well-specified. Implementation is trivial.

### AINM Handshake (`ainm.initialize`)

The client must implement the initialization handshake:

1. Open WebSocket to server
2. Send `ainm.initialize` with protocol version, user identity, capabilities
3. Receive `ainm.initialized` with server identity, protocol version, server capabilities
4. Begin normal message exchange

This is a critical first integration point. Test against the existing Rust demo server.

## What NOT to Add

| Temptation | Why Avoid |
|------------|-----------|
| **Combine framework** | Swift Concurrency (`async/await`, `AsyncStream`) replaces Combine for new SwiftUI projects. Combine is not evolving. GRDB and URLSession both support async/await natively. |
| **RxSwift** | Same as Combine but third-party. Swift Concurrency is the standard. |
| **Alamofire** | We are WebSocket-first, not REST-first. URLSession handles the minimal HTTP needs (agent card fetches). |
| **SwiftData** | Immature. No background sync. Performance degrades at scale. GRDB is the right choice. |
| **Core Data** | Legacy. GRDB does everything Core Data does with less code. |
| **TCA (The Composable Architecture)** | Over-engineered for a v1 reference client. MVVM + @Observable is simpler and sufficient. TCA's learning curve delays shipping. |
| **swift-protobuf** | Protocol uses JSON on the wire. Protobuf is a potential v2 optimization, not v1. |
| **gRPC-Swift** | Protocol transport is WebSocket + JSON-RPC. gRPC adds complexity without benefit. |
| **Starscream** | URLSessionWebSocketTask is sufficient. No need for third-party WebSocket library. |
| **Firebase / CloudKit** | We have our own server. No BaaS needed. |
| **CocoaPods / Carthage** | Swift Package Manager only. No legacy dependency managers. |

## Swift Package Manager Dependencies

```swift
// Package.swift (or Xcode project SPM dependencies)
dependencies: [
    // Database
    .package(url: "https://github.com/groue/GRDB.swift.git", from: "7.10.0"),
    .package(url: "https://github.com/groue/GRDBQuery.git", from: "0.11.0"),

    // Keychain
    .package(url: "https://github.com/kishikawakatsumi/KeychainAccess.git", from: "4.2.2"),

    // Markdown rendering
    .package(url: "https://github.com/gonzalezreal/swift-markdown-ui.git", from: "2.4.1"),

    // Code syntax highlighting
    .package(url: "https://github.com/appstefan/HighlightSwift.git", from: "1.1.0"),

    // Image loading
    .package(url: "https://github.com/kean/Nuke.git", from: "12.9.0"),

    // Data structures
    .package(url: "https://github.com/apple/swift-collections.git", from: "1.4.1"),

    // Algorithms
    .package(url: "https://github.com/apple/swift-algorithms.git", from: "1.2.0"),
]
```

**Total external dependencies: 8 packages.**

Everything else (WebSocket, CryptoKit, JCS, UUIDv7, OSLog, NavigationStack) is built-in or custom-implemented.

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Local DB | GRDB.swift 7.10 | SwiftData | No background sync, poor large dataset performance |
| Local DB | GRDB.swift 7.10 | Core Data | Legacy API, more boilerplate |
| WebSocket | URLSessionWebSocketTask | Starscream | Native API sufficient, zero-dependency |
| WebSocket | URLSessionWebSocketTask | Network.framework | Too low-level for primary use; keep as fallback |
| Crypto | Apple CryptoKit | CryptoSwift | CryptoKit is native, hardware-accelerated, Ed25519 built-in |
| Keychain | KeychainAccess 4.2.2 | Raw Security framework | Verbose API, error-prone. KeychainAccess simplifies. |
| Markdown | MarkdownUI 2.4.1 | Native AttributedString | No block elements (headers, tables, code blocks) |
| Markdown | MarkdownUI 2.4.1 | Textual 0.3.1 | Too immature (pre-1.0) |
| Images | Nuke 12.9 | AsyncImage | No caching, no progressive loading |
| Images | Nuke 12.9 | Kingfisher | Nuke is lighter, better Swift Concurrency |
| Architecture | MVVM + @Observable | TCA | Over-engineered for v1. Delays shipping. |
| State | @Observable macro | ObservableObject | @Observable is more efficient (per-property tracking) |
| JCS | Custom implementation | N/A | No Swift JCS library exists |
| UUIDv7 | Custom implementation | N/A | Trivial (~30 lines). No library needed. |

## Version Verification

| Technology | Verified Version | Source | Date Checked |
|------------|-----------------|--------|--------------|
| GRDB.swift | 7.10.0 | GitHub releases | 2026-03-29 |
| GRDBQuery | 0.11.0 | GitHub / Swift Package Index | 2026-03-29 |
| KeychainAccess | 4.2.2 | GitHub releases | 2026-03-29 |
| MarkdownUI | 2.4.1 | GitHub (maintenance mode) | 2026-03-29 |
| HighlightSwift | 1.1.0+ | GitHub | 2026-03-29 |
| Nuke | 12.9.0 | GitHub / Swift Package Index | 2026-03-29 |
| swift-collections | 1.4.1 | GitHub releases | 2026-03-29 |
| swift-algorithms | 1.2.0 | GitHub | 2026-03-29 |
| Apple CryptoKit | Built-in (iOS 13+) | Apple Developer Docs | 2026-03-29 |
| URLSessionWebSocketTask | Built-in (iOS 13+) | Apple Developer Docs | 2026-03-29 |

## Risk Assessment

| Risk | Severity | Mitigation |
|------|----------|------------|
| RFC 8785 JCS custom implementation produces different output than Rust's `serde_json_canonicalizer` | **Critical** | Use reference test vectors. Write cross-platform test: Rust server signs, iOS client verifies (and vice versa). This is Phase 1, Task 1. |
| KeychainAccess 4.2.2 has Swift 6 strict concurrency warnings | **Low** | Isolate Keychain access behind an `actor`. If warnings are blockers, replace with ~100 lines of raw Security framework code. |
| MarkdownUI in maintenance mode stops working on future iOS | **Low** | Monitor Textual maturity. If MarkdownUI breaks, Textual should be 1.0+ by then. Worst case: use `AttributedString` for basic formatting + `WKWebView` for complex blocks. |
| UUIDv7 custom implementation drift from Rust's `uuid` crate | **Low** | UUIDv7 is spec-defined (RFC 9562). Validate with comparison tests against server-generated IDs. Format is the same; only generation timing differs. |

## Sources

- [GRDB.swift v7.10.0](https://github.com/groue/GRDB.swift) -- Latest release Feb 15, 2026
- [GRDBQuery v0.11.0](https://github.com/groue/GRDBQuery) -- SwiftUI companion for GRDB
- [Apple CryptoKit - Curve25519.Signing](https://developer.apple.com/documentation/cryptokit/curve25519/signing) -- Ed25519 signing
- [URLSessionWebSocketTask](https://developer.apple.com/documentation/foundation/urlsessionwebsockettask) -- Native WebSocket
- [KeychainAccess v4.2.2](https://github.com/kishikawakatsumi/KeychainAccess/releases/tag/v4.2.2) -- Keychain wrapper
- [MarkdownUI](https://github.com/gonzalezreal/swift-markdown-ui) -- Markdown rendering (maintenance mode)
- [HighlightSwift](https://github.com/appstefan/HighlightSwift) -- Code syntax highlighting
- [Nuke v12.9.0](https://github.com/kean/Nuke) -- Image loading
- [swift-collections v1.4.1](https://github.com/apple/swift-collections) -- OrderedDictionary, Deque
- [RFC 8785 JCS reference implementations](https://github.com/cyberphone/json-canonicalization) -- Test vectors for JCS validation
- [RFC 9562 - UUIDs](https://www.rfc-editor.org/rfc/rfc9562) -- UUIDv7 specification
- [@Observable macro migration](https://developer.apple.com/documentation/SwiftUI/Migrating-from-the-observable-object-protocol-to-the-observable-macro) -- Apple's migration guide
- [Approachable Concurrency in Swift 6.2](https://www.avanderlee.com/concurrency/approachable-concurrency-in-swift-6-2-a-clear-guide/) -- Swift concurrency patterns
- [PointFree SharingGRDB](https://www.pointfree.co/blog/posts/168-sharinggrdb-a-swiftdata-alternative) -- GRDB as SwiftData alternative
- [Donny Wals - WebSocket AsyncStream](https://www.donnywals.com/iterating-over-web-socket-messages-with-async-await-in-swift/) -- WebSocket + async/await patterns
