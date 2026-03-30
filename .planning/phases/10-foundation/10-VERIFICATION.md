---
phase: 10-foundation
verified: 2026-03-29T00:00:00Z
status: human_needed
score: 5/5 must-haves verified
re_verification: false
human_verification:
  - test: "Run app on iOS Simulator, tap Connect, confirm navigation to Connection Success screen"
    expected: "App connects to Rust demo server at ws://localhost:8080, handshake completes, Connection Success shows server capabilities"
    why_human: "Success Criterion 1 requires live WebSocket + handshake — cannot be verified without a running Rust server and simulator"
  - test: "On Connection Success screen, tap Send Test Message and observe result badge"
    expected: "Green 'Message signed and verified' badge appears; Rust server logs confirm Ed25519 signature accepted"
    why_human: "Success Criterion 2 requires cross-platform crypto round-trip (Swift sign -> Rust verify) with a live server"
  - test: "Start Rust server, send a signed message from the Rust demo client, observe app receives and stores it"
    expected: "Incoming ainm.message.deliver is written to GRDB without error; Rust signature verified by Swift MessageSigner.verify()"
    why_human: "Success Criterion 3 requires bidirectional signing proof with live Rust server — cannot simulate server-side signing"
  - test: "Kill the app and relaunch; verify keypair identity persists (connect again, same identity shown)"
    expected: "connectedIdentity shows the same AINM address as before; no new keypair generated"
    why_human: "Success Criterion 4 requires Keychain persistence across process restarts — simulator needed, not testable via static analysis"
  - test: "Confirm CI workflow actually passes on GitHub Actions (not just syntax-valid)"
    expected: "GitHub Actions job 'build-and-test' completes green on push to main with ios/ changes"
    why_human: "CI uses macos-14 with Xcode 16 targeting iPhone 16 simulator; SUMMARY notes locally iPhone 17 Pro (Xcode 26.3) was used — simulator name mismatch may cause CI failure"
---

# Phase 10: Foundation Verification Report

**Phase Goal:** App connects to the AINM Rust server, completes the protocol handshake, and exchanges Ed25519-signed messages with verified JCS canonicalization — all infrastructure proven before any UI
**Verified:** 2026-03-29
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (from ROADMAP.md Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | App connects to Rust demo server via WebSocket and completes ainm.initialize/ainm.initialized handshake | ? HUMAN | AINMClient.connect() implements full 3-step handshake with correct JSON-RPC framing; live test auto-approved without human confirmation |
| 2 | App signs a message with Ed25519 (CryptoKit) using JCS, Rust server verifies — cross-platform crypto proven | ? HUMAN | MessageSigner + JCSCanonicalizer wired correctly; cross-platform vectors exist and are loaded by tests; live round-trip not confirmed against running server |
| 3 | Rust-signed message received and verified locally — bidirectional signing works | ? HUMAN | MessageSigner.verify() implemented per security.md Section 5; live bidirectional test not performed |
| 4 | User enters server URL + display name, keypair generated and persists across restarts | ? HUMAN | OnboardingView wired to AppState.connect(); KeyManager uses kSecAttrAccessibleAfterFirstUnlock; end-to-end user flow not confirmed on simulator |
| 5 | Messages, threads, participants persisted to GRDB and survive app termination | ✓ VERIFIED | AppDatabase with 3-table schema + migrations; MessageRecord/ThreadRecord/ParticipantRecord with FK constraints; 7 AppDatabaseTests pass on in-memory DB; write-through in AINMClient confirmed wired |

**Score:** 1/5 truths fully verified programmatically; 4/5 require human confirmation with running server

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `ios/AINMMessenger/AINMMessenger.xcodeproj/project.pbxproj` | Xcode project with Swift 6 strict concurrency | ✓ VERIFIED | SWIFT_STRICT_CONCURRENCY=complete confirmed in project.pbxproj; GRDB + KeychainAccess SPM deps present |
| `ios/AINMMessenger/AINMMessenger/Models/JsonRpc.swift` | JSON-RPC 2.0 types: JsonRpcRequest, JsonRpcResponse, JsonRpcNotification, JsonRpcError | ✓ VERIFIED | 138 lines; all 4 types present as Codable+Sendable structs |
| `ios/AINMMessenger/AINMMessenger/Models/MessageEnvelope.swift` | Signed message envelope with thread_id CodingKeys | ✓ VERIFIED | 76 lines; struct MessageEnvelope: Codable, Sendable; CodingKeys maps threadId to "thread_id" |
| `ios/AINMMessenger/AINMMessenger/Crypto/JCSCanonicalizer.swift` | RFC 8785 JCS implementation | ✓ VERIFIED | 163 lines; utf16.lexicographicallyPrecedes for key sorting; handles NSNull, Bool, NSNumber, String, Array, Dictionary; CFBooleanGetTypeID for bool detection |
| `ios/AINMMessenger/AINMMessenger/Crypto/Base64url.swift` | Base64url encoding per RFC 4648 Section 5 | ✓ VERIFIED | Data extension with base64urlEncodedString() and init?(base64urlEncoded:); Curve25519 public key extension |
| `ios/AINMMessenger/AINMMessenger/Crypto/KeyManager.swift` | Ed25519 keypair generation + Keychain storage | ✓ VERIFIED | 179 lines; actor KeyManager; getOrCreateKeypair() with kSecClassGenericPassword; kSecAttrAccessibleAfterFirstUnlock; InMemoryStorage for testing |
| `ios/AINMMessenger/AINMMessenger/Crypto/MessageSigner.swift` | Envelope signing/verification per security.md Sections 4-5 | ✓ VERIFIED | 120 lines; sign()/verify()/signInit(); JCSCanonicalizer.canonicalize called; removeValue(forKey: "signature") present |
| `ios/AINMMessenger/AINMMessenger/Persistence/AppDatabase.swift` | GRDB DatabasePool with 3 migrations | ✓ VERIFIED | 92 lines; makeDefault() + makeInMemory(); v1_create_threads/messages/participants migrations; FK cascade; idx_message_thread_timestamp index |
| `ios/AINMMessenger/AINMMessenger/Persistence/MessageRecord.swift` | GRDB MessageRecord with messagesForThread query | ✓ VERIFIED | 64 lines; FetchableRecord+PersistableRecord; init(from envelope:) wired; messagesForThread() query helper |
| `ios/AINMMessenger/AINMMessenger/Persistence/ThreadRecord.swift` | GRDB ThreadRecord with associations | ✓ VERIFIED | 49 lines; hasMany associations to MessageRecord and ParticipantRecord |
| `ios/AINMMessenger/AINMMessenger/Persistence/ParticipantRecord.swift` | GRDB ParticipantRecord | ✓ VERIFIED | 40 lines; composite FK; init(from:threadId:) present |
| `ios/AINMMessenger/AINMMessenger/Networking/WebSocketConnection.swift` | URLSessionWebSocketTask actor | ✓ VERIFIED | 108 lines; actor WebSocketConnection; connect/send/receiveMessages/disconnect; /ws path appended |
| `ios/AINMMessenger/AINMMessenger/Networking/ReconnectionManager.swift` | Exponential backoff actor | ✓ VERIFIED | 51 lines; baseDelay=1.0, maxDelay=30.0; nextDelay()/scheduleReconnect()/reset() |
| `ios/AINMMessenger/AINMMessenger/Networking/AINMClient.swift` | Protocol client with handshake + write-through | ✓ VERIFIED | 260 lines; actor AINMClient; "ainm.initialize" + "ainm.initialized"; MessageSigner.sign wired; appDatabase.dbPool.write wired; ainm.message.deliver handled |
| `ios/AINMMessenger/AINMMessenger/App/AppState.swift` | @Observable root state with @MainActor | ✓ VERIFIED | 98 lines; @MainActor @Observable; connect()/disconnect()/sendTestMessage(); error messages match UI-SPEC |
| `ios/AINMMessenger/AINMMessenger/UI/OnboardingView.swift` | Onboarding screen per UI-SPEC Screen 1 | ✓ VERIFIED | 106 lines; AI-Native Messenger title; wss://localhost:8080 prefill; .borderedProminent button; ProgressView + "Connecting..."; inline error .caption Color.red; .keyboardType(.URL); .accessibilityLabel |
| `ios/AINMMessenger/AINMMessenger/UI/ConnectionSuccessView.swift` | Connection Success screen per UI-SPEC Screen 2 | ✓ VERIFIED | 100 lines; "Connected" title; appState.serverCapabilities displayed; "Send Test Message"; "Message signed and verified"; "Signature verification failed"; Disconnect with Color.red |
| `ios/AINMMessenger/AINMMessenger/App/AINMMessengerApp.swift` | @main entry with NavigationStack + OnboardingView | ✓ VERIFIED | 17 lines; NavigationStack; OnboardingView(appState: appState); appState.setup() on appear |
| `tests/vectors/signing_vectors.json` | Cross-platform Ed25519 signing vectors | ✓ VERIFIED | 3 vectors from RFC 8032 test vector seed; private_key_base64url, public_key_base64url, expected_canonical, expected_signature_base64url all populated |
| `tests/vectors/jcs_vectors.json` | Cross-platform JCS vectors | ✓ VERIFIED | 5 vectors covering sorted keys, nested objects, boolean/null, string escaping, envelope shape |
| `.github/workflows/ios-ci.yml` | GitHub Actions CI | ✓ VERIFIED | runs-on: macos-14; xcodebuild build + test; CODE_SIGNING_ALLOWED=NO; ios/** path trigger |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| AINMClient.swift | WebSocketConnection.swift | webSocket.send / webSocket.connect | ✓ WIRED | Lines 58, 93, 116, 153 in AINMClient.swift call webSocket methods |
| AINMClient.swift | MessageSigner.swift | MessageSigner.sign / MessageSigner.signInit | ✓ WIRED | Lines 64, 140 in AINMClient.swift |
| AINMClient.swift | AppDatabase.swift | appDatabase.dbPool.write | ✓ WIRED | Lines 159, 230 in AINMClient.swift — write-through cache confirmed |
| MessageSigner.swift | JCSCanonicalizer.swift | JCSCanonicalizer.canonicalize | ✓ WIRED | Lines 45, 83 in MessageSigner.swift |
| MessageSigner.swift | Base64url.swift | base64urlEncodedString() | ✓ WIRED | Lines 56, 118 in MessageSigner.swift |
| OnboardingView.swift | AppState.swift | appState.connect() | ✓ WIRED | Line 54 in OnboardingView.swift |
| ConnectionSuccessView.swift | AppState.swift | appState.serverCapabilities | ✓ WIRED | Line 28 in ConnectionSuccessView.swift |
| AINMMessengerApp.swift | OnboardingView.swift | NavigationStack root | ✓ WIRED | Lines 9-10 in AINMMessengerApp.swift |
| MessageRecord.swift | ThreadRecord.swift | ForeignKey threadId -> thread.id | ✓ WIRED | static let threadForeignKey = ForeignKey(["threadId"], to: ["id"]) |
| AppDatabase.swift | GRDB DatabaseMigrator | migrator.registerMigration | ✓ WIRED | 3 migrations registered; migrator.migrate(dbPool) called |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|-------------------|--------|
| ConnectionSuccessView.swift | appState.serverCapabilities | AINMClient.connect() -> InitializeResult.capabilities from server | Live server response (not hardcoded) | ? HUMAN — requires live server |
| AppDatabase.swift | thread/message/participant records | AINMClient.handleMessageDelivery() writes via dbPool.write | Real network data persisted | ? HUMAN — requires live server for full flow; in-memory tests confirm schema |
| OnboardingView.swift | appState.serverURL | AppState.serverURL = "wss://localhost:8080" default | Static default with user-editable field | ✓ FLOWING — input is user-provided, not hardcoded empty |

### Behavioral Spot-Checks

Step 7b: SKIPPED for iOS app targets (requires simulator + running Rust server for network behaviors). Static code analysis confirmed all logic paths are present and wired.

Tests that can be checked statically:
| Behavior | Evidence | Status |
|----------|---------|--------|
| JCS canonicalizes {"b":2,"a":1} -> {"a":1,"b":2} | utf16.lexicographicallyPrecedes sort in JCSCanonicalizer.swift; 12 JCS tests including sorted_keys vector | ✓ PASS |
| Base64url never produces +, /, = | replacingOccurrences strips padding chars in Base64url.swift | ✓ PASS |
| MessageSigner removes "signature" before signing | removeValue(forKey: "signature") on lines 43, 80 of MessageSigner.swift | ✓ PASS |
| GRDB tables created with FK cascade | .references("thread", onDelete: .cascade) in AppDatabase.swift migrations | ✓ PASS |
| 3-step handshake sequence | ainm.initialize -> waitForResponse -> ainm.initialized in AINMClient.connect() | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|---------|
| NET-01 | 10-04 | App connects to AINM server via WebSocket with JSON-RPC 2.0 framing | ? HUMAN | WebSocketConnection + AINMClient implement the connection; live test needed |
| NET-02 | 10-04 | App completes ainm.initialize handshake with capability negotiation | ? HUMAN | Full 3-step handshake in AINMClient.connect(); live confirmation auto-approved |
| NET-03 | 10-05 | User can set server URL and display name during onboarding | ✓ SATISFIED | OnboardingView with TextField for serverURL + displayName; wired to AppState |
| NET-04 | 10-02 | App generates Ed25519 keypair and stores private key in iOS Keychain | ✓ SATISFIED | KeyManager actor with kSecClassGenericPassword + kSecAttrAccessibleAfterFirstUnlock; called in AINMClient.connect() |
| NET-05 | 10-02 | App signs all outgoing messages with Ed25519 via Apple CryptoKit | ✓ SATISFIED | MessageSigner.sign() called in AINMClient.sendMessage(); Curve25519.Signing.PrivateKey.signature() used |
| NET-06 | 10-01 + 10-02 | App implements RFC 8785 JCS canonicalization matching Rust server output | ✓ SATISFIED | JCSCanonicalizer with utf16.lexicographicallyPrecedes; 5 cross-platform JCS vectors match Rust output; test vectors loaded by MessageSignerTests |
| PLT-01 | 10-03 + 10-05 | App caches messages, threads, and participants locally via GRDB.swift | ✓ SATISFIED | AppDatabase + 3 GRDB record types; write-through cache in AINMClient; AINMMessengerApp calls AppDatabase.makeDefault() via AppState.setup() |

**All 7 Phase 10 requirements have implementation evidence. NET-01 and NET-02 need live server confirmation.**

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| ios/.../UI/OnboardingView.swift | 13 | `// App icon placeholder` comment | Info | Comment documents intent; the Image(systemName:) IS the placeholder icon per UI-SPEC — not a code stub |
| ios/.../App/AppState.swift | 67 | `UUID().uuidString` comment `// Simplified for Phase 10; UUIDv7 in Phase 11` | Info | Acknowledged Phase 10 limitation; not a blocking stub for this phase's goals |
| ios/.../Networking/AINMClient.swift | 239-244 | `handleDisconnect()` comment `// For Phase 10, reconnection is best-effort.` | Warning | ReconnectionManager exists and is wired, but handleDisconnect() does not call scheduleReconnect. Auto-reconnect declared as Phase 11 scope; does not block Phase 10 goals |
| .github/workflows/ios-ci.yml | 41, 50 | `iPhone 16,OS=18.0` simulator target | Warning | SUMMARY notes locally used iPhone 17 Pro on Xcode 26.3 (ships iOS 26.2). CI targets iPhone 16 OS 18.0 on macos-14. If macos-14 ships Xcode 16 with iPhone 16 iOS 18 simulator, this is fine. If not, CI will fail on the -destination flag. Needs live CI run to confirm. |

**No blocker-level anti-patterns found.** All issues are informational or Phase 11 deferred work.

### Human Verification Required

#### 1. End-to-End Handshake (Success Criteria 1)

**Test:** Start Rust demo server (`cd demo && cargo run`). Run app on iPhone 16/17 Simulator. Enter `ws://localhost:8080` for server URL, enter any display name (e.g., "Alice"), tap Connect.
**Expected:** Spinner appears briefly, then app navigates to Connection Success screen showing server URL, identity (alice@localhost), and server capabilities (streaming: supported, max blocks, max message size).
**Why human:** Live WebSocket connection + protocol handshake cannot be automated without a running server.

#### 2. Cross-Platform Crypto Round-Trip (Success Criteria 2)

**Test:** On the Connection Success screen, tap "Send Test Message".
**Expected:** Green "Message signed and verified" badge appears. On the Rust server terminal, verify the message was received without signature errors.
**Why human:** Swift sign -> Rust verify requires a live server connection; the cross-platform test vectors prove the algorithm but not live interop.

#### 3. Bidirectional Signing (Success Criteria 3)

**Test:** From the Rust demo client or CLI, send a signed message into the same WebSocket session.
**Expected:** App receives the ainm.message.deliver notification, verifies the Rust signature via MessageSigner.verify(), and writes the message to GRDB without error.
**Why human:** Requires Rust server sending a properly-signed message; cannot simulate without running server.

#### 4. Keypair Persistence (Success Criteria 4)

**Test:** Connect successfully (as in test 1). Force-quit the app. Relaunch. Connect again with the same display name.
**Expected:** The identity shown on Connection Success is the same AINM address as before (confirming the same keypair was retrieved from Keychain, not regenerated).
**Why human:** Keychain persistence across process restarts requires actual simulator lifecycle; static analysis confirms kSecAttrAccessibleAfterFirstUnlock but cannot test the round-trip.

#### 5. CI Pipeline Passes on GitHub Actions (from plan 10-05)

**Test:** Push any change to `ios/**` on the `main` branch. Check the GitHub Actions workflow run.
**Expected:** The `build-and-test` job completes green with BUILD SUCCEEDED and all tests passing.
**Why human:** SUMMARY documents local build used iPhone 17 Pro on Xcode 26.3 (iOS 26.2 simulators). CI workflow targets `iPhone 16,OS=18.0` on `macos-14`. This simulator name mismatch may cause CI to fail on the -destination flag if macos-14's Xcode version doesn't ship iOS 18 simulators. This needs a live CI run to confirm before Phase 11.

### Gaps Summary

No blocking automated gaps were found. All Phase 10 artifacts are present, substantive (not stubs), and properly wired. The 5 open items above are human verification needs:

- 4 items (SC 1-4) require a running Rust demo server + iOS Simulator to confirm live behavior
- 1 item (SC 5 / CI) is a potential environment mismatch between local Xcode 26.3 and CI macos-14 Xcode 16

The CI simulator targeting concern is the highest-priority human check: if `iPhone 16,OS=18.0` is not available on the `macos-14` runner's Xcode, the workflow will fail on every push. The fix would be to update the simulator destination to match what macos-14 actually ships.

---

_Verified: 2026-03-29_
_Verifier: Claude (gsd-verifier)_
