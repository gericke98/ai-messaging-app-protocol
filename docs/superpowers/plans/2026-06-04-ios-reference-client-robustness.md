# iOS Reference-Client Robustness — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the iOS reference client survive real use — remember the signed-in account across launches, auto-reconnect on socket drop, recover gracefully from onboarding failures, expose the assistant entry point, and lock today's wire-format fixes behind regression tests.

**Architecture:** Persisted identity (`serverURL`+`username` in `UserDefaults`/`@AppStorage`) + the device keypair in the Keychain are the source of truth for "onboarded". On launch, `AppState.setup()` routes to a resume path (Face-ID-if-enrolled → `connect()`) or onboarding. `AINMClient`'s existing `ReconnectionManager` drives backoff re-handshake on drops. Onboarding splits verify from bind/connect; the server's `bind_key_handler` becomes idempotent for the same (account, key).

**Tech Stack:** SwiftUI (`@Observable` AppState, iOS 18+), CryptoKit/Keychain (`KeyManager`), `URLSessionWebSocketTask` (`AINMClient`), Rust/Axum server (`bind_key_handler`), XCTest + the Rust `ws_conversation_e2e` integration test.

**Spec:** `docs/superpowers/specs/2026-06-04-ios-reference-client-robustness-design.md`

**Build env reminders:** iOS builds via `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -configuration Debug -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build` (NO `CODE_SIGNING_ALLOWED=NO` — that breaks Keychain; the default "Sign to Run Locally" is required). Server tests need `AINM_DATABASE__URL="postgres://santiagogerickeparga@localhost:5432/ainm"`. SourceKit "No such module GRDB/XCTest" is a false positive — `xcodebuild` is authoritative. Use `rtk proxy git` for git.

---

## File Structure

- `ios/.../App/AppState.swift` — persisted-session props, `bootstrap()` launch routing, persist on connect, `signOut()`, reconnect state surfacing.
- `ios/.../App/AINMMessengerApp.swift` — root view routes on a new `sessionPhase` (onboarding / locked / active), not just `connectionState`.
- `ios/.../Crypto/KeyManager.swift` — `hasStoredKeypair()` (non-creating existence check) + expose `deleteKeypair()`.
- `ios/.../UI/Onboarding/SessionGateView.swift` (new) — Face-ID-if-enrolled unlock + Locked screen (Try again / Sign out).
- `ios/.../Networking/AINMClient.swift` — receive-loop close/error → reconnection driver; expose a user-initiated `disconnect()` that cancels reconnect.
- `ios/.../UI/Onboarding/OnboardingFlowView.swift` — verify at the `.code` step; `working` does only bind+escrow+connect.
- `ios/.../UI/ConversationList/ConversationListView.swift` — fix assistant-button SF Symbol; reconnecting banner; Sign out control.
- `server/src/server.rs` — `bind_key_handler` idempotent for same (account, key).
- `server/src/accounts.rs` — helper to check if a public key is already bound to an account (if needed).
- `server/tests/ws_conversation_e2e.rs` — assert canonical delivered envelope.
- `ios/.../AINMMessengerTests/NetworkingTests/EnvelopeDecodeTests.swift` (new) — the three decode regression tests.

---

## Phase 1 — Persisted session + launch routing

### Task 1.1: `KeyManager.hasStoredKeypair()` + delete (TDD)

**Files:** Modify `ios/.../Crypto/KeyManager.swift`; Test `ios/.../AINMMessengerTests/CryptoTests/KeyManagerTests.swift`

- [ ] **Step 1 — failing test.** In `KeyManagerTests.swift`, add (using the existing in-memory `KeyStorage` test pattern in that file):
```swift
func test_hasStoredKeypair_reflectsPresence() async throws {
    let mgr = KeyManager(storage: InMemoryStorage())   // match existing test ctor
    XCTAssertFalse(mgr.hasStoredKeypair())
    _ = try await mgr.getOrCreateKeypair()
    XCTAssertTrue(mgr.hasStoredKeypair())
    try mgr.deleteKeypair()
    XCTAssertFalse(mgr.hasStoredKeypair())
}
```
(Check the exact `KeyManager` test initializer name in the file; mirror it.)

- [ ] **Step 2 — run, expect FAIL** (`hasStoredKeypair`/`deleteKeypair` undefined): `xcodebuild ... -only-testing:AINMMessengerTests/KeyManagerTests/test_hasStoredKeypair_reflectsPresence test`

- [ ] **Step 3 — implement.** Add to `KeyManager`:
```swift
/// Non-mutating: true iff a keypair is already stored. Does NOT create one
/// (unlike getOrCreateKeypair). Used by launch routing to detect an onboarded device.
func hasStoredKeypair() -> Bool {
    (try? storage.load()) ?? nil != nil
}
/// Remove the stored keypair (used by Sign out when the user opts to forget the device key).
func deleteKeypair() throws { try storage.delete() }
```

- [ ] **Step 4 — run, expect PASS.**
- [ ] **Step 5 — commit:** `rtk proxy git add ios/.../KeyManager.swift ios/.../KeyManagerTests.swift && rtk proxy git commit -m "feat(ios): KeyManager.hasStoredKeypair + deleteKeypair"`

### Task 1.2: Persisted identity + `signOut()` in AppState

**Files:** Modify `ios/.../App/AppState.swift`

- [ ] **Step 1 — add persisted properties** (top of AppState, near the stored props). Use `UserDefaults` directly (these are non-secret):
```swift
// Persisted session (non-secret; the secret is the Keychain key). Source of truth for
// "this device has an onboarded account" alongside KeyManager.hasStoredKeypair().
private enum SessionKey { static let serverURL = "ainm.session.serverURL"
    static let username = "ainm.session.username"; static let faceID = "ainm.session.faceIDEnabled" }
private func persistSession(serverURL: String, username: String, faceIDEnabled: Bool) {
    let d = UserDefaults.standard
    d.set(serverURL, forKey: SessionKey.serverURL)
    d.set(username, forKey: SessionKey.username)
    d.set(faceIDEnabled, forKey: SessionKey.faceID)
}
var persistedServerURL: String? { UserDefaults.standard.string(forKey: SessionKey.serverURL) }
var persistedUsername: String? { UserDefaults.standard.string(forKey: SessionKey.username) }
var faceIDEnabled: Bool { UserDefaults.standard.bool(forKey: SessionKey.faceID) }
func clearPersistedSession() {
    let d = UserDefaults.standard
    [SessionKey.serverURL, SessionKey.username, SessionKey.faceID].forEach { d.removeObject(forKey: $0) }
}
```

- [ ] **Step 2 — persist on successful connect.** In `completeSignup` (after `await connect()` returns `.connected`) and in `completeRecovery` (same point), call `persistSession(serverURL: serverURL, username: username, faceIDEnabled: <user's Face ID choice>)`. Thread the Face-ID choice from `OnboardingFlowView` into these methods (add a `faceIDEnabled: Bool` parameter to `completeSignup`/`completeRecovery`; the view already knows whether the user tapped "Enable Face ID" vs "Skip"). Only persist when `connectionState == .connected`.

- [ ] **Step 3 — `signOut()`:**
```swift
/// Clear the persisted session and disconnect. Keeps the Keychain key by default so
/// same-device recovery still works; pass forgetKey: true to also delete it.
func signOut(forgetKey: Bool = false) {
    clearPersistedSession()
    if forgetKey { try? keyManager.deleteKeypair() }
    disconnect()                       // existing method (sets .disconnected)
    connectedIdentity = nil
    sessionPhase = .onboarding
}
```

- [ ] **Step 4 — build** (`xcodebuild ... build`), then **commit:** `rtk proxy git add ios/.../AppState.swift && rtk proxy git commit -m "feat(ios): persist session identity + signOut"`

### Task 1.3: `sessionPhase` + launch routing

**Files:** Modify `ios/.../App/AppState.swift`, `ios/.../App/AINMMessengerApp.swift`

- [ ] **Step 1 — add session phase to AppState:**
```swift
enum SessionPhase: Sendable { case onboarding, locked, active }
var sessionPhase: SessionPhase = .onboarding
```

- [ ] **Step 2 — `bootstrap()` after setup.** Add to AppState:
```swift
/// Decide the launch route. Called once after setup(). Active session = persisted identity
/// + a stored device key. Face-ID gate is applied by SessionGateView when faceIDEnabled.
func bootstrap() async {
    guard let url = persistedServerURL, let user = persistedUsername,
          keyManager.hasStoredKeypair() else {
        sessionPhase = .onboarding; return
    }
    serverURL = url; displayName = user
    sessionPhase = faceIDEnabled ? .locked : .active
    if sessionPhase == .active { await connect() }   // SessionGateView handles .locked → connect
}
```

- [ ] **Step 3 — call bootstrap in app entry.** In `AINMMessengerApp.swift` `.task`, after `try? appState.setup()` (and outside the `#if DEBUG` UAT block, which force-sets `.connected`), add: `await appState.bootstrap()`. Guard so the DEBUG seed path (which sets `.connected` directly) is unaffected — only call `bootstrap()` when not in the seeded-fixture path.

- [ ] **Step 4 — root view routes on sessionPhase.** Replace the `switch appState.connectionState` in `AINMMessengerApp.swift` body with:
```swift
switch appState.sessionPhase {
case .onboarding: OnboardingFlowView(appState: appState)
case .locked:     SessionGateView(appState: appState)   // Task 1.4
case .active:     ConversationListView(appState: appState)
}
```
On a successful onboarding/recovery connect, set `appState.sessionPhase = .active` (do this in `completeSignup`/`completeRecovery` right after persisting). Keep the DEBUG seed path setting `.connected` AND `sessionPhase = .active`.

- [ ] **Step 5 — build, manual sanity** (relaunch in sim with an existing account skips onboarding), then **commit:** `rtk proxy git add ios/.../AppState.swift ios/.../AINMMessengerApp.swift && rtk proxy git commit -m "feat(ios): launch routing via sessionPhase (resume saved account)"`

### Task 1.4: SessionGateView (Face-ID unlock + Locked)

**Files:** Create `ios/.../UI/Onboarding/SessionGateView.swift`

- [ ] **Step 1 — implement** a view that on appear runs `BiometricManager.authenticate(reason:)` (existing); on success → `await appState.connect(); appState.sessionPhase = .active`. On failure → show "Locked" with **Try again** (re-run auth) and **Sign out** (`appState.signOut()`). If biometrics unavailable, follow the existing best-effort pattern (proceed to connect). Use `appState.serverURL`/`displayName` already set by `bootstrap()`.
```swift
struct SessionGateView: View {
    @Bindable var appState: AppState
    @State private var failed = false
    var body: some View {
        VStack(spacing: 16) { /* lock icon + "Unlock AINM" + Try again / Sign out when failed */ }
            .task { await unlock() }
    }
    private func unlock() async {
        do { try await BiometricManager.authenticate(reason: "Unlock AINM")
             await appState.connect(); appState.sessionPhase = .active }
        catch { failed = true }
    }
}
```

- [ ] **Step 2 — build, commit:** `rtk proxy git add ios/.../SessionGateView.swift && rtk proxy git commit -m "feat(ios): SessionGateView (Face-ID unlock on resume)"`

---

## Phase 2 — Auto-reconnect

### Task 2.1: Reconnect on WS drop

**Files:** Modify `ios/.../Networking/AINMClient.swift`, `ios/.../App/AppState.swift`, `ios/.../UI/ConversationList/ConversationListView.swift`

- [ ] **Step 1 — read** `AINMClient.startReceiveLoop()` and the `ReconnectionManager` API (backoff/next-delay/reset) to match its surface.
- [ ] **Step 2 — on receive-loop termination** (WS closed/errored) while `state` was `.connected` and no user-initiated disconnect: set `state = .connecting`, notify AppState, then loop: `await reconnection.nextDelay()` sleep → re-run the `connect(serverURL:displayName:)` flow; on success stop; on repeated failure stop at the backoff ceiling with `state = .disconnected`. Add an `isUserDisconnect` flag set true by a new `disconnect()` so user sign-out/disconnect cancels reconnection.
- [ ] **Step 3 — surface state.** Add an `onConnectionStateChange` callback (or reuse an existing state callback) so AppState maps client `.connecting/.connected/.disconnected` → `AppState.connectionState`. Ensure reconnect does NOT bounce `sessionPhase` out of `.active`.
- [ ] **Step 4 — banner.** In `ConversationListView`, when `appState.connectionState == .connecting` (and session active), show a thin "Reconnecting…" banner; hide on `.connected`.
- [ ] **Step 5 — build; manual test:** kill the dogfood server, confirm the app shows Reconnecting then reconnects when the server returns (no re-onboard). **Commit:** `rtk proxy git add ios/.../AINMClient.swift ios/.../AppState.swift ios/.../ConversationListView.swift && rtk proxy git commit -m "feat(ios): auto-reconnect on WebSocket drop"`

---

## Phase 3 — Onboarding robustness

### Task 3.1: Server — idempotent key bind (TDD)

**Files:** Modify `server/src/server.rs` (`bind_key_handler`), maybe `server/src/accounts.rs`; Test `server/tests/` (auth/account integration test, gated on DB)

- [ ] **Step 1 — failing test.** Add an integration test (mirror `auth_sessions_test.rs` account+key insert helpers): signup→verify→bind a key, then bind the **same** public key with a fresh enrollment token for the **same** account → expect `204` (today: `409`). And: binding that key to a **different** account → expect `409`.
- [ ] **Step 2 — run, expect FAIL** (currently returns 409 on the repeat).
- [ ] **Step 3 — implement.** In `bind_key_handler`, before/instead of mapping `StorageError::Duplicate` to 409: if the duplicate is because this exact `public_key` is already bound to **this** `account_id` (look it up via accounts.rs), return `StatusCode::NO_CONTENT`. Only a key bound to a *different* account returns `409`.
- [ ] **Step 4 — run live** (`AINM_DATABASE__URL=... cargo test -p ainm-server --test <file>`), expect PASS.
- [ ] **Step 5 — clippy on touched files; commit:** `rtk proxy git add server/src/server.rs server/src/accounts.rs server/tests/<file> && rtk proxy git commit -m "fix(server): idempotent key bind for same account+key (was 409)"`

### Task 3.2: Client — verify-step split

**Files:** Modify `ios/.../UI/Onboarding/OnboardingFlowView.swift`, `ios/.../App/AppState.swift`

- [ ] **Step 1 — split AppState.** Replace `completeSignup(...code:pin:)` with two methods: `verifyCode(serverURL:email:code:) async -> String?` (calls `account.verify`, returns the enrollment token or sets `onboardingError`) and `finishSignup(serverURL:username:enrollmentToken:pin:faceIDEnabled:) async -> Bool` (getOrCreateKeypair → bindKey → escrow enroll → set serverURL/displayName → connect → persist + `sessionPhase=.active` on success). Keep `completeRecovery` analogous if it has the same trap.
- [ ] **Step 2 — restructure the view.** At the `.code` step's Continue button, call `verifyCode`; on success store the returned token in `@State` and advance to `.pin`; on failure stay on `.code` with the error. `.faceID` → `.working` calls `finishSignup(... enrollmentToken: heldToken ...)`. On `working` failure, route back to a **bind/connect retry** (re-call `finishSignup` with the same held token) — NOT back to `.code`. The OTP is already consumed at `.code`, so it's never re-verified.
- [ ] **Step 3 — build; manual test** the failure path doesn't strand on the code step. **Commit:** `rtk proxy git add ios/.../OnboardingFlowView.swift ios/.../AppState.swift && rtk proxy git commit -m "fix(ios): split verify from bind/connect so failures don't strand on consumed code"`

---

## Phase 4 — Assistant button

### Task 4.1: Fix the SF Symbol

**Files:** Modify `ios/.../UI/ConversationList/ConversationListView.swift`

- [ ] **Step 1 — replace** `Image(systemName: "bubble.left.and.sparkles")` with a verified-valid symbol. Verify availability (e.g. in a quick SwiftUI preview or against SF Symbols): prefer `"sparkles"` or `"wand.and.stars"` or `"bubble.left.and.text.bubble.right"`. Keep the `.accessibilityLabel("Chat with Assistant")` and the `.disabled(connectionState != .connected)` gate and `startAssistantThread` action.
- [ ] **Step 2 — build; confirm** the button renders in the toolbar when connected (screenshot the sim). **Commit:** `rtk proxy git add ios/.../ConversationListView.swift && rtk proxy git commit -m "fix(ios): render Chat-with-Assistant toolbar button (valid SF Symbol)"`

---

## Phase 5 — Regression tests

### Task 5.1: Server — assert canonical delivered envelope

**Files:** Modify `server/tests/ws_conversation_e2e.rs`

- [ ] **Step 1 — extend the existing assertion.** Where the test reads the `ainm.message.deliver` `params.message`, add asserts: `message["type"] == "message"`; first content block has `type=="text"` and non-null `version`, `source`, `fallback`, `text`. (Keep the existing canned-text substring + signature asserts.)
- [ ] **Step 2 — run live, expect PASS** (the Phase-fix-from-today server already emits this). **Commit:** `rtk proxy git add server/tests/ws_conversation_e2e.rs && rtk proxy git commit -m "test(server): assert hosted-agent delivers a canonical envelope"`

### Task 5.2: iOS — envelope/capability decode tests

**Files:** Create `ios/.../AINMMessengerTests/NetworkingTests/EnvelopeDecodeTests.swift`

- [ ] **Step 1 — write three XCTests** (following the app's existing decode-test patterns):
  1. `test_initializeResult_decodesWithoutMaxFields` — decode a JSON `InitializeResult` whose `capabilities` lacks `maxContentBlocks`/`maxMessageSize` → succeeds, `streaming==true`.
  2. `test_deliverWrapper_decodesInnerEnvelope` — given `{ "message": {…canonical envelope…}, "verified": true }`, replicate `handleMessageDelivery`'s extract-`message`-then-decode → `MessageEnvelope.from == "assistant@localhost"`.
  3. `test_agentCanonicalEnvelope_decodes` — decode an envelope with `type:"message"` + a `{type:text,version,source,text,fallback}` block into `MessageEnvelope` with `content` first block `.text`.
- [ ] **Step 2 — run, expect PASS:** `xcodebuild ... -only-testing:AINMMessengerTests/EnvelopeDecodeTests test`. **Commit:** `rtk proxy git add ios/.../EnvelopeDecodeTests.swift && rtk proxy git commit -m "test(ios): decode regression tests for initialize + delivered envelopes"`

---

## Final verification (DoD)

- [ ] Relaunch app w/ saved account → no onboarding, auto-connect (Face-ID-gated if enabled).
- [ ] Kill+restart server → app reconnects automatically (banner).
- [ ] Onboarding failure after code step → no strand; retry resumes from bind/connect.
- [ ] Same device-key re-bind → 204, not 409.
- [ ] Assistant button visible + opens agent thread.
- [ ] `AINM_DATABASE__URL=... cargo test -p ainm-server` green except the two known failures; iOS test bundle green; clippy clean on touched files.
- [ ] Dispatch final code-review across the branch; then `superpowers:finishing-a-development-branch`.

## Self-Review

- **Spec coverage:** Phase 1 ↔ spec §3; Phase 2 ↔ §4; Phase 3 ↔ §5; Phase 4 ↔ §6; Phase 5 ↔ §7. DoD ↔ §8. Covered.
- **Type consistency:** `sessionPhase`/`SessionPhase`, `hasStoredKeypair()`, `verifyCode`/`finishSignup`, `signOut(forgetKey:)`, `persistSession` used consistently across tasks.
- **Implementer adaptation flags:** exact `KeyManager` test ctor, `ReconnectionManager` API surface, and `BiometricManager.authenticate` signature must be matched against the real code (each task says "read/mirror"). These are existing symbols, not placeholders.
- **No-signing-flag reminder** is called out (Keychain depends on the sign-to-run-locally build).
