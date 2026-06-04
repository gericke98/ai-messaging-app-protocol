# iOS Reference-Client Robustness — Design (v1)

**Date:** 2026-06-04
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)
**Context:** The IOS-11 live run (first real iOS client ↔ real server conversation) succeeded but
exposed a cluster of reference-client gaps that made the app unusable across launches and brittle
during onboarding. Three client/server schema mismatches were fixed inline (commits `8ba8a2a`,
`77bf7da`); this sub-project hardens the remaining behavioral gaps so the app survives real use and
the fixed contracts can't silently regress.

## 1. Goal & scope

Make the iOS reference client behave like a real messaging app: remember the signed-in account
across launches, reconnect automatically when the socket drops, recover gracefully from onboarding
failures, expose the assistant entry point, and lock today's wire-format fixes behind regression
tests.

**In scope (five phases):** persisted session + launch routing (keystone), auto-reconnect,
onboarding robustness (verify-step split + idempotent key bind), the "Chat with Assistant" button
fix, and regression tests.

**Out of scope:** multi-account on one device; agent discovery UX; deployment/HA; changing the
identity/handshake crypto; the streaming-delta render path (the terminal `ainm.message.deliver` is
the authoritative render and is covered).

**Keystone approach (chosen — "A"):** identity in `UserDefaults`/`@AppStorage`, device keypair in
the Keychain as the source of truth. The WS handshake is stateless and key-based (`verify_init`), so
"resume" is just: persisted identity + Keychain keypair present → re-handshake. (Rejected: reusing
the DB-backed auth-session token — that gates REST account-management, not the key-based WS
handshake.)

## 2. Architecture overview

```
App launch
  └─ AppState.bootstrap()
       ├─ hasPersistedSession? (serverURL + username persisted  AND  keypair in Keychain)
       │     ├─ yes → SessionGate
       │     │         ├─ Face ID enrolled at onboarding? → BiometricManager.authenticate()
       │     │         │        ├─ ok / not-enrolled → connect()  (auto-reconnect on drop)
       │     │         │        └─ fail → Locked screen (retry / Sign out)
       │     └─ no  → OnboardingFlowView  (verify-step split; idempotent bind)
       └─ Root view renders by (sessionState, connectionState)
```

Root view today switches onboarding ↔ ConversationListView on `connectionState == .connected`. It
gains a **session dimension**: `none` (onboarding), `locked` (have session, awaiting Face ID),
`active` (connected or reconnecting). ConversationListView shows once a session is `active`, with a
connection banner reflecting `.connecting`/`.disconnected`.

## 3. Phase 1 — Persisted session + launch routing (keystone)

**Persisted identity store.** After a successful `connect()` (in both `completeSignup` and
`completeRecovery`), persist `serverURL` and `username` (the `displayName`) via `@AppStorage`/
`UserDefaults` (these are non-secret; the secret is the Keychain key). Also persist a
`faceIDEnabled` bool reflecting the onboarding choice. Clearing the store = "signed out".

**Launch routing.** Add `AppState.bootstrap()` run at app start:
- `hasPersistedSession = (persisted serverURL & username present) && keyManager.hasStoredKeypair()`.
  Add a non-mutating `KeyManager.hasStoredKeypair()` (a Keychain existence check that does NOT
  create a key — distinct from `getOrCreateKeypair`).
- If `hasPersistedSession`: set `serverURL`/`displayName` from the store and enter the `SessionGate`.
- Else: show `OnboardingFlowView`.

**SessionGate (Face-ID-if-enrolled).** If `faceIDEnabled`, call
`BiometricManager.authenticate(reason:)`; on success (or if biometrics unavailable, per the existing
best-effort pattern) → `connect()`. On failure → a **Locked** screen with "Try again" and "Sign
out". If `!faceIDEnabled` → connect seamlessly.

**Sign out.** A control (Conversations toolbar overflow or a Settings affordance) that clears the
persisted identity and the `faceIDEnabled` flag, disconnects, and returns to onboarding. Whether it
also deletes the Keychain key is a flagged implementer choice; default: keep the key (so recovery on
the same device still works) but clear identity so the next launch onboards/recovers.

**Files:** `App/AppState.swift` (bootstrap, persisted props, connect-time persistence, signOut),
`Crypto/KeyManager.swift` (`hasStoredKeypair()`), the root/app entry view (session-state routing), a
small `SessionGate`/`LockedView` SwiftUI view.

## 4. Phase 2 — Auto-reconnect

`AINMClient` already holds a `reconnection` coordinator (reset on connect). Wire it into the receive
loop: on WS close/error, if the client was `.connected` and not an explicit user disconnect, attempt
a bounded-backoff re-handshake (the full `connect` flow — stateless re-handshake per the protocol).
`AppState.connectionState` transitions `.connected → .connecting → .connected` (or `.disconnected`
after the backoff ceiling). The UI shows a non-blocking "Reconnecting…" banner; threads stay
visible. Explicit Sign out / disconnect cancels reconnection.

**Files:** `Networking/AINMClient.swift` (receive-loop close/error handling + reconnection driver),
`App/AppState.swift` (surface reconnecting state), a small banner in `ConversationListView`.

## 5. Phase 3 — Onboarding robustness

**Verify-step split (client).** `OnboardingFlowView` currently collects the code at the `.code` step
but defers `account.verify(...)` to the terminal `working` step inside `completeSignup`; any later
failure (bind/escrow/connect) dumps back to `.code` with the OTP already consumed → unrecoverable.
Restructure: at the `.code` step, call `verify` immediately and hold the returned `enrollmentToken`
in flow state. `.pin` → `.faceID` → `.working` then performs only bind + escrow + connect using the
held token. A `working` failure returns to a **retry of bind/connect** (re-using the held token),
never to re-verify. `completeSignup` is split accordingly (`verifyCode` → returns token;
`finishSignup(token:pin:)`).

**Idempotent key bind (server).** `bind_key_handler` (server.rs) currently maps a duplicate-key
insert to `409 Conflict`. Make it idempotent: if the presented `public_key` is already bound **to
the same account** (the account resolved from the enrollment token), return `204 No Content`
(success). A key bound to a **different** account still returns `409`. This removes the
"key already bound" wall when onboarding is retried on a device whose key is already enrolled, while
preserving the cross-account safety the global-unique `account_keys.public_key` index provides.

**Files:** `UI/Onboarding/OnboardingFlowView.swift`, `App/AppState.swift` (verify/finish split),
`server/src/server.rs` (`bind_key_handler` idempotency), `server/src/accounts.rs` if a
"key bound to this account?" lookup is needed.

## 6. Phase 4 — "Chat with Assistant" button

`ConversationListView` renders the entry point with `Image(systemName: "bubble.left.and.sparkles")`,
which did not appear in the running build (toolbar showed only memory / discover / compose). Replace
with a verified-valid SF Symbol (candidates: `sparkles`, `wand.and.stars`,
`bubble.left.and.text.bubble.right`) and confirm the button renders and is enabled when
`connectionState == .connected`. No logic change to `startAssistantThread`.

**Files:** `UI/ConversationList/ConversationListView.swift`.

## 7. Phase 5 — Regression tests

Lock the three fixes from today so they can't silently recur.

**Server (Rust):** extend `server/tests/ws_conversation_e2e.rs` to assert the delivered
`ainm.message.deliver` message is a **canonical envelope**: top-level `type == "message"` and a text
content block carrying `version`, `source`, `fallback`, and `text`. (Complements the existing
substring + signature assertions.)

**iOS (XCTest), following existing patterns:**
1. Decode a representative server `ainm.initialize` *result* JSON (with `capabilities` lacking
   `maxContentBlocks`/`maxMessageSize`) into `InitializeResult` — must succeed.
2. Feed a `{ "message": {…canonical envelope…}, "verified": true }` params object through the
   `handleMessageDelivery` decode path (extract `message`, decode `MessageEnvelope`) — must produce
   the envelope with the agent `from`.
3. Decode the agent's canonical envelope (with a `{type:text, version, source, text, fallback}`
   block) into `MessageEnvelope` + `ContentBlock.text` — must succeed.

**Files:** `server/tests/ws_conversation_e2e.rs`, new `ios/.../AINMMessengerTests/` decode tests.

## 8. Testing & DoD

1. Relaunching the app with a saved account skips onboarding and auto-connects (Face-ID-gated when
   the user enabled it); no re-onboarding, no `409`.
2. Killing/restarting the server reconnects the app automatically (banner shows Reconnecting →
   Connected) without user action.
3. An onboarding failure after the code step never strands the user on a consumed code; retry
   resumes from bind/connect.
4. Re-binding the same device key to the same account returns success, not `409`.
5. The "Chat with Assistant" button is visible and opens an agent-addressed thread.
6. Server WS E2E asserts the canonical delivered envelope; iOS decode unit tests cover the three
   payloads; full server suite green except the two known pre-existing failures
   (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`); iOS build green.
7. A short runbook note documents Sign out + that relaunch resumes the session.

## 9. Build order

Phase 1 → 2 → 3 → 4 → 5. Phase 1 delivers the biggest relief (no more forced re-onboarding); Phase 3
removes the remaining onboarding traps; Phase 5 lands last so it also covers any envelope shape
touched along the way.
