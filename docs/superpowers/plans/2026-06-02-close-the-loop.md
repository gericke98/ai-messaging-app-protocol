# Close the Loop: App ↔ Hosted Runtime — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Wire the iOS app to address the built-in `assistant@<domain>` agent so a human can have one real, streamed conversation with it — via a hardcoded "Chat with Assistant" entry point — against a documented DB-backed local server, culminating in the human-run IOS-11 validation.

**Architecture:** App-side only (+ a server run-helper script + a runbook). Add an optional `to` field to `MessageEnvelope` (the server already triggers the agent on `params.to == agent.address`), persist a target `agentAddress` on a thread (GRDB), derive `assistant@<host>` from `serverURL`, add a toolbar entry point, and set `to` on every message in an agent thread. Replies stream back through the already-working `ainm.stream.*`/`ainm.message.deliver` path. No server logic changes.

**Tech Stack:** Swift / SwiftUI (Xcode project `ios/AINMMessenger/AINMMessenger.xcodeproj`, scheme `AINMMessenger`), GRDB (incremental migrations), XCTest (`AINMMessengerTests`). Build/test: `xcodebuild ... -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16,OS=18.0' CODE_SIGNING_ALLOWED=NO` (discover an available sim with `xcrun simctl list devices available` and substitute if iPhone 16/OS 18.0 is absent; for compile-only checks `-destination 'generic/platform=iOS Simulator'` is acceptable). The `AinmEscrow.xcframework` is present at `ios/AINMMessenger/Frameworks/`.

**Paths:** all iOS paths below are under `ios/AINMMessenger/AINMMessenger/`.

---

## Verified facts (from exploration — don't re-derive)

- `Models/MessageEnvelope.swift`: `struct MessageEnvelope: Codable, Sendable` with a **custom `encode(to:)`** and a **synthesized** `init(from:)`. Fields: id, from, threadId, timestamp, type, content, `var signature: String?`, `let parentId: String?`. CodingKeys map snake_case (`thread_id`, `parent_id`). Optional fields are encoded only when present.
- `Persistence/ThreadRecord.swift`: GRDB `FetchableRecord, PersistableRecord`, table `"thread"`. Has a `init(from metadata:)` and a memberwise `init(...)`.
- `Persistence/AppDatabase.swift`: incremental GRDB migrations; latest relevant is `v4_create_agent_cards`. Pattern for adding a column: `migrator.registerMigration("vN_...") { db in try db.alter(table: "thread") { t in t.add(column: "x", .text) } }` (see `v2_add_thread_hasUnread`).
- `App/AppState.swift`: `var serverURL: String = "wss://localhost:8080"`; `restBaseURL(from:)` (line ~201) **already** derives the HTTP REST base from `serverURL` (so §3.4 REST-base concern is already solved); `connectedIdentity = "\(displayName)@\(URL(string: serverURL)?.host ?? "localhost")"` (~178); `createThread(title:firstMessageText:)` (~344) builds a threadId UUID + envelope + persists; a send path around ~445-465 builds `MessageEnvelope(...)` for a user message in a thread; `sendMessage(envelope:)` (~336) forwards to the client.
- `Networking/AINMClient.swift`: `sendMessage(envelope:)` (~186) encodes the envelope → dict → signs → wraps `{method:"ainm.message.send", params: dict}`. So any field on `MessageEnvelope` flows into `params` and is **signed**. Connect (~109) uses the passed `serverURL` directly (honors `ws://`). Streaming + `ainm.message.deliver` handling is complete (~587-690).
- `Networking/AccountService.swift`: `init(baseURL: URL)`, appends paths to it.
- Server trigger (`server/src/server.rs:2455`): agent runs if `thread_id.contains("assistant@") || params.to == agent.address || content_mentions(...)`. We use `params.to`.
- Server run: human-mode with `AINM_DATABASE__URL` auto-provisions `assistant@<server_domain>` and (with `AINM_LLM__API_KEY`) a real LLM (`server/src/main.rs:511-525`).

---

## Task LOOP-1: Data model — `to` on the envelope + `agentAddress` on threads

**Files:**
- Modify: `Models/MessageEnvelope.swift`
- Modify: `Persistence/ThreadRecord.swift`
- Modify: `Persistence/AppDatabase.swift` (new migration)
- Test: `AINMMessengerTests/NetworkingTests/` (envelope) + `AINMMessengerTests/PersistenceTests/` (thread record)

- [ ] **Step 1: Write the failing tests**

Add an envelope test (in the NetworkingTests group, e.g. `MessageEnvelopeToFieldTests.swift`):

```swift
import XCTest
@testable import AINMMessenger

final class MessageEnvelopeToFieldTests: XCTestCase {
    private func sample(to: String?) -> MessageEnvelope {
        MessageEnvelope(
            id: "m1", from: "alice@localhost", threadId: "t1",
            timestamp: "2026-06-02T00:00:00.000Z", type: .message,
            content: [], signature: nil, parentId: nil, to: to
        )
    }

    func testToEncodedWhenPresent() throws {
        let data = try JSONEncoder().encode(sample(to: "assistant@localhost"))
        let json = try JSONSerialization.jsonObject(with: data) as! [String: Any]
        XCTAssertEqual(json["to"] as? String, "assistant@localhost")
    }

    func testToOmittedWhenNil() throws {
        let data = try JSONEncoder().encode(sample(to: nil))
        let json = try JSONSerialization.jsonObject(with: data) as! [String: Any]
        XCTAssertNil(json["to"], "`to` must be omitted (not null) when unset")
    }

    func testRoundTrip() throws {
        let data = try JSONEncoder().encode(sample(to: "assistant@localhost"))
        let decoded = try JSONDecoder().decode(MessageEnvelope.self, from: data)
        XCTAssertEqual(decoded.to, "assistant@localhost")
    }
}
```

Add a thread-record test (PersistenceTests group, e.g. `ThreadAgentAddressTests.swift`) — follow the existing PersistenceTests setup for an in-memory/temp `AppDatabase` (mirror a sibling test's harness):

```swift
import XCTest
import GRDB
@testable import AINMMessenger

final class ThreadAgentAddressTests: XCTestCase {
    func testAgentAddressPersistsAndDefaultsNil() throws {
        let dbQueue = try DatabaseQueue() // in-memory
        try AppDatabase.migrator.migrate(dbQueue)   // use the app's shared migrator (see note)

        try dbQueue.write { db in
            var withAgent = ThreadRecord(
                id: "t-agent", title: "Assistant", createdAt: "2026-06-02T00:00:00.000Z",
                createdBy: "alice@localhost", parentThreadId: nil, parentMessageId: nil,
                agentAddress: "assistant@localhost"
            )
            try withAgent.insert(db)
            var plain = ThreadRecord(
                id: "t-plain", title: "Chat", createdAt: "2026-06-02T00:00:00.000Z",
                createdBy: "alice@localhost", parentThreadId: nil, parentMessageId: nil
            )
            try plain.insert(db)
        }
        try dbQueue.read { db in
            let a = try ThreadRecord.fetchOne(db, key: "t-agent")
            XCTAssertEqual(a?.agentAddress, "assistant@localhost")
            let p = try ThreadRecord.fetchOne(db, key: "t-plain")
            XCTAssertNil(p?.agentAddress)
        }
    }
}
```

NOTE: how the app exposes its `migrator` — read `AppDatabase.swift` and use the real accessor (it may be `static var migrator`, or migrations applied in an initializer). If migrations aren't exposed as a static `migrator`, mirror exactly how the existing PersistenceTests build a migrated test database (there are sibling tests in `AINMMessengerTests/PersistenceTests` — copy their harness).

- [ ] **Step 2: Run to verify they fail**

Run (substitute an available sim if needed):
```
cd ios/AINMMessenger && xcodebuild test -project AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16,OS=18.0' CODE_SIGNING_ALLOWED=NO \
  -only-testing:AINMMessengerTests/MessageEnvelopeToFieldTests \
  -only-testing:AINMMessengerTests/ThreadAgentAddressTests
```
Expected: FAIL to compile (`to:`/`agentAddress:` args don't exist yet).

- [ ] **Step 3: Add `to` to `MessageEnvelope`**

In `Models/MessageEnvelope.swift`:
- Add the stored property (default `nil` so existing call sites compile unchanged): after `parentId`, add `var to: String? = nil   // optional explicit recipient (e.g. an agent address)`.
- Add `case to` to `CodingKeys` (no rename needed — JSON key is `to`).
- In the custom `encode(to:)`, after the `parentId` block, add:
```swift
        if let to {
            try container.encode(to, forKey: .to)
        }
```
- The synthesized `init(from:)` handles the optional automatically (decodeIfPresent). Do NOT add a custom decoder.

Because `to` has a default, the memberwise initializer keeps working for all existing call sites; the test constructs it with `to:` explicitly (allowed).

- [ ] **Step 4: Add `agentAddress` to `ThreadRecord` + migration**

In `Persistence/ThreadRecord.swift`:
- Add stored property: `var agentAddress: String? = nil   // target agent for this thread (e.g. assistant@host); nil = human thread`.
- In `init(from metadata:)`, set `self.agentAddress = nil`.
- In the memberwise `init(...)`, add a trailing parameter `agentAddress: String? = nil` and `self.agentAddress = agentAddress`.

In `Persistence/AppDatabase.swift`, register a NEW migration after the last one (`v4_create_agent_cards`):
```swift
        migrator.registerMigration("v5_add_thread_agentAddress") { db in
            try db.alter(table: "thread") { t in
                t.add(column: "agentAddress", .text)   // nullable; nil = human thread
            }
        }
```
(Match the exact migrator variable/closure style used by `v2_add_thread_hasUnread`.)

- [ ] **Step 5: Run the tests to verify they pass**

Same command as Step 2. Expected: PASS (3 envelope + 1 persistence). If the full simulator test is slow/flaky in this environment, at minimum run `xcodebuild build ...` to confirm compilation and run the targeted tests once; report the result.

- [ ] **Step 6: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Models/MessageEnvelope.swift \
  ios/AINMMessenger/AINMMessenger/Persistence/ThreadRecord.swift \
  ios/AINMMessenger/AINMMessenger/Persistence/AppDatabase.swift \
  ios/AINMMessenger/AINMMessengerTests/
rtk proxy git commit -m "feat(loop): MessageEnvelope.to + thread.agentAddress (migration v5)"
```

---

## Task LOOP-2: AppState — derive assistant address, start an agent thread, set `to` on sends

**Files:**
- Modify: `App/AppState.swift`
- Test: `AINMMessengerTests/` (a new `AssistantAddressTests.swift`, App/logic group)

FIRST read `App/AppState.swift` fully (esp. `serverURL`, `createThread` ~344, the send path ~445-465, `sendMessage` ~336, and the in-memory thread list/lookup) so the wiring matches the real method names.

- [ ] **Step 1: Write the failing test for the pure helper**

```swift
import XCTest
@testable import AINMMessenger

final class AssistantAddressTests: XCTestCase {
    func testDerivesAssistantAddressFromServerURL() {
        XCTAssertEqual(AppState.assistantAddress(forServerURL: "ws://localhost:8080"), "assistant@localhost")
        XCTAssertEqual(AppState.assistantAddress(forServerURL: "wss://ainm.example.com:443"), "assistant@ainm.example.com")
    }
    func testNilWhenNoHost() {
        XCTAssertNil(AppState.assistantAddress(forServerURL: "not a url"))
    }
}
```

- [ ] **Step 2: Run to verify it fails** — same `xcodebuild test` form with `-only-testing:AINMMessengerTests/AssistantAddressTests`. Expected: FAIL (no `assistantAddress`).

- [ ] **Step 3: Add the pure helper**

In `App/AppState.swift`, add a `static` function (testable without constructing AppState):
```swift
    /// Derive the built-in agent address `assistant@<host>` from a WS server URL.
    /// Returns nil if the URL has no host. The host must equal the server's
    /// `server_domain` (the handshake/account binding is `username@<host>`).
    static func assistantAddress(forServerURL serverURL: String) -> String? {
        guard let host = URL(string: serverURL)?.host else { return nil }
        return "assistant@\(host)"
    }
```

- [ ] **Step 4: Add `startAssistantThread()` + set `to` on agent-thread sends**

Add a method that creates a thread bound to the assistant and returns its id (mirror `createThread` but persist `agentAddress` and don't require a first message):
```swift
    /// Create (and persist) a thread addressed to the built-in assistant agent.
    /// Returns the new threadId, or nil if the agent address can't be derived.
    @discardableResult
    func startAssistantThread() async -> String? {
        guard let agent = AppState.assistantAddress(forServerURL: serverURL) else { return nil }
        let threadId = UUID().uuidString
        let now = ISO8601DateFormatter.ainmString()   // use the SAME timestamp helper the file already uses
        let record = ThreadRecord(
            id: threadId, title: "Assistant", createdAt: now,
            createdBy: connectedIdentity ?? "me", parentThreadId: nil, parentMessageId: nil,
            agentAddress: agent
        )
        // Persist via the same path createThread uses (ThreadRepository / database write).
        await persistThread(record)   // adapt to the real persistence call used by createThread
        return threadId
    }
```
Then, in the method that builds the outbound `MessageEnvelope` for a user-typed message in a thread (the ~445-465 send path), look up that thread's `agentAddress` and pass it as `to`:
```swift
        let agentTo = try? await threadAgentAddress(threadId)   // fetch ThreadRecord.agentAddress, nil if none/human
        let envelope = MessageEnvelope(
            id: ..., from: connectedIdentity ?? ..., threadId: threadId,
            timestamp: ..., type: .message, content: ..., signature: nil, parentId: ...,
            to: agentTo
        )
```

IMPLEMENTATION NOTES:
- Use the file's existing timestamp formatter and thread-persistence call rather than the placeholders above — read `createThread` and copy its exact idioms (`ISO8601DateFormatter` usage, the `ThreadRepository`/`db` write, how `connectedIdentity`/`from` is set).
- `threadAgentAddress(threadId)` can read the in-memory thread list if AppState keeps one, else fetch `ThreadRecord.fetchOne(db, key: threadId)?.agentAddress`. Keep it small and synchronous-ish; non-agent threads return nil so `to` is omitted (human↔human unchanged).
- Do NOT change the human↔human send behavior when `agentAddress` is nil.

- [ ] **Step 5: Run tests + build** — `-only-testing:AINMMessengerTests/AssistantAddressTests` passes; full `xcodebuild build` is clean. Report results.

- [ ] **Step 6: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/App/AppState.swift ios/AINMMessenger/AINMMessengerTests/
rtk proxy git commit -m "feat(loop): AppState assistant-address helper, startAssistantThread, to-on-send"
```

---

## Task LOOP-3: "Chat with Assistant" entry point (UI)

**Files:**
- Modify: `UI/ConversationList/ConversationListView.swift`

- [ ] **Step 1: Read the view** to find the toolbar and how it currently opens a new thread / navigates (it already has a new-thread affordance via `NewThreadComposerView` and a navigation mechanism — reuse the same navigation path).

- [ ] **Step 2: Add the entry point**

Add a toolbar button (e.g. a `bubble.left.and.sparkles` / `cpu` SF Symbol labeled "Assistant") that:
```swift
            ToolbarItem(placement: .topBarLeading) {  // or wherever the existing new-thread button sits
                Button {
                    Task {
                        if let threadId = await appState.startAssistantThread() {
                            // navigate into the new thread using the SAME mechanism the list
                            // uses to open a thread (e.g. set a NavigationPath / selection).
                            navigateToThread(threadId)
                        }
                    }
                } label: {
                    Label("Chat with Assistant", systemImage: "bubble.left.and.sparkles")
                }
            }
```
Wire `navigateToThread` to the view's existing navigation (NavigationStack path append or selection binding — mirror how tapping a `ConversationRowView` opens `MessageThreadView`). If the list isn't connected yet at launch (no live connection), the button can still create the thread; sending requires being connected (handled by the existing send path).

- [ ] **Step 3: Build** — `xcodebuild build ... -scheme AINMMessenger ...` clean. (UI; no unit test required, but it must compile and the symbol/navigation must resolve.)

- [ ] **Step 4: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/ConversationList/ConversationListView.swift
rtk proxy git commit -m "feat(loop): Chat with Assistant entry point in conversation list"
```

---

## Task LOOP-4: Dogfood run helper + IOS-11 runbook

**Files:**
- Create: `scripts/dogfood.sh`
- Modify: `Makefile` (add a `dogfood` target)
- Create: `docs/runbooks/2026-06-02-ios11-close-the-loop.md`

No app/server logic. This makes a DB-backed human-mode server trivially launchable for the live test.

- [ ] **Step 1: `scripts/dogfood.sh`**

```bash
#!/usr/bin/env bash
# Launch a DB-backed human-mode AINM server for iOS dogfooding (IOS-11).
# Prereqs: a reachable Postgres (see server/docker-compose.yml's `postgres` service,
# or a local PG). Set AINM_LLM__API_KEY for real assistant replies.
set -euo pipefail

: "${AINM_DATABASE__URL:=postgres://ainm:dev_password@localhost:5432/ainm}"
export AINM_DATABASE__URL
# No Resend key -> MockEmailSender logs the OTP code to stdout.
unset AINM_EMAIL__RESEND_API_KEY || true
# Optional: real LLM replies. Without it the agent loop runs but the LLM call won't produce output.
if [ -z "${AINM_LLM__API_KEY:-}" ]; then
  echo "[dogfood] WARNING: AINM_LLM__API_KEY unset — the assistant will run its loop but produce no LLM reply." >&2
fi

echo "[dogfood] DB: $AINM_DATABASE__URL"
echo "[dogfood] starting human-mode server on ws://localhost:8080  (identity/domain = localhost)"
echo "[dogfood] point the iOS app's serverURL at  ws://localhost:8080"
echo "[dogfood] watch this log for the email OTP code during signup."
cd "$(dirname "$0")/../server"
exec cargo run --bin ainm-server -- --mode human --port 8080 --identity localhost
```
Make it executable: `chmod +x scripts/dogfood.sh`.

NOTE: confirm the exact human-mode CLI flags by reading `server/src/main.rs` arg parsing (clap `Cli`) — `--mode human --port 8080 --identity localhost` should set `server_id/server_domain = localhost`. If the flag names differ (e.g. `--identity` vs `--server-domain`), use the real ones so `server_domain == localhost`. The host the simulator connects to (`localhost`) MUST equal the server domain (account binding is `username@localhost`).

- [ ] **Step 2: `Makefile` target**

```make
dogfood: ## Launch a DB-backed human-mode server for iOS dogfooding (IOS-11). Set AINM_LLM__API_KEY for real replies.
	@bash scripts/dogfood.sh
```

- [ ] **Step 3: Runbook `docs/runbooks/2026-06-02-ios11-close-the-loop.md`**

Write a concrete, ordered runbook:
1. **Start Postgres:** `docker compose -f server/docker-compose.yml up -d postgres` (or use a local PG; ensure a DB matching `AINM_DATABASE__URL`).
2. **Start the server:** `AINM_LLM__API_KEY=sk-... make dogfood` — confirm it logs provisioning of `assistant@localhost` and "server ready". Migrations 001–006 apply on connect.
3. **Configure the app:** in the iOS app onboarding, set the server URL to `ws://localhost:8080` (plain ws — the local server's TLS is on the separate federation port). The simulator reaches the host via `localhost`.
4. **Sign up:** email + username → the server log prints the OTP code (MockEmailSender) → enter it → set PIN. This binds the key (`username@localhost`).
5. **Connect:** the app's `ainm.initialize` handshake is signed by the bound key and the server verifies it (±300s skew — host clock = simulator clock, fine).
6. **Chat with Assistant:** tap the "Chat with Assistant" toolbar button → a thread opens addressed to `assistant@localhost`.
7. **Send "What's 2+2?"** → observe `ainm.stream.*` deltas render the assistant's reply live, then the final `ainm.message.deliver`.
8. **Record:** success/failure, round-trip latency, and any errors (with server-log tail). Note whether memory briefing / tool calls appeared.

Document the common failure modes: handshake rejected (host ≠ server domain, or unbound key), no reply (AINM_LLM__API_KEY unset), connection refused (server not on 8080 / app pointed at wss).

- [ ] **Step 4: Commit**

```bash
rtk proxy git add scripts/dogfood.sh Makefile docs/runbooks/2026-06-02-ios11-close-the-loop.md
rtk proxy git commit -m "chore(loop): dogfood run helper + IOS-11 runbook"
```

---

## Task LOOP-5: Full build + test + verify §3.4 + DoD

**Files:** none (verification); small fixes only if §3.4 items are actually broken.

- [ ] **Step 1: §3.4 verification (verify; fix ONLY if broken)**

- **`ws://` scheme:** confirm `AINMClient.connect` (and the onboarding server-URL field) accept `ws://` — it passes `serverURL` straight to `URLSessionWebSocketTask`, which honors the scheme. If the onboarding UI hard-forces `wss://` or rejects `ws://`, relax it to accept the user-entered scheme. If already fine, no change.
- **REST base URL:** confirm `AppState.restBaseURL(from:)` maps `ws→http`/`wss→https` and the same host/port, and that `AccountService` is constructed from it. (Exploration says this already works.) If already fine, no change.
Report findings; commit any fix separately with message `fix(loop): accept ws:// + REST base for local dogfood`.

- [ ] **Step 2: Full app build**

```
cd ios/AINMMessenger && xcodebuild build -project AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16,OS=18.0' CODE_SIGNING_ALLOWED=NO
```
Expected: BUILD SUCCEEDED. (Discover/replace the destination if needed.)

- [ ] **Step 3: Full app test suite**

```
cd ios/AINMMessenger && xcodebuild test -project AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16,OS=18.0' CODE_SIGNING_ALLOWED=NO
```
Expected: all tests pass, including the new `MessageEnvelopeToFieldTests`, `ThreadAgentAddressTests`, `AssistantAddressTests`. Name any failure and classify (regression vs pre-existing/env). If the simulator/test run is infeasible in this environment, report that explicitly and fall back to a clean `xcodebuild build` + the targeted new tests; do NOT claim tests passed if they didn't run.

- [ ] **Step 4: Verify DoD against the spec**

Re-read `docs/superpowers/specs/2026-06-02-close-the-loop-design.md` §6 and confirm each of the 7 items. In particular: (1) `to` omitted when nil (test asserts), (2) agent-thread messages set `to`, (3) "Chat with Assistant" opens a thread addressed to `assistant@<host>`, (5) `make dogfood` + runbook exist, (6) build green + new tests pass. Item 7 (the live IOS-11 conversation) is the user's human-run gate — note it as the remaining manual step, not something this task executes.

- [ ] **Step 5: Final commit (if any verification fixes)** — otherwise nothing to commit; report done.

---

## Self-Review (completed during planning)

- **Spec coverage:** §3.1 `to` → LOOP-1; §3.2 `agentAddress` → LOOP-1; §3.3 entry point → LOOP-2 (logic) + LOOP-3 (UI); §3.4 verify → LOOP-5; §3.5 run helper → LOOP-4; §3.6 runbook → LOOP-4; §5 tests → LOOP-1/2 + LOOP-5; §6 DoD → LOOP-5. All covered.
- **Placeholder scan:** the AppState send-wiring (LOOP-2 Step 4) intentionally references "the real idioms" because the exact method names/timestamp helper must be read from the file — the contract (every agent-thread message carries `to`; human threads unchanged) is explicit, with file:line hints. All other code is complete.
- **Type consistency:** `MessageEnvelope(... to:)`, `ThreadRecord(... agentAddress:)`, `AppState.assistantAddress(forServerURL:)`, `startAssistantThread() -> String?` are referenced identically across tasks.
- **Biggest risk:** the AppState send path (which exact method builds the outbound user-message envelope) — LOOP-2 must read AppState to wire `to` at the right spot; flagged. Second: iOS simulator availability for `xcodebuild test` — destination discovery + a build-only fallback are specified so the task can't silently stall.
- **Scope honesty:** IOS-11 (the live conversation) is human-run and is the closing validation, not an automated task.
