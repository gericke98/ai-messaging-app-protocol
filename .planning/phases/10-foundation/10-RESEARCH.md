# Phase 10: Foundation - Research

**Researched:** 2026-03-29
**Domain:** iOS greenfield app -- WebSocket networking, Ed25519 cryptography, JCS canonicalization, GRDB persistence, SwiftUI onboarding
**Confidence:** HIGH

## Summary

Phase 10 is a greenfield iOS project creating the networking/crypto/persistence infrastructure that all subsequent phases build on. The app must connect to the existing Rust demo server via WebSocket, complete the AINM handshake (ainm.initialize / ainm.initialized), and exchange Ed25519-signed messages with RFC 8785 JCS canonicalization verified cross-platform. The entire data path flows through GRDB as the single source of truth.

The technology stack is well-defined by CLAUDE.md decisions: SwiftUI + Swift 6.1 strict concurrency, Apple CryptoKit for Ed25519, GRDB.swift 7.10.x for persistence, URLSessionWebSocketTask for WebSocket, and a custom RFC 8785 JCS implementation (no Swift library exists). The only significant risk is JCS canonicalization correctness -- specifically UTF-16 code unit ordering for key sorting and ECMAScript number serialization rules. Shared test vectors between Rust and Swift are the primary mitigation.

**Primary recommendation:** Build the networking and crypto layers as Swift actors with async/await, validate cross-platform signing via test vectors before any integration testing, and use GRDB's DatabaseMigrator from day one to establish the schema.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Single onboarding screen with server URL field, display name field, and Connect button. Keypair generated silently on connect.
- **D-02:** No connection status indicator in Phase 10 -- defer polished status UI to Phase 11.
- **D-03:** Monolith app target with folder-based layers: App/, Networking/, Crypto/, Persistence/, UI/, Models/. Single Xcode project, no local SPM packages.
- **D-04:** iOS project lives at `ios/AINMMessenger/` relative to repo root, alongside `demo/`, `spec/`, `schemas/`.
- **D-05:** Swift 6 strict concurrency mode enabled from day one in build settings.
- **D-06:** Auto-reconnect with exponential backoff (1s, 2s, 4s, 8s... capped at 30s). Silent reconnection with subtle status -- no user action needed for transient failures.
- **D-07:** Always full re-handshake on reconnect (ainm.initialize / ainm.initialized). Stateless -- server may have restarted.
- **D-08:** Shared test vectors (JSON files) for cross-platform crypto validation. Rust generates vectors, Swift validates against them. No live server needed for unit tests.
- **D-09:** Test vectors live at `tests/vectors/` at repo root -- shared across Rust and Swift projects.
- **D-10:** Basic GitHub Actions CI in Phase 10 -- builds iOS project and runs unit tests on every push (macos-14 runner, Xcode 16).

### Claude's Discretion
- Error handling on failed connection (inline error vs alert sheet -- best SwiftUI pattern)
- Whether to pre-fill demo server URL (decided in UI-SPEC: yes, `wss://localhost:8080`)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| NET-01 | App connects to AINM server via WebSocket with JSON-RPC 2.0 framing | URLSessionWebSocketTask + AsyncStream pattern; JSON-RPC 2.0 types from protocol.rs as reference for Swift Codable models |
| NET-02 | App completes ainm.initialize handshake with capability negotiation | Transport spec Section 4.3 defines exact request/response; server.rs shows server-side handling |
| NET-03 | User can set server URL and display name during onboarding | UI-SPEC defines onboarding screen; single NavigationStack flow |
| NET-04 | App generates Ed25519 keypair and stores private key in iOS Keychain | Apple CryptoKit Curve25519.Signing + Keychain storage (see KeychainAccess assessment below) |
| NET-05 | App signs all outgoing messages with Ed25519 via Apple CryptoKit | Signing procedure from security.md Section 4; must match signing.rs sign_envelope exactly |
| NET-06 | App implements RFC 8785 JCS canonicalization matching Rust server output | Custom Swift implementation required -- no Swift library exists; Rust uses serde_json_canonicalizer 0.3 |
| PLT-01 | App caches messages, threads, and participants locally via GRDB.swift | GRDB 7.10.x with DatabaseMigrator; schema derived from threading.schema.json and security.schema.json |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| SwiftUI | iOS 18+ / Xcode 16+ | UI framework | Apple's declarative UI. Locked in CLAUDE.md |
| Apple CryptoKit | iOS 13+ (built-in) | Ed25519 signing via Curve25519.Signing | Native, hardware-accelerated, zero dependencies. Exact match for protocol's signing requirements |
| GRDB.swift | 7.10.x | Local SQLite database | Swift 6 compatible, migration system, observation for UI updates. Locked in CLAUDE.md |
| URLSessionWebSocketTask | iOS 13+ (built-in) | WebSocket client | Native Apple API, integrates with URLSession auth/certs. No third-party dependency needed |
| Foundation (JSONSerialization) | Built-in | JSON parsing for JCS implementation | Required for custom RFC 8785 canonicalization |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| KeychainAccess | 4.2.2 | Keychain wrapper for Ed25519 key storage | Store/retrieve private key bytes. **WARNING:** Last release March 2020, not Swift 6 tested -- see Pitfalls section |
| swift-collections | 1.1+ | OrderedDictionary (if needed for JCS) | Only if JCS implementation needs ordered key iteration during canonicalization |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| KeychainAccess 4.2.2 | Raw Security framework | More boilerplate but guaranteed Swift 6 safe. Fallback if KeychainAccess has Sendable issues |
| KeychainAccess 4.2.2 | SwiftKeychainKit (new, Swift 6.2 native) | Modern but very new library, less battle-tested. Consider if KeychainAccess fails |
| URLSessionWebSocketTask | Starscream / swift-websocket | Unnecessary complexity -- native API is sufficient for our use case |

**Installation (Package.swift dependencies):**
```swift
// Package dependencies for ios/AINMMessenger/
.package(url: "https://github.com/groue/GRDB.swift.git", from: "7.10.0"),
.package(url: "https://github.com/kishikawakatsumi/KeychainAccess.git", from: "4.2.2"),
```

**Version verification:** GRDB.swift 7.10.0 released 2026-02-15 (verified via Swift Package Index). KeychainAccess 4.2.2 released 2020-03-01 (last release -- see pitfalls).

## Architecture Patterns

### Recommended Project Structure
```
ios/AINMMessenger/
├── AINMMessenger.xcodeproj
├── AINMMessenger/
│   ├── App/
│   │   ├── AINMMessengerApp.swift          # @main entry point
│   │   └── AppState.swift                  # Root @Observable state
│   ├── Models/
│   │   ├── MessageEnvelope.swift           # Codable model matching security.schema.json
│   │   ├── JsonRpc.swift                   # JSON-RPC 2.0 request/response/notification types
│   │   ├── InitializeParams.swift          # ainm.initialize params (transport.schema.json)
│   │   ├── InitializeResult.swift          # Server capabilities response
│   │   ├── ThreadMetadata.swift            # Thread model (threading.schema.json)
│   │   └── Participant.swift               # Participant model
│   ├── Networking/
│   │   ├── WebSocketConnection.swift       # URLSessionWebSocketTask wrapper (actor)
│   │   ├── AINMClient.swift                # Protocol client: handshake, send, receive (actor)
│   │   └── ReconnectionManager.swift       # Exponential backoff reconnection logic
│   ├── Crypto/
│   │   ├── KeyManager.swift                # Ed25519 keypair generation + Keychain storage (actor)
│   │   ├── MessageSigner.swift             # Sign/verify envelopes per security.md
│   │   └── JCSCanonicalizer.swift          # RFC 8785 JSON Canonicalization Scheme
│   ├── Persistence/
│   │   ├── AppDatabase.swift               # GRDB DatabasePool setup + migrations
│   │   ├── MessageRecord.swift             # GRDB FetchableRecord/PersistableRecord
│   │   ├── ThreadRecord.swift              # GRDB record for threads
│   │   └── ParticipantRecord.swift         # GRDB record for participants
│   ├── UI/
│   │   ├── OnboardingView.swift            # Server URL + display name + connect
│   │   ├── OnboardingViewModel.swift       # @Observable, drives onboarding state
│   │   ├── ConnectionSuccessView.swift     # Debug/validation screen
│   │   └── ConnectionSuccessViewModel.swift
│   └── Package.swift                       # SPM dependencies
└── AINMMessengerTests/
    ├── CryptoTests/
    │   ├── JCSCanonicalizerTests.swift      # Test against shared vectors
    │   ├── MessageSignerTests.swift         # Sign/verify round-trip
    │   └── KeyManagerTests.swift            # Keypair generation + persistence
    ├── NetworkingTests/
    │   └── JsonRpcTests.swift              # JSON-RPC encode/decode
    └── PersistenceTests/
        └── AppDatabaseTests.swift          # Migration + CRUD
```

### Pattern 1: Actor-based Networking
**What:** WebSocket connection and protocol client as Swift actors for thread safety under Swift 6 strict concurrency.
**When to use:** All networking code that manages mutable WebSocket state.
**Example:**
```swift
// Source: Swift Concurrency + URLSessionWebSocketTask pattern
actor WebSocketConnection {
    private var task: URLSessionWebSocketTask?
    private let session: URLSession

    func connect(to url: URL) async throws {
        task = session.webSocketTask(with: url)
        task?.resume()
    }

    func send(_ message: URLSessionWebSocketTask.Message) async throws {
        guard let task else { throw ConnectionError.notConnected }
        try await task.send(message)
    }

    func receiveMessages() -> AsyncStream<URLSessionWebSocketTask.Message> {
        AsyncStream { continuation in
            Task {
                while let task = self.task {
                    do {
                        let message = try await task.receive()
                        continuation.yield(message)
                    } catch {
                        continuation.finish()
                        break
                    }
                }
            }
        }
    }

    func disconnect() {
        task?.cancel(with: .normalClosure, reason: nil)
        task = nil
    }
}
```

### Pattern 2: MVVM with @Observable
**What:** ViewModels using Swift 5.9+ @Observable macro, injected via SwiftUI environment.
**When to use:** All view-model pairs in Phase 10 (OnboardingViewModel, ConnectionSuccessViewModel).
**Example:**
```swift
// Source: Apple WWDC 2024 @Observable pattern
@Observable
final class OnboardingViewModel {
    var serverURL: String = "wss://localhost:8080"
    var displayName: String = ""
    var connectionState: ConnectionState = .idle
    var errorMessage: String?

    private let client: AINMClient
    private let keyManager: KeyManager

    enum ConnectionState {
        case idle, connecting, connected, error
    }

    func connect() async {
        connectionState = .connecting
        errorMessage = nil
        do {
            // Generate or load keypair (NET-04)
            let keypair = try await keyManager.getOrCreateKeypair(displayName: displayName)
            // Connect WebSocket (NET-01)
            try await client.connect(to: serverURL)
            // Handshake (NET-02)
            let capabilities = try await client.initialize(
                identity: keypair.identity,
                capabilities: .phase10Defaults
            )
            connectionState = .connected
        } catch {
            connectionState = .error
            errorMessage = error.localizedDescription
        }
    }
}
```

### Pattern 3: Write-Through Cache with GRDB
**What:** All server data flows through GRDB before reaching UI. GRDB is the single source of truth.
**When to use:** Every message received from server is persisted first, then UI observes GRDB.
**Example:**
```swift
// Source: GRDB documentation pattern
struct AppDatabase {
    let dbPool: DatabasePool

    var migrator: DatabaseMigrator {
        var migrator = DatabaseMigrator()

        migrator.registerMigration("v1_create_threads") { db in
            try db.create(table: "thread") { t in
                t.primaryKey("id", .text).notNull()         // UUIDv7
                t.column("title", .text)
                t.column("createdAt", .datetime).notNull()
                t.column("createdBy", .text).notNull()      // AINM address
                t.column("parentThreadId", .text)
                    .references("thread", onDelete: .cascade)
                t.column("parentMessageId", .text)
            }
        }

        migrator.registerMigration("v1_create_messages") { db in
            try db.create(table: "message") { t in
                t.primaryKey("id", .text).notNull()         // UUIDv7
                t.column("threadId", .text).notNull()
                    .references("thread", onDelete: .cascade)
                t.column("fromAddress", .text).notNull()    // AINM address
                t.column("timestamp", .datetime).notNull()
                t.column("type", .text).notNull()
                t.column("contentJson", .text).notNull()    // JSON array of content blocks
                t.column("signature", .text).notNull()
                t.column("serverTimestamp", .datetime)
            }
        }

        migrator.registerMigration("v1_create_participants") { db in
            try db.create(table: "participant") { t in
                t.primaryKey {
                    t.column("threadId", .text).notNull()
                        .references("thread", onDelete: .cascade)
                    t.column("identity", .text).notNull()   // AINM address
                }
                t.column("type", .text).notNull()           // "human" or "agent"
                t.column("joinedAt", .datetime).notNull()
                t.column("role", .text).notNull()           // "creator" or "member"
            }
        }

        return migrator
    }
}
```

### Anti-Patterns to Avoid
- **Skipping GRDB for UI state:** Never hold messages only in memory. Always persist first, then observe. This is the "write-through cache" pattern mandated by STATE.md.
- **Blocking the main actor with crypto:** Ed25519 signing is fast but JCS canonicalization on large messages could block. Run signing on a background actor.
- **Using Combine for WebSocket streams:** Swift Concurrency (AsyncStream) replaces Combine for new SwiftUI projects. Do not mix paradigms.
- **Non-deterministic JSON serialization:** Swift's JSONSerialization does NOT guarantee key ordering. The JCS implementation must sort keys explicitly.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| SQLite database access | Raw SQLite3 C API | GRDB.swift | Migrations, observation, type-safe queries, Swift 6 concurrency support |
| Ed25519 signing | Custom crypto | Apple CryptoKit Curve25519.Signing | Hardware-accelerated, audited, zero dependencies |
| Keychain access | Raw Security framework calls | KeychainAccess (or thin wrapper) | Keychain API is notoriously error-prone; wrapper prevents common mistakes |
| JSON-RPC 2.0 framing | Ad-hoc JSON construction | Codable structs matching protocol.rs types | Type safety, compile-time checks, matches Rust server exactly |
| UUIDv7 generation | Custom timestamp-based UUID | Foundation UUID (or small UUIDv7 helper) | UUIDv7 requires correct timestamp embedding |
| Base64url encoding | Manual string manipulation | Data extension with RFC 4648 Section 5 | Off-by-one errors in padding removal are common |

**Key insight:** The only thing that MUST be hand-rolled is RFC 8785 JCS canonicalization. There is no Swift library. The reference implementations exist in Rust (serde_json_canonicalizer), JavaScript, Java, Go, Python -- but not Swift.

## Common Pitfalls

### Pitfall 1: JCS UTF-16 Code Unit Ordering
**What goes wrong:** RFC 8785 requires sorting object keys by comparing their UTF-16 code unit sequences, NOT by UTF-8 byte order or Swift String default comparison. For ASCII keys (which all AINM protocol fields use), UTF-8 and UTF-16 orderings are identical. But if any key contains characters outside the BMP (emoji, some CJK), the orderings diverge.
**Why it happens:** Swift's `String.compare` uses Unicode scalar comparison by default, which differs from UTF-16 code unit comparison for surrogate pairs.
**How to avoid:** For Phase 10, all AINM protocol keys are ASCII, so standard string sorting works. Add a comment noting the UTF-16 requirement for future-proofing, and include a test vector with non-ASCII keys.
**Warning signs:** Signature verification fails on messages with non-ASCII field names.

### Pitfall 2: ECMAScript Number Serialization in JCS
**What goes wrong:** RFC 8785 requires numbers to be serialized using ECMAScript `Number.toString()` rules. This means: `1.0` becomes `"1"`, `-0` becomes `"0"`, and very large/small numbers use exponential notation. Swift's JSONSerialization may not match these rules exactly.
**Why it happens:** Swift serializes `1.0` as `1` (correct), but edge cases around `-0`, `Infinity`, and `NaN` differ.
**How to avoid:** In AINM protocol, version strings like `"1.0"` are JSON strings (not numbers), so the main risk is minimal for Phase 10. Add test vectors that exercise number edge cases. The protocol's `version` field is a string, not a number -- verify the canonical form preserves `"1.0"` as a string.
**Warning signs:** Canonical form differs between Rust and Swift for messages containing numeric values.

### Pitfall 3: KeychainAccess Swift 6 Compatibility
**What goes wrong:** KeychainAccess 4.2.2 was last released in March 2020 -- before Swift 6 existed. Its types may not conform to `Sendable`, causing compiler errors under strict concurrency.
**Why it happens:** The library predates Swift 6 strict concurrency requirements.
**How to avoid:** Test KeychainAccess import immediately with Swift 6 strict concurrency enabled. If it produces warnings/errors, fall back to one of: (1) a thin wrapper around the raw Security framework using `@unchecked Sendable`, (2) SwiftKeychainKit which is designed for Swift 6.2, or (3) keychain-swift which has a Sendable conformance PR. The keychain operations needed are minimal: store bytes, retrieve bytes, delete -- this is achievable with a ~50-line Security framework wrapper.
**Warning signs:** Build errors mentioning `Sendable` or `@Sendable` when importing KeychainAccess.

### Pitfall 4: Base64url vs Base64 Encoding
**What goes wrong:** The protocol uses base64url encoding WITHOUT padding (RFC 4648 Section 5), not standard base64. Swift's `Data.base64EncodedString()` produces standard base64 with padding.
**Why it happens:** Developers forget to replace `+` with `-`, `/` with `_`, and strip `=` padding.
**How to avoid:** Create a `Data` extension early that handles base64url encoding/decoding correctly. Test against the spec's test vectors which provide expected base64url outputs.
**Warning signs:** Signatures are 88 characters instead of 86, or contain `+`, `/`, or `=`.

### Pitfall 5: WebSocket Receive Loop Lifecycle
**What goes wrong:** URLSessionWebSocketTask's `receive()` method must be called repeatedly -- it only returns one message per call. Forgetting to loop causes the app to stop receiving after the first message.
**Why it happens:** Unlike callback-based WebSocket libraries, URLSessionWebSocketTask uses a pull model.
**How to avoid:** Wrap receive in an AsyncStream that continuously calls `receive()` in a loop. Handle cancellation via task cancellation (the Task.isCancelled check or catching CancellationError).
**Warning signs:** App receives the handshake response but no subsequent messages.

### Pitfall 6: GRDB Database Location
**What goes wrong:** The database file path must be in the app's Application Support directory (not Documents or tmp). Using the wrong directory causes data loss on tmp cleanup or iCloud backup issues.
**Why it happens:** Developers use `FileManager.default.temporaryDirectory` during development and forget to change it.
**How to avoid:** Use `FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)` from day one. Create the directory if it doesn't exist.
**Warning signs:** Database disappears after app restart, or GRDB throws "unable to open database file".

### Pitfall 7: Signature Field Removal Before Canonicalization
**What goes wrong:** The signing procedure requires removing the `signature` field BEFORE canonicalization. If the field is included (even as null), the canonical form will differ and signature verification will fail.
**Why it happens:** It's easy to build the full message including a placeholder signature, then try to canonicalize.
**How to avoid:** Follow security.md Section 4 exactly: build message WITHOUT signature field, canonicalize, sign, THEN add signature. The Rust implementation in signing.rs demonstrates this pattern (lines 28-48).
**Warning signs:** All signatures fail verification despite correct key material.

## Code Examples

### Ed25519 Key Generation and Keychain Storage
```swift
// Source: Apple CryptoKit documentation + KeychainAccess pattern
import CryptoKit

struct KeyPairInfo {
    let privateKey: Curve25519.Signing.PrivateKey
    let publicKey: Curve25519.Signing.PublicKey
    let publicKeyBase64url: String
}

actor KeyManager {
    private static let keychainKey = "com.ainm.ed25519.privateKey"

    func getOrCreateKeypair() throws -> KeyPairInfo {
        // Try to load existing key from Keychain
        if let existingKeyData = try loadFromKeychain() {
            let privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: existingKeyData)
            return makeInfo(privateKey)
        }

        // Generate new keypair
        let privateKey = Curve25519.Signing.PrivateKey()
        try saveToKeychain(privateKey.rawRepresentation)
        return makeInfo(privateKey)
    }

    private func makeInfo(_ privateKey: Curve25519.Signing.PrivateKey) -> KeyPairInfo {
        KeyPairInfo(
            privateKey: privateKey,
            publicKey: privateKey.publicKey,
            publicKeyBase64url: privateKey.publicKey.rawRepresentation.base64urlEncodedString()
        )
    }

    // Keychain operations (thin wrapper -- replace with KeychainAccess if Swift 6 compatible)
    private func saveToKeychain(_ data: Data) throws { /* Security framework calls */ }
    private func loadFromKeychain() -> Data? { /* Security framework calls */ }
}
```

### Base64url Encoding Extension
```swift
// Source: RFC 4648 Section 5
extension Data {
    func base64urlEncodedString() -> String {
        base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    init?(base64urlEncoded string: String) {
        var base64 = string
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        // Add padding
        let remainder = base64.count % 4
        if remainder > 0 {
            base64 += String(repeating: "=", count: 4 - remainder)
        }
        self.init(base64Encoded: base64)
    }
}
```

### JCS Canonicalization (Core Logic)
```swift
// Source: RFC 8785 specification + cyberphone/json-canonicalization reference
/// RFC 8785 JSON Canonicalization Scheme implementation.
/// Produces deterministic JSON: sorted keys, no whitespace, ECMAScript number serialization.
struct JCSCanonicalizer {

    /// Canonicalize a JSON value to its RFC 8785 canonical form.
    static func canonicalize(_ value: Any) throws -> String {
        var output = ""
        try serialize(value, to: &output)
        return output
    }

    private static func serialize(_ value: Any, to output: inout String) throws {
        switch value {
        case is NSNull:
            output += "null"
        case let bool as Bool:
            output += bool ? "true" : "false"
        case let number as NSNumber:
            // ECMAScript Number.toString() rules
            output += serializeNumber(number)
        case let string as String:
            output += serializeString(string)
        case let array as [Any]:
            output += "["
            for (i, element) in array.enumerated() {
                if i > 0 { output += "," }
                try serialize(element, to: &output)
            }
            output += "]"
        case let dict as [String: Any]:
            output += "{"
            // Sort keys by UTF-16 code unit comparison (RFC 8785 Section 3.2.3)
            let sortedKeys = dict.keys.sorted { a, b in
                // For ASCII keys this matches standard string comparison
                a.utf16.lexicographicallyPrecedes(b.utf16)
            }
            for (i, key) in sortedKeys.enumerated() {
                if i > 0 { output += "," }
                output += serializeString(key)
                output += ":"
                try serialize(dict[key]!, to: &output)
            }
            output += "}"
        default:
            throw JCSError.unsupportedType
        }
    }

    private static func serializeString(_ s: String) -> String {
        var result = "\""
        for scalar in s.unicodeScalars {
            switch scalar.value {
            case 0x08: result += "\\b"
            case 0x09: result += "\\t"
            case 0x0A: result += "\\n"
            case 0x0C: result += "\\f"
            case 0x0D: result += "\\r"
            case 0x22: result += "\\\""
            case 0x5C: result += "\\\\"
            case 0x00...0x1F:
                result += String(format: "\\u%04x", scalar.value)
            default:
                result += String(scalar)
            }
        }
        result += "\""
        return result
    }
}
```

### Message Signing (Matching Rust signing.rs)
```swift
// Source: security.md Section 4 + demo/src/signing.rs reference
import CryptoKit

struct MessageSigner {

    /// Sign a message envelope per security.md Section 4.
    /// Returns the base64url-encoded Ed25519 signature.
    static func sign(
        envelope: [String: Any],
        privateKey: Curve25519.Signing.PrivateKey
    ) throws -> String {
        // Step 1: Build message without signature (caller should not include it)
        var signingInput = envelope
        signingInput.removeValue(forKey: "signature")

        // Step 2: Canonicalize per RFC 8785
        let canonical = try JCSCanonicalizer.canonicalize(signingInput)

        // Step 3: Encode as UTF-8 bytes
        guard let data = canonical.data(using: .utf8) else {
            throw SigningError.encodingFailed
        }

        // Step 4: Sign with Ed25519
        let signature = try privateKey.signature(for: data)

        // Step 5: Encode as base64url without padding
        return signature.base64urlEncodedString()
    }

    /// Verify a message envelope's signature per security.md Section 5.
    static func verify(
        envelope: [String: Any],
        publicKey: Curve25519.Signing.PublicKey
    ) throws -> Bool {
        // Step 1: Extract signature
        guard let sigString = envelope["signature"] as? String else {
            throw SigningError.missingSignature
        }

        // Step 2: Remove signature field
        var signingInput = envelope
        signingInput.removeValue(forKey: "signature")

        // Step 3-4: Canonicalize and encode
        let canonical = try JCSCanonicalizer.canonicalize(signingInput)
        guard let data = canonical.data(using: .utf8) else {
            throw SigningError.encodingFailed
        }

        // Step 5: Decode signature from base64url
        guard let sigData = Data(base64urlEncoded: sigString) else {
            throw SigningError.invalidSignature
        }

        // Step 6: Verify Ed25519 signature
        return publicKey.isValidSignature(sigData, for: data)
    }
}
```

### JSON-RPC 2.0 Types (Matching Rust protocol.rs)
```swift
// Source: JSON-RPC 2.0 specification + demo/src/protocol.rs
struct JsonRpcRequest: Codable {
    let jsonrpc: String  // Always "2.0"
    let id: AnyCodableValue
    let method: String
    let params: AnyCodableValue?

    init(method: String, params: some Encodable, id: Int) {
        self.jsonrpc = "2.0"
        self.id = .int(id)
        self.method = method
        self.params = try? AnyCodableValue(params)
    }
}

struct JsonRpcResponse: Codable {
    let jsonrpc: String
    let id: AnyCodableValue
    let result: AnyCodableValue?
    let error: JsonRpcError?
}

struct JsonRpcNotification: Codable {
    let jsonrpc: String
    let method: String
    let params: AnyCodableValue?

    static func initialized() -> JsonRpcNotification {
        JsonRpcNotification(jsonrpc: "2.0", method: "ainm.initialized", params: nil)
    }
}

struct JsonRpcError: Codable {
    let code: Int
    let message: String
    let data: AnyCodableValue?
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| ObservableObject + @Published | @Observable macro | iOS 17+ (WWDC 2023) | Less boilerplate, better performance, simpler ViewModels |
| Combine for async streams | AsyncStream + Swift Concurrency | Swift 5.5+ / iOS 15+ | No Combine dependency needed, cleaner actor model |
| Core Data | GRDB.swift | Community consensus 2024-2026 | Better Swift integration, compile-time SQL, no legacy ORM |
| SwiftData (Apple's new ORM) | GRDB.swift | 2024-2026 evaluation | SwiftData lacks background tasks, poor at scale -- GRDB remains superior for messaging workloads |
| KeychainAccess (widely used) | Uncertain -- library stale since 2020 | 2025-2026 | Swift 6 strict concurrency may require alternatives |

**Deprecated/outdated:**
- **Combine for WebSocket streams:** Replaced by AsyncStream in new Swift Concurrency projects
- **ObservableObject:** Replaced by @Observable for new iOS 17+ targets
- **SwiftData for messaging apps:** Explicitly rejected in CLAUDE.md due to missing background task support

## Open Questions

1. **KeychainAccess Swift 6 Compatibility**
   - What we know: Last release was March 2020 (v4.2.2). Swift 6 strict concurrency was introduced years later. No known Swift 6 compatibility PR.
   - What's unclear: Whether it compiles cleanly with `SWIFT_STRICT_CONCURRENCY=complete` in Xcode 16.
   - Recommendation: Try it first. If it fails, implement a thin Keychain wrapper (~50 lines using Security framework) or use SwiftKeychainKit. The required operations are trivial: save Data, load Data, delete.

2. **UUIDv7 Generation in Swift**
   - What we know: Foundation's `UUID()` generates v4 (random), not v7 (time-ordered). The protocol requires UUIDv7 per RFC 9562.
   - What's unclear: Whether a lightweight UUIDv7 Swift library exists or if we need a small custom implementation.
   - Recommendation: Implement a small UUIDv7 generator (~30 lines) or use a package. The format is well-specified: 48-bit Unix timestamp in milliseconds + 4-bit version (7) + 12-bit random + 2-bit variant + 62-bit random.

3. **Rust Demo Server WebSocket Path**
   - What we know: The Rust server listens on `/ws` (see server.rs line 72). The protocol spec says `/ainm/v1`. The demo server does NOT use the spec path.
   - What's unclear: Whether to connect to `/ws` (matching the running server) or `/ainm/v1` (matching the spec).
   - Recommendation: Connect to `/ws` for Phase 10 integration testing (it's what actually works). The spec path can be used in production; the demo is a development convenience.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Xcode | iOS build + test | Yes | 26.3 (Build 17C529) | -- |
| Swift | All Swift code | Yes | 6.2.4 | -- |
| Cargo (Rust) | Demo server build | Yes | 1.94.0 | -- |
| GitHub CLI (gh) | CI setup | Yes | 2.87.3 | -- |
| Simulator (iOS 18+) | UI testing | Yes (via Xcode) | -- | -- |

**Missing dependencies with no fallback:** None -- all required tools are available.

**Missing dependencies with fallback:** None.

## Project Constraints (from CLAUDE.md)

- **Protocol-first:** Spec must be complete before building iOS app (specs exist: transport.md, security.md, identity.md)
- **Transport:** WebSocket primary, HTTP/SSE fallback, HTTP for federation
- **Security:** Cryptographic identity and message signing non-negotiable from day one
- **iOS Client Stack:** SwiftUI iOS 18+, Swift 6.1+, GRDB.swift 7.10.x, Apple CryptoKit, KeychainAccess 4.x, URLSessionWebSocketTask
- **Architecture:** MVVM + Repository, @Observable, NavigationStack + NavigationPath, Environment + Protocol-based DI
- **Rejected technologies:** SwiftData, Core Data, Alamofire, Realm, RxSwift/Combine, gRPC-Swift 2

## Sources

### Primary (HIGH confidence)
- `spec/security.md` -- Full signing/verification procedure with test vectors and worked examples
- `spec/transport.md` -- WebSocket lifecycle, ainm.initialize/initialized handshake, JSON-RPC 2.0 framing
- `spec/identity.md` -- Address format, identity resolution, key rotation
- `schemas/transport.schema.json` -- JSON Schema for all transport types
- `schemas/threading.schema.json` -- Thread and participant schemas
- `demo/src/signing.rs` -- Rust signing implementation (integration target)
- `demo/src/server.rs` -- Rust server handshake handling
- `demo/src/protocol.rs` -- Rust JSON-RPC 2.0 type definitions
- [Apple CryptoKit Curve25519.Signing](https://developer.apple.com/documentation/cryptokit/curve25519/signing)
- [GRDB.swift GitHub](https://github.com/groue/GRDB.swift) - v7.10.0, Swift 6 compatible
- [GRDB 7 Migration Guide](https://github.com/groue/GRDB.swift/blob/master/Documentation/GRDB7MigrationGuide.md)

### Secondary (MEDIUM confidence)
- [cyberphone/json-canonicalization](https://github.com/cyberphone/json-canonicalization) -- Reference implementations in 9 languages (no Swift), confirms no Swift JCS library exists
- [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785) -- JSON Canonicalization Scheme specification
- [KeychainAccess Swift Package Index](https://swiftpackageindex.com/kishikawakatsumi/KeychainAccess) -- v4.2.2, last release 2020-03-01
- [SwiftKeychainKit Swift Forums](https://forums.swift.org/t/swiftkeychainkit-an-ios-macos-keychain-library-designed-around-strict-concurrency-and-non-copyable-types/85074) -- Swift 6.2 native keychain alternative
- [URLSessionWebSocketTask docs](https://developer.apple.com/documentation/foundation/urlsessionwebsockettask) -- Apple's native WebSocket API
- [Iterating over WebSocket messages with async/await](https://www.donnywals.com/iterating-over-web-socket-messages-with-async-await-in-swift/) -- AsyncStream wrapping pattern

### Tertiary (LOW confidence)
- KeychainAccess Swift 6 compatibility status -- not verified, flagged as open question

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries locked in CLAUDE.md and verified available
- Architecture: HIGH -- patterns well-established (MVVM + @Observable, GRDB migrations, actor-based networking)
- Pitfalls: HIGH -- JCS is the known risk, documented with mitigation via shared test vectors
- Crypto: HIGH -- Apple CryptoKit Ed25519 is well-documented and matches protocol requirements exactly
- KeychainAccess compatibility: LOW -- last release 2020, Swift 6 untested

**Research date:** 2026-03-29
**Valid until:** 2026-04-28 (stable domain -- iOS/Swift ecosystem moves slowly)
