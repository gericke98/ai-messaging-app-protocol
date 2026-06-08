# A2A Arc Live Verification Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the loop on the multi-agent / A2A UI arc (SP3 name-tinting + directional `→` byline + federation provenance, the live participant roster, and the SP4 digest capsule) by verifying the **real app** renders and behaves correctly against a realistic multi-agent thread — both as an automated UITest (with screenshots) and a human visual runbook.

**Scope reality (read first):** The server runtime is strictly **human ↔ single-agent today** — there is **no inter-agent message routing** (`runtime_server.rs` `stream_reply` always broadcasts back to the human; the agent loop only responds to human input). Genuine runtime-generated A2A traffic would require building a multi-agent routing feature — a **large, separate effort, explicitly out of scope here**. This plan verifies the **UI/rendering/behavior** of the arc through the real app's real code paths against **seeded** multi-agent data (the same `ScenarioFixture` → `SeedLoader` → GRDB → views pipeline used by all existing UATs). That is the meaningful, achievable "close the loop" today; runtime A2A generation is noted as future work.

**Architecture:** A new seed fixture `scenario-07-a2a-burst.json` (human + 2 agents, one foreign-host, a bounded ≥3-message A2A burst that folds, plus an agent reply directed at the human that stays inline) + a UITest identifier on the capsule + an `A2ACapsuleUITests` that seeds, opens the thread, asserts the collapsed capsule, expands it, asserts the inner messages, and captures screenshots + a human runbook. No production behavior change beyond one accessibility identifier.

**Tech Stack:** Swift 6 / SwiftUI, GRDB, XCTest / XCUITest. Build/test via `xcodebuild`. iOS project at `ios/AINMMessenger`.

---

## Conventions (read once)

- **Git:** ALL git via `rtk proxy git ...` (never bare `git`). Commit to `main` (project pattern — no branch). Do **not** push; that's a separate user-gated step after the plan completes.
- **Build (unit):** `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test`
- **UITests:** same but `-scheme AINMMessengerUITests`. Scope a class with `-only-testing:AINMMessengerUITests/<Class>`.
- **pbxproj hygiene:** new `.swift`/`.json` under `AINMMessenger/`, `AINMMessengerTests/`, `AINMMessengerUITests/` auto-include via the synchronized folder group — zero pbxproj edits. After any `xcodebuild` run, if `project.pbxproj` or any `*.xcscheme` was rewritten, revert: `rtk proxy git checkout -- <path>`. Never commit those.
- **SourceKit noise:** in-editor "Cannot find 'X'" / "No such module" are stale-index false positives. Trust only `xcodebuild`.
- **Existing facts (verified):**
  - Seed launch arg: `-UITestSeed <name>` (no `.json`); fixture resolved via `Bundle.main.url(forResource:withExtension:"json")` from `Testing/Fixtures/`. Seeding sets `connectedIdentity = "uat-user@localhost"`, `sessionPhase = .active`, and populates the DB; it does **not** auto-navigate — tests call `openThread(threadId:)`.
  - `FixtureMessage` decodes `id, threadId, fromAddress, timestamp, type, contentJson, signature, serverTimestamp` — **no `toAddress` key** (so a seeded message's directional target derives from a `mention` inside `contentJson`, via `MessageGroup.deriveDirectedTo` → first mention).
  - Fixture seeds `participant` rows directly → `isMultiParticipant` is true without the roster backfill (backfill only runs when participants are empty).
  - `A2ACapsule.id` = the first folded group's id = the first A2A message's id.
  - UITest base `AINMUITestCase` provides `launchWith(seedFixture:eventScript:deepLink:timeout:)` and `openThread(threadId:)`; readiness marker `UITestID.App.ready`; conversation cell `UITestID.ConversationList.cell(threadId:)`; message row `UITestID.SubThread.messageRow(messageId:)`.
  - `SeedLoaderIntegrationTests.test_seedLoader_loadsEachFixtureWithExpectedRowCounts` is parametrized over fixtures with **expected row counts** — adding a fixture requires adding its expected counts there (Task LV-1 Step 4).

---

## File Structure

**New files:**
- `ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-07-a2a-burst.json`
- `ios/AINMMessenger/AINMMessengerUITests/A2ACapsuleUITests.swift`
- `docs/runbooks/2026-06-08-a2a-arc-verify.md`

**Modified files:**
- `ios/AINMMessenger/AINMMessenger/App/UITestIdentifiers.swift` — add `UITestID.A2ACapsule`.
- `ios/AINMMessenger/AINMMessenger/UI/MessageThread/A2ACapsuleView.swift` — apply the capsule identifier.
- `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/SeedLoaderIntegrationTests.swift` — add expected row counts for the new fixture (only if it enumerates fixtures with a counts table — verify in LV-1 Step 4).

---

### Task LV-1: Author the multi-agent A2A burst fixture

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-07-a2a-burst.json`
- Possibly modify: `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/SeedLoaderIntegrationTests.swift`

**Design of the thread** (exercises the whole arc):
- 3 participants: `uat-user@localhost` (human/creator), `agent-alpha@localhost` (agent, local), `analyst@acme.test` (agent, **foreign host** → provenance suffix).
- Messages in order:
  1. `msg-01` from user — opens the thread (boundary).
  2. `msg-a2a-01` from `agent-alpha@localhost` — A2A (no mention → undirected).
  3. `msg-a2a-02` from `analyst@acme.test` — A2A, **mentions `agent-alpha@localhost`** (→ directional tag in expanded view; foreign sender → `· acme.test` provenance).
  4. `msg-a2a-03` from `agent-alpha@localhost` — A2A (undirected).
  5. `msg-04` from `agent-alpha@localhost` — **mentions `uat-user@localhost`** → directed at the human → NOT A2A → **bounds** the burst and renders **inline** (the "report back").
- Result: messages 2–4 are a bounded run of 3 A2A messages across 2 distinct agents → **folds into one capsule** (`A2ACapsule.id == "msg-a2a-01"`); message 5 renders inline below it.

- [ ] **Step 1: Write the fixture** `scenario-07-a2a-burst.json`:

```json
{
  "version": "1.0",
  "scenario": "scenario-07-a2a-burst",
  "identity": "uat-user@localhost",
  "threads": [
    {
      "id": "thread-01",
      "title": "Launch readiness",
      "createdAt": "2026-04-18T10:00:00.000Z",
      "createdBy": "uat-user@localhost",
      "parentThreadId": null,
      "parentMessageId": null,
      "hasUnread": false
    }
  ],
  "participants": [
    { "threadId": "thread-01", "identity": "uat-user@localhost", "type": "human", "joinedAt": "2026-04-18T10:00:00.000Z", "role": "creator", "displayName": "UAT User" },
    { "threadId": "thread-01", "identity": "agent-alpha@localhost", "type": "agent", "joinedAt": "2026-04-18T10:00:00.000Z", "role": "member", "displayName": "Alpha" },
    { "threadId": "thread-01", "identity": "analyst@acme.test", "type": "agent", "joinedAt": "2026-04-18T10:00:00.000Z", "role": "member", "displayName": "Analyst" }
  ],
  "messages": [
    {
      "id": "msg-01",
      "threadId": "thread-01",
      "fromAddress": "uat-user@localhost",
      "timestamp": "2026-04-18T10:00:01.000Z",
      "type": "message",
      "contentJson": "[{\"type\":\"text\",\"version\":\"1.0\",\"source\":\"user\",\"text\":\"Are we ready to launch?\",\"fallback\":\"Are we ready to launch?\"}]",
      "signature": "USER01",
      "serverTimestamp": null
    },
    {
      "id": "msg-a2a-01",
      "threadId": "thread-01",
      "fromAddress": "agent-alpha@localhost",
      "timestamp": "2026-04-18T10:00:05.000Z",
      "type": "message",
      "contentJson": "[{\"type\":\"text\",\"version\":\"1.0\",\"source\":\"agent\",\"text\":\"Let me confirm the p99 numbers with the analyst.\",\"fallback\":\"Let me confirm the p99 numbers with the analyst.\"}]",
      "signature": "ALPHA01",
      "serverTimestamp": "2026-04-18T10:00:05.100Z"
    },
    {
      "id": "msg-a2a-02",
      "threadId": "thread-01",
      "fromAddress": "analyst@acme.test",
      "timestamp": "2026-04-18T10:00:10.000Z",
      "type": "message",
      "contentJson": "[{\"type\":\"text\",\"version\":\"1.0\",\"source\":\"agent\",\"text\":\"Alpha p99 is 142ms, down 18%.\",\"mentions\":[{\"address\":\"agent-alpha@localhost\",\"offset\":0,\"length\":5}],\"fallback\":\"Alpha p99 is 142ms, down 18%.\"}]",
      "signature": "ANALYST01",
      "serverTimestamp": "2026-04-18T10:00:10.100Z"
    },
    {
      "id": "msg-a2a-03",
      "threadId": "thread-01",
      "fromAddress": "agent-alpha@localhost",
      "timestamp": "2026-04-18T10:00:15.000Z",
      "type": "message",
      "contentJson": "[{\"type\":\"text\",\"version\":\"1.0\",\"source\":\"agent\",\"text\":\"Good — that clears the launch gate.\",\"fallback\":\"Good — that clears the launch gate.\"}]",
      "signature": "ALPHA02",
      "serverTimestamp": "2026-04-18T10:00:15.100Z"
    },
    {
      "id": "msg-04",
      "threadId": "thread-01",
      "fromAddress": "agent-alpha@localhost",
      "timestamp": "2026-04-18T10:00:20.000Z",
      "type": "message",
      "contentJson": "[{\"type\":\"text\",\"version\":\"1.0\",\"source\":\"agent\",\"text\":\"You're clear to launch — p99 is well within budget.\",\"mentions\":[{\"address\":\"uat-user@localhost\",\"offset\":0,\"length\":3}],\"fallback\":\"You're clear to launch — p99 is well within budget.\"}]",
      "signature": "ALPHA03",
      "serverTimestamp": "2026-04-18T10:00:20.100Z"
    }
  ],
  "toolCalls": [],
  "archivalFacts": [],
  "agentCards": [],
  "savedAgents": []
}
```

> Note: keep the top-level keys exactly matching the other `scenario-*.json` files. If `ScenarioFixture` does not declare some of `toolCalls`/`archivalFacts`/`agentCards`/`savedAgents`, omit the ones it doesn't decode (open `scenario-02-mentions.json` and `ScenarioFixture.swift` and match its key set exactly — add only keys the decoder accepts; a stray key will fail decoding).

- [ ] **Step 2: Verify the fixture decodes + seeds.** Run the seed-loader integration test:
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/SeedLoaderIntegrationTests
```

- [ ] **Step 3: If that test FAILS** because it enumerates fixtures with an expected-row-count table that doesn't include `scenario-07-a2a-burst`, open `SeedLoaderIntegrationTests.swift`, find the expected-counts table, and add an entry for `scenario-07-a2a-burst`: threads = 1, participants = 3, messages = 5 (match the table's exact shape/keys; if it also counts toolCalls/archivalFacts/agentCards/savedAgents, those are all 0). Re-run until green.

- [ ] **Step 4: Verify capsule folding against the seed via a tiny unit assertion.** Confirm the seeded data actually folds (don't rely only on visual). Add a focused test to a NEW file `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/A2AFixtureFoldTests.swift`:

```swift
import XCTest
import GRDB
@testable import AINMMessenger

@MainActor
final class A2AFixtureFoldTests: XCTestCase {
    func test_scenario07_foldsIntoOneCapsule_withInlineReportBack() async throws {
        let db = try AppDatabase.makeInMemory()
        // Seed the fixture directly via the SeedLoader (same path the app uses).
        try SeedLoader.loadFixture(named: "scenario-07-a2a-burst", into: db)

        let vm = MessageThreadViewModel(threadId: "thread-01",
                                        currentUserIdentity: "uat-user@localhost",
                                        database: db)
        await vm.loadInitial()

        XCTAssertTrue(vm.isMultiParticipant)
        let capsules: [A2ACapsule] = vm.threadItems.compactMap {
            if case .capsule(let c) = $0 { return c } else { return nil }
        }
        XCTAssertEqual(capsules.count, 1, "the bounded 3-message 2-agent burst should fold")
        XCTAssertEqual(capsules.first?.messageCount, 3)
        XCTAssertEqual(Set(capsules.first?.agents ?? []), ["agent-alpha@localhost", "analyst@acme.test"])
        // msg-04 is directed at the human → inline, not in the capsule.
        let groupItems = vm.threadItems.filter { if case .group = $0 { return true } else { return false } }
        XCTAssertGreaterThanOrEqual(groupItems.count, 2) // user msg + inline report-back
    }
}
```

> **VERIFY the SeedLoader API first:** open `Testing/SeedLoader.swift` and use its actual loading entry point. The launch path is `SeedLoader.loadFixtureIfRequested()` reading from `Bundle.main`; there may or may not be a direct `loadFixture(named:into:)`. If no direct-by-name API exists, either (a) add a small internal `static func loadFixture(named:into:)` that mirrors the existing decode+insert logic (preferred — also makes seeding testable), or (b) decode the bundled JSON in the test and insert via the existing insert helper. Pick whichever matches the real code; keep the assertions above. Run:
```
xcodebuild ... test -only-testing:AINMMessengerTests/A2AFixtureFoldTests
```
Expected: PASS. Revert any pbxproj rewrite.

- [ ] **Step 5: Commit**
```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-07-a2a-burst.json \
  ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/A2AFixtureFoldTests.swift
# include SeedLoaderIntegrationTests.swift and/or SeedLoader.swift if you modified them
rtk proxy git add -A ios/AINMMessenger/AINMMessengerTests/PersistenceTests/SeedLoaderIntegrationTests.swift 2>/dev/null || true
rtk proxy git commit -m "test(loop): scenario-07 multi-agent A2A burst fixture + fold assertion"
```

---

### Task LV-2: Capsule UITest identifier

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/App/UITestIdentifiers.swift`
- Modify: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/A2ACapsuleView.swift`

- [ ] **Step 1: Add the identifier enum.** In `UITestIdentifiers.swift`, add a new nested enum alongside the others:

```swift
    // MARK: - SP4: A2A digest capsule
    enum A2ACapsule {
        static func capsule(capsuleId: String) -> String { "thread.a2aCapsule.\(capsuleId)" }
    }
```

- [ ] **Step 2: Apply it to the capsule.** In `A2ACapsuleView.swift`, on the outer `VStack` (the one that already has `.accessibilityElement(...)`/`.accessibilityLabel(...)`), add — after the existing accessibility modifiers:

```swift
        .accessibilityIdentifier(UITestID.A2ACapsule.capsule(capsuleId: capsule.id))
```

(Placing it on the same view that carries `.accessibilityElement(children: .ignore)` makes the collapsed card a single addressable element; tapping it hits the header button beneath and toggles expansion — the same pattern `SubThreadBannerView` uses.)

- [ ] **Step 3: Build + re-run the capsule render suite** (identifier must not break rendering):
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/A2ACapsuleRenderTests
```
Expected: PASS (4 tests). Revert any pbxproj rewrite.

- [ ] **Step 4: Commit**
```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/App/UITestIdentifiers.swift \
  ios/AINMMessenger/AINMMessenger/UI/MessageThread/A2ACapsuleView.swift
rtk proxy git commit -m "test(loop): UITest accessibility identifier on A2A capsule"
```

---

### Task LV-3: `A2ACapsuleUITests` — seed, assert, expand, screenshot

**Files:**
- Create: `ios/AINMMessenger/AINMMessengerUITests/A2ACapsuleUITests.swift`

- [ ] **Step 1: Write the UITest.** It subclasses the existing base (`AINMUITestCase`) and uses its `launchWith`/`openThread` helpers:

```swift
import XCTest

/// Live render/behavior verification of the A2A arc against the seeded multi-agent thread
/// (scenario-07-a2a-burst): the bounded 3-message / 2-agent burst folds into one capsule,
/// which expands in place to reveal the inner agent messages and collapses again.
final class A2ACapsuleUITests: AINMUITestCase {

    private func captureScreenshot(_ name: String) {
        let shot = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: shot)
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    func test_a2aBurst_collapsesToCapsule_expandsAndCollapses() {
        launchWith(seedFixture: "scenario-07-a2a-burst")
        _ = openThread(threadId: "thread-01")

        // Collapsed capsule present (id == first A2A message id).
        let capsuleId = UITestID.A2ACapsule.capsule(capsuleId: "msg-a2a-01")
        let capsule = app.descendants(matching: .any)[capsuleId].firstMatch
        XCTAssertTrue(capsule.waitForExistence(timeout: 15),
                      "A2A burst did not fold into a capsule (\(capsuleId))")
        captureScreenshot("a2a-capsule-collapsed")

        // The inline report-back (msg-04, directed at the human) is NOT folded — visible alongside.
        let reportBack = app.descendants(matching: .any)[UITestID.SubThread.messageRow(messageId: "msg-04")].firstMatch
        XCTAssertTrue(reportBack.waitForExistence(timeout: 5),
                      "Inline report-back message should render outside the capsule")

        // Inner A2A messages are hidden while collapsed.
        let innerCollapsed = app.descendants(matching: .any)[UITestID.SubThread.messageRow(messageId: "msg-a2a-02")].firstMatch
        XCTAssertFalse(innerCollapsed.exists, "Inner A2A messages should be hidden while collapsed")

        // Expand.
        capsule.tap()
        let inner = app.descendants(matching: .any)[UITestID.SubThread.messageRow(messageId: "msg-a2a-02")].firstMatch
        XCTAssertTrue(inner.waitForExistence(timeout: 5),
                      "Inner A2A message not visible after expanding the capsule")
        captureScreenshot("a2a-capsule-expanded")

        // Collapse again (tap the capsule header — still addressable by id).
        let header = app.descendants(matching: .any)[capsuleId].firstMatch
        if header.exists { header.tap() }
        XCTAssertTrue(app.descendants(matching: .any)[UITestID.SubThread.messageRow(messageId: "msg-a2a-02")]
                        .firstMatch.waitForNonExistence(timeout: 5),
                      "Inner A2A message should hide again after collapse")
    }
}
```

> Notes for the implementer: (1) `AINMUITestCase` is the existing base class — confirm its exact name (grep `class AINMUITestCase` / how `MemoryContextUITests` subclasses) and match it; if `openThread` returns `Bool`, the `_ =` discard is correct. (2) `UITestID` is in the app target — UITests already reference it (e.g. `UITestID.Memory.toolbarPill`), so it's importable as written. (3) `waitForNonExistence(timeout:)` exists on `XCUIElement` (XCTest); if the deployment target lacks it, replace with an `expectation(for: NSPredicate(format: "exists == false") ...)` wait. (4) If `app.descendants(matching:.any)[id]` is flaky for the collapsed element, fall back to `app.otherElements[id]` / `app.buttons[id]` as `MemoryContextUITests` does.

- [ ] **Step 2: Run the new UITest**
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessengerUITests \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerUITests/A2ACapsuleUITests
```
Expected: PASS. If it fails on element lookup, iterate per the notes (this is the one task where the matcher may need adjustment to the real accessibility tree — adjust the matcher, not the assertions' intent). Revert any pbxproj rewrite.

- [ ] **Step 3: Commit**
```bash
rtk proxy git add ios/AINMMessenger/AINMMessengerUITests/A2ACapsuleUITests.swift
rtk proxy git commit -m "test(loop): A2ACapsuleUITests — seed multi-agent thread, assert fold/expand/collapse + screenshots"
```

---

### Task LV-4: Human runbook + full suites + DoD

**Files:**
- Create: `docs/runbooks/2026-06-08-a2a-arc-verify.md`

- [ ] **Step 1: Write the runbook** `docs/runbooks/2026-06-08-a2a-arc-verify.md`:

```markdown
# Runbook — A2A Arc Visual Verification (2026-06-08)

Verifies the multi-agent / A2A UI arc (SP3 + participant roster + SP4 capsule) renders correctly.

> **Note:** the server runtime does not generate agent-to-agent traffic today (human ↔ single
> agent only). This runbook verifies the *UI* against a seeded multi-agent thread through the real
> app. Real runtime A2A generation is separate future work.

## A. Automated (CI-able)

Runs the seeded UITest and captures collapsed/expanded screenshots as test attachments.

```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessengerUITests \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerUITests/A2ACapsuleUITests
```
Open the `.xcresult` (Xcode → Report navigator, or `xcrun xcresulttool`) to view the
`a2a-capsule-collapsed` / `a2a-capsule-expanded` screenshots.

## B. Manual visual check (eyeball the design)

1. In Xcode, edit the **AINMMessenger** scheme → Run → Arguments → Arguments Passed On Launch,
   add: `-UITestSeed` and `scenario-07-a2a-burst` (two separate entries).
2. Run the app on the iPhone 16 simulator. It opens already "connected" as `uat-user@localhost`.
3. Open the **Launch readiness** conversation.
4. Confirm, in order (light mode, then re-run in dark mode):
   - [ ] A **collapsed capsule** sits between your message and the final agent reply:
         overlapped avatars + `Alpha ↔ Analyst` (each in its own identity color) + `3 messages`.
   - [ ] The final agent message ("You're clear to launch…") renders **inline** below the capsule
         (it's directed at you, so it is NOT folded).
   - [ ] Tap the capsule → it **expands in place** to the three agent messages, each with its
         identity-tinted serif name; **`Analyst`** shows a dim **`· acme.test`** provenance suffix
         (foreign host); the analyst's message shows a directional **`→ Alpha`** tag.
   - [ ] Tap **Collapse** → it folds back to the digest card.
   - [ ] The thread header shows **3 participants**.
   - [ ] Dark mode: all of the above renders with correct dark tokens (no white-on-white).
```

- [ ] **Step 2: Full unit suite**
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test
```
Expected: entire `AINMMessengerTests` green (incl. the new `A2AFixtureFoldTests` and any updated `SeedLoaderIntegrationTests`).

- [ ] **Step 3: Full UITest suite**
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessengerUITests \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test
```
Expected: green (incl. `A2ACapsuleUITests`).

- [ ] **Step 4: DoD audit (report each PASS/FAIL with evidence)**
1. `scenario-07-a2a-burst.json` seeds a human + 2 agents (one foreign-host) + a bounded 3-message 2-agent burst + an inline report-back (LV-1).
2. `A2AFixtureFoldTests` proves the seed folds to exactly one capsule (3 msgs, 2 agents) with the report-back inline (LV-1).
3. Capsule has a UITest identifier; render suite still green (LV-2).
4. `A2ACapsuleUITests` seeds the thread, asserts collapsed capsule + inline report-back, expands → inner messages visible, collapses → hidden; screenshots attached (LV-3).
5. Runbook documents both the automated path and the manual visual checklist incl. tinting, directional tag, `· acme.test` provenance, capsule digest, expand/collapse, 3 participants, dark mode (LV-4).
6. Scope reality documented: runtime A2A generation is out of scope / future work (LV-4 runbook + this plan's header).
7. Full unit + UITest suites green; clean build; committed to `main`.

- [ ] **Step 5: Commit**
```bash
rtk proxy git add docs/runbooks/2026-06-08-a2a-arc-verify.md
rtk proxy git commit -m "docs(loop): A2A arc verification runbook (automated UITest + manual visual checklist)"
```

---

## Self-Review

**Coverage:** LV-1 seeds realistic multi-agent data + proves it folds (unit); LV-2 makes the capsule UITest-addressable; LV-3 verifies fold/expand/collapse behavior live in the running app + screenshots; LV-4 gives a human the visual checklist and runs full suites. The arc's pieces are each exercised: roster (`isMultiParticipant` + 3 participants), tinting (multi-participant names), directional tag (mention in `msg-a2a-02`), provenance (`analyst@acme.test` foreign host), capsule fold + expand/collapse, inline boundary (`msg-04` to the human). ✓

**Honest scope:** the plan's header and the runbook both state plainly that runtime A2A generation does not exist and is out of scope — this verifies UI/rendering through real code paths against seeded data, not runtime-generated A2A. No overstatement of "live." ✓

**Type/identifier consistency:** `UITestID.A2ACapsule.capsule(capsuleId:)` defined in LV-2, applied in LV-2, referenced in LV-3 with `capsuleId: "msg-a2a-01"` (= `A2ACapsule.id` = first A2A group id = first A2A message id). `UITestID.SubThread.messageRow(messageId:)` reused for inner/inline messages (existing). ✓

**Risk flagged inline:** LV-1 Step 4 (SeedLoader API may need a by-name helper) and LV-3 notes (UITest matcher / `waitForNonExistence` may need adjustment to the real accessibility tree) are the two places reality may differ from the verbatim code — each has a concrete fallback, and the assertions' intent is fixed.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-08-a2a-arc-live-verify.md`. Per the established pattern, proceeding with **Subagent-Driven execution** (superpowers:subagent-driven-development): a fresh implementer subagent per task (LV-1…LV-4), build/test verification between tasks, commits to `main` via `rtk proxy git`. Pushing to origin remains a separate, user-gated step after the plan completes. The manual visual checklist in the runbook (LV-4 §B) is a human step you run when you want to eyeball the design.
```
