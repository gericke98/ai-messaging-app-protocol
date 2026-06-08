# A2A & Multi-Participant Expression (SP3) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make multi-participant / agent-to-agent threads legible in the AINM iOS client — identity-tinted names, directional `→ @name` bylines, and foreign-host `· host` provenance — using the existing Editorial Warm tokens, in light + dark.

**Architecture:** Persist the envelope `to` field on `MessageRecord` (new GRDB migration). Add small **pure** helpers (`ParticipantProvenance`, `MentionTarget`, and a `MessageGroup.deriveDirectedTo` static) that are unit-testable without a host. Introduce one shared SwiftUI view, `ParticipantByline`, used by both `AgentMessageView` (settled) and `StreamingMessageView` (live) so the two can't drift. All additions are display-only — no network, runtime, or server change.

**Tech Stack:** Swift 6 / SwiftUI (iOS 18+), GRDB 7.x, XCTest. Build/test via `xcodebuild`. iOS project at `ios/AINMMessenger`.

---

## Conventions (read once before starting)

- **Git:** ALL git operations use `rtk proxy git ...` (never bare `git`). Commit directly to `main` (project's established pattern — no feature branch). Do **not** push to origin; that is a separate user-gated step after the whole plan completes.
- **Build (unit tests):**
  ```
  xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj \
    -scheme AINMMessenger \
    -destination 'platform=iOS Simulator,name=iPhone 16' \
    -derivedDataPath /tmp/ainm-dd test
  ```
  Scope a single class with `-only-testing:AINMMessengerTests/<ClassName>`.
- **pbxproj hygiene:** New `.swift` files under `AINMMessenger/` and `AINMMessengerTests/` auto-include via the synchronized folder group — **zero** pbxproj edits needed. After any `xcodebuild` run, if `project.pbxproj` or any `.xcscheme` was rewritten by Xcode, revert it: `rtk proxy git checkout -- ios/AINMMessenger/AINMMessenger.xcodeproj/project.pbxproj` (and any `*.xcscheme`). Never commit those auto-rewrites.
- **No snapshot library:** Adding an SPM dependency normalizes the hand-authored pbxproj (confirmed in SP2/CSP-2). Visual coverage stays the **dependency-free `UIHostingController` render/smoke harness** already in `AINMMessengerTests/DesignSystemTests/ConversationSurfaceRenderTests.swift`.
- **SourceKit noise:** "Cannot find 'Theme' / 'AvatarView' / 'ParticipantColor' in scope" and "No such module 'XCTest'" from the editor are stale-index false positives. Trust only `xcodebuild` results.
- **Existing tokens (do not redefine):** `Theme.Typography.senderName()` (serif semibold subheadline), `Theme.Typography.caption()` (caption2), `Theme.Color.inkPrimary`, `Theme.Color.inkSecondary`, `Theme.Spacing.{xs,sm,md,lg}`, `ParticipantColor.color(for:)`, `ParticipantColor.initials(for:)`.

---

## File Structure

**New files:**
- `ios/AINMMessenger/AINMMessenger/UI/Shared/ParticipantProvenance.swift` — pure host helpers.
- `ios/AINMMessenger/AINMMessenger/UI/Shared/MentionTarget.swift` — pure first-mention extraction.
- `ios/AINMMessenger/AINMMessenger/UI/Shared/ParticipantByline.swift` — the shared byline view.
- `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/ParticipantProvenanceTests.swift`
- `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/MentionTargetTests.swift`
- `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/MessageGroupDirectedToTests.swift`
- `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/MessageToAddressTests.swift`
- `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/ParticipantBylineRenderTests.swift`

**Modified files:**
- `ios/AINMMessenger/AINMMessenger/Persistence/AppDatabase.swift` — register migration `v9_add_message_toAddress`.
- `ios/AINMMessenger/AINMMessenger/Persistence/MessageRecord.swift` — `+ var toAddress: String?`.
- `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadViewModel.swift` — `MessageGroup.directedTo`, `deriveDirectedTo`, `isMultiParticipant`.
- `ios/AINMMessenger/AINMMessenger/UI/MessageThread/AgentMessageView.swift` — header uses `ParticipantByline`.
- `ios/AINMMessenger/AINMMessenger/UI/MessageThread/StreamingMessageView.swift` — name row uses `ParticipantByline` (name tint + provenance only).
- `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageGroupView.swift` — thread `isMultiParticipant` / `homeHost` passed down.
- `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift` — compute `homeHost`; pass into groups + streams; tokenize memory-context pill.
- `ios/AINMMessenger/AINMMessenger/Persistence/ThreadRepository.swift` — `ConversationRow.agentAddress`.
- `ios/AINMMessenger/AINMMessenger/UI/ConversationList/ConversationRowView.swift` — `rowAddress` = thread's primary agent address.

---

### Task SP3-1: Persist `toAddress` (migration v9 + MessageRecord)

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/Persistence/AppDatabase.swift` (after the `v8_create_saved_agents` migration block, before `try migrator.migrate(dbPool)`)
- Modify: `ios/AINMMessenger/AINMMessenger/Persistence/MessageRecord.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/MessageToAddressTests.swift` (create)

- [ ] **Step 1: Write the failing test**

Create `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/MessageToAddressTests.swift`:

```swift
import XCTest
import GRDB
@testable import AINMMessenger

final class MessageToAddressTests: XCTestCase {
    var db: AppDatabase!

    override func setUp() async throws {
        db = try AppDatabase.makeInMemory()
    }

    /// init(from envelope:) copies the envelope `to` field.
    func testInitFromEnvelopeCopiesTo() throws {
        let env = MessageEnvelope(
            id: "m1", from: "assistant@localhost", threadId: "t1",
            timestamp: "2026-06-08T09:41:00.000Z", type: .message,
            content: [.text(.init(version: "1.0", source: "agent", text: "hi", mentions: nil, fallback: "hi"))],
            signature: "sig", parentId: nil, to: "researcher@acme.test"
        )
        let record = try MessageRecord(from: env)
        XCTAssertEqual(record.toAddress, "researcher@acme.test")
    }

    /// A record with a toAddress round-trips through the DB.
    func testToAddressRoundTrips() throws {
        let record = MessageRecord(
            id: "m2", threadId: "t1", fromAddress: "assistant@localhost",
            timestamp: "2026-06-08T09:41:00.000Z", type: "message",
            contentJson: "[]", signature: "sig", serverTimestamp: nil,
            toAddress: "researcher@acme.test"
        )
        try db.dbPool.write { db in try record.insert(db) }
        let fetched = try db.dbPool.read { db in try MessageRecord.fetchOne(db, key: "m2") }
        XCTAssertEqual(fetched?.toAddress, "researcher@acme.test")
    }

    /// A record without a toAddress reads back nil (existing-row behavior).
    func testMissingToAddressReadsNil() throws {
        let record = MessageRecord(
            id: "m3", threadId: "t1", fromAddress: "alice@localhost",
            timestamp: "2026-06-08T09:42:00.000Z", type: "message",
            contentJson: "[]", signature: "sig", serverTimestamp: nil
        )
        try db.dbPool.write { db in try record.insert(db) }
        let fetched = try db.dbPool.read { db in try MessageRecord.fetchOne(db, key: "m3") }
        XCTAssertNil(fetched?.toAddress)
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/MessageToAddressTests
```
Expected: FAIL — compile error, `MessageRecord` has no `toAddress` argument/property.

- [ ] **Step 3: Add the migration**

In `AppDatabase.swift`, immediately after the `migrator.registerMigration("v8_create_saved_agents") { ... }` block and before `try migrator.migrate(dbPool)`, add:

```swift
        migrator.registerMigration("v9_add_message_toAddress") { db in
            try db.alter(table: "message") { t in
                t.add(column: "toAddress", .text)   // nullable; nil = no explicit recipient
            }
        }
```

- [ ] **Step 4: Add the property to `MessageRecord`**

In `MessageRecord.swift`, add the stored property after `var serverTimestamp: String?` (line 18):

```swift
    var serverTimestamp: String?
    var toAddress: String?     // explicit recipient from envelope.to; nil = none
```

In `init(from envelope:)`, after `self.serverTimestamp = nil`, add:

```swift
        self.serverTimestamp = nil
        self.toAddress = envelope.to
```

In the memberwise initializer, add `toAddress` as a defaulted trailing parameter so existing call sites compile unchanged. Change the signature's last parameter and body:

```swift
    init(
        id: String,
        threadId: String,
        fromAddress: String,
        timestamp: String,
        type: String,
        contentJson: String,
        signature: String,
        serverTimestamp: String?,
        toAddress: String? = nil
    ) {
        self.id = id
        self.threadId = threadId
        self.fromAddress = fromAddress
        self.timestamp = timestamp
        self.type = type
        self.contentJson = contentJson
        self.signature = signature
        self.serverTimestamp = serverTimestamp
        self.toAddress = toAddress
    }
```

- [ ] **Step 5: Run the test to verify it passes**

Run the same command as Step 2. Expected: PASS (3 tests). Revert any pbxproj/xcscheme rewrite (see Conventions).

- [ ] **Step 6: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Persistence/AppDatabase.swift \
  ios/AINMMessenger/AINMMessenger/Persistence/MessageRecord.swift \
  ios/AINMMessenger/AINMMessengerTests/PersistenceTests/MessageToAddressTests.swift
rtk proxy git commit -m "feat(sp3): persist envelope to on message (migration v9 + MessageRecord.toAddress)"
```

---

### Task SP3-2: `ParticipantProvenance` pure helpers

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/UI/Shared/ParticipantProvenance.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/ParticipantProvenanceTests.swift` (create)

- [ ] **Step 1: Write the failing test**

Create `ParticipantProvenanceTests.swift`:

```swift
import XCTest
@testable import AINMMessenger

final class ParticipantProvenanceTests: XCTestCase {

    func testHomeHostExtractsHostPart() {
        XCTAssertEqual(ParticipantProvenance.homeHost(from: "alice@localhost"), "localhost")
        XCTAssertEqual(ParticipantProvenance.homeHost(from: "bob@acme.test"), "acme.test")
    }

    func testHomeHostNilWhenNoAtOrEmptyHost() {
        XCTAssertNil(ParticipantProvenance.homeHost(from: "noatsign"))
        XCTAssertNil(ParticipantProvenance.homeHost(from: "trailing@"))
    }

    func testHostSuffixMatchesHomeHostLogic() {
        XCTAssertEqual(ParticipantProvenance.hostSuffix(of: "x@acme.test"), "acme.test")
        XCTAssertNil(ParticipantProvenance.hostSuffix(of: "x"))
    }

    func testIsForeignHostTrueWhenHostsDiffer() {
        XCTAssertTrue(ParticipantProvenance.isForeignHost("bob@acme.test", homeHost: "localhost"))
    }

    func testIsForeignHostFalseWhenSameHost() {
        XCTAssertFalse(ParticipantProvenance.isForeignHost("bob@localhost", homeHost: "localhost"))
    }

    func testIsForeignHostFalseWhenHomeHostNil() {
        XCTAssertFalse(ParticipantProvenance.isForeignHost("bob@acme.test", homeHost: nil))
    }

    func testIsForeignHostFalseWhenAddressHasNoHost() {
        XCTAssertFalse(ParticipantProvenance.isForeignHost("bob", homeHost: "localhost"))
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/ParticipantProvenanceTests
```
Expected: FAIL — `ParticipantProvenance` is undefined.

- [ ] **Step 3: Write the implementation**

Create `ios/AINMMessenger/AINMMessenger/UI/Shared/ParticipantProvenance.swift`:

```swift
import Foundation

/// Pure helpers for federation provenance: extracting and comparing the host
/// part of an AINM address (`name@host`). No host/UI dependencies — unit-testable.
enum ParticipantProvenance {

    /// Host part (after `@`) of an address, or nil if there's no `@` or the host is empty.
    static func homeHost(from address: String) -> String? {
        guard let at = address.firstIndex(of: "@") else { return nil }
        let host = String(address[address.index(after: at)...])
        return host.isEmpty ? nil : host
    }

    /// Host substring of an address for display. Same rule as `homeHost(from:)`.
    static func hostSuffix(of address: String) -> String? {
        homeHost(from: address)
    }

    /// True when `address`'s host differs from `homeHost`. False when `homeHost`
    /// is nil (not connected → treat all as local) or the address has no host.
    static func isForeignHost(_ address: String, homeHost: String?) -> Bool {
        guard let homeHost else { return false }
        guard let host = hostSuffix(of: address) else { return false }
        return host != homeHost
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run the Step 2 command. Expected: PASS (7 tests). Revert any pbxproj rewrite.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/Shared/ParticipantProvenance.swift \
  ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/ParticipantProvenanceTests.swift
rtk proxy git commit -m "feat(sp3): ParticipantProvenance pure host helpers + tests"
```

---

### Task SP3-3: `MentionTarget` pure first-mention extraction

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/UI/Shared/MentionTarget.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/MentionTargetTests.swift` (create)

- [ ] **Step 1: Write the failing test**

Create `MentionTargetTests.swift`:

```swift
import XCTest
@testable import AINMMessenger

final class MentionTargetTests: XCTestCase {

    private func textBlock(_ s: String, mentions: [ContentBlock.Mention]?) -> ContentBlock {
        .text(.init(version: "1.0", source: "agent", text: s, mentions: mentions, fallback: s))
    }

    func testReturnsFirstMentionAddress() {
        let blocks = [textBlock("hey @researcher", mentions: [
            .init(address: "researcher@acme.test", offset: 4, length: 11)
        ])]
        XCTAssertEqual(MentionTarget.firstMentionAddress(in: blocks), "researcher@acme.test")
    }

    func testReturnsLowestOffsetMentionWithinABlock() {
        let blocks = [textBlock("@b and @a", mentions: [
            .init(address: "a@host", offset: 7, length: 2),
            .init(address: "b@host", offset: 0, length: 2)
        ])]
        XCTAssertEqual(MentionTarget.firstMentionAddress(in: blocks), "b@host")
    }

    func testNilWhenNoMentions() {
        XCTAssertNil(MentionTarget.firstMentionAddress(in: [textBlock("plain", mentions: nil)]))
        XCTAssertNil(MentionTarget.firstMentionAddress(in: [textBlock("empty", mentions: [])]))
    }

    func testScansToFirstBlockThatHasMentions() {
        let blocks = [
            textBlock("no mentions here", mentions: nil),
            textBlock("now @x", mentions: [.init(address: "x@host", offset: 4, length: 2)])
        ]
        XCTAssertEqual(MentionTarget.firstMentionAddress(in: blocks), "x@host")
    }

    func testNilForNonTextBlocks() {
        let code = ContentBlock.code(.init(version: "1.0", source: "agent", language: "swift",
                                           code: "let x = 1", filename: nil, fallback: "code"))
        XCTAssertNil(MentionTarget.firstMentionAddress(in: [code]))
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/MentionTargetTests
```
Expected: FAIL — `MentionTarget` is undefined.

- [ ] **Step 3: Write the implementation**

Create `ios/AINMMessenger/AINMMessenger/UI/Shared/MentionTarget.swift`:

```swift
import Foundation

/// Pure helper: extract the first mentioned address from a message's content blocks.
/// "First" = the lowest-offset mention in the first text block that has any mention.
enum MentionTarget {

    static func firstMentionAddress(in blocks: [ContentBlock]) -> String? {
        for block in blocks {
            if case .text(let content) = block,
               let mentions = content.mentions, !mentions.isEmpty {
                return mentions.min(by: { $0.offset < $1.offset })?.address
            }
        }
        return nil
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run the Step 2 command. Expected: PASS (5 tests). Revert any pbxproj rewrite.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/Shared/MentionTarget.swift \
  ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/MentionTargetTests.swift
rtk proxy git commit -m "feat(sp3): MentionTarget first-mention extraction + tests"
```

---

### Task SP3-4: `MessageGroup.directedTo` + `isMultiParticipant`

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadViewModel.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/MessageGroupDirectedToTests.swift` (create)

- [ ] **Step 1: Write the failing test**

Create `MessageGroupDirectedToTests.swift`. It exercises the **pure** static derivation directly (no view model / DB needed):

```swift
import XCTest
@testable import AINMMessenger

final class MessageGroupDirectedToTests: XCTestCase {

    private func mentionBlocks(_ address: String) -> [ContentBlock] {
        [.text(.init(version: "1.0", source: "agent", text: "@x",
                     mentions: [.init(address: address, offset: 0, length: 2)], fallback: "@x"))]
    }
    private let plainBlocks: [ContentBlock] =
        [.text(.init(version: "1.0", source: "agent", text: "hello", mentions: nil, fallback: "hello"))]

    func testToAddressWins() {
        let result = MessageGroup.deriveDirectedTo(
            toAddress: "researcher@acme.test", contentBlocks: mentionBlocks("other@host"),
            senderAddress: "assistant@localhost", currentUserIdentity: "me@localhost"
        )
        XCTAssertEqual(result, "researcher@acme.test")
    }

    func testFallsBackToFirstMentionWhenToAddressNil() {
        let result = MessageGroup.deriveDirectedTo(
            toAddress: nil, contentBlocks: mentionBlocks("researcher@acme.test"),
            senderAddress: "assistant@localhost", currentUserIdentity: "me@localhost"
        )
        XCTAssertEqual(result, "researcher@acme.test")
    }

    func testEmptyToAddressFallsThroughToMention() {
        let result = MessageGroup.deriveDirectedTo(
            toAddress: "", contentBlocks: mentionBlocks("researcher@acme.test"),
            senderAddress: "assistant@localhost", currentUserIdentity: "me@localhost"
        )
        XCTAssertEqual(result, "researcher@acme.test")
    }

    func testNilWhenNoTargetAtAll() {
        let result = MessageGroup.deriveDirectedTo(
            toAddress: nil, contentBlocks: plainBlocks,
            senderAddress: "assistant@localhost", currentUserIdentity: "me@localhost"
        )
        XCTAssertNil(result)
    }

    func testSelfTargetSuppressed() {
        let result = MessageGroup.deriveDirectedTo(
            toAddress: "assistant@localhost", contentBlocks: plainBlocks,
            senderAddress: "assistant@localhost", currentUserIdentity: "me@localhost"
        )
        XCTAssertNil(result)
    }

    func testCurrentUserTargetSuppressed() {
        let result = MessageGroup.deriveDirectedTo(
            toAddress: "me@localhost", contentBlocks: plainBlocks,
            senderAddress: "assistant@localhost", currentUserIdentity: "me@localhost"
        )
        XCTAssertNil(result)
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/MessageGroupDirectedToTests
```
Expected: FAIL — `MessageGroup.deriveDirectedTo` and `MessageGroup.directedTo` are undefined.

- [ ] **Step 3: Add `directedTo` to the struct + the pure derivation**

In `MessageThreadViewModel.swift`, replace the `MessageGroup` struct (lines 6–13) with:

```swift
/// Group of consecutive messages from the same sender per D-03.
struct MessageGroup: Identifiable {
    let id: String       // first message ID
    let senderAddress: String
    let senderType: String  // "human" or "agent"
    let senderName: String  // display name portion of AINM address
    let messages: [MessageRecord]
    let isFromCurrentUser: Bool
    let directedTo: String?  // resolved A2A target address, or nil (no tag)

    /// Pure derivation of a directional target for a group's first message:
    /// `toAddress` (if non-empty) else the first mention; suppressed when the
    /// target is the sender itself or the current user (those are noise).
    static func deriveDirectedTo(
        toAddress: String?,
        contentBlocks: [ContentBlock],
        senderAddress: String,
        currentUserIdentity: String
    ) -> String? {
        let raw: String?
        if let to = toAddress, !to.isEmpty {
            raw = to
        } else {
            raw = MentionTarget.firstMentionAddress(in: contentBlocks)
        }
        guard let target = raw, !target.isEmpty else { return nil }
        if target == senderAddress || target == currentUserIdentity { return nil }
        return target
    }
}
```

- [ ] **Step 4: Populate `directedTo` in `makeGroup` and expose `isMultiParticipant`**

In `makeGroup(messages:sender:)` (lines 171–184), replace the body with:

```swift
    private func makeGroup(messages: [MessageRecord], sender: String) -> MessageGroup {
        let participant = participants[sender]
        let senderType = participant?.type ?? "human"
        // Extract display name from AINM address (part before @)
        let senderName = sender.components(separatedBy: "@").first ?? sender
        let firstMessage = messages[0]
        let directedTo = MessageGroup.deriveDirectedTo(
            toAddress: firstMessage.toAddress,
            contentBlocks: contentBlocks(for: firstMessage),
            senderAddress: sender,
            currentUserIdentity: currentUserIdentity
        )
        return MessageGroup(
            id: firstMessage.id,
            senderAddress: sender,
            senderType: senderType,
            senderName: senderName,
            messages: messages,
            isFromCurrentUser: sender == currentUserIdentity,
            directedTo: directedTo
        )
    }
```

Add a computed property near the top of the class, immediately after `var hasAgentParticipant: Bool = false` (line 23):

```swift
    var hasAgentParticipant: Bool = false

    /// True when the thread has more than one non-current-user participant — drives
    /// multi-agent name-tinting. 1:1 human↔agent threads stay calm (false).
    var isMultiParticipant: Bool {
        participants.values.filter { $0.identity != currentUserIdentity }.count > 1
    }
```

- [ ] **Step 5: Run the test to verify it passes**

Run the Step 2 command. Expected: PASS (6 tests). Then build the full app target to confirm no other `MessageGroup(...)` constructor call sites broke:
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build
```
Expected: BUILD SUCCEEDED. (If any other site constructs `MessageGroup` directly, add `directedTo: nil` there.) Revert any pbxproj rewrite.

- [ ] **Step 6: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadViewModel.swift \
  ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/MessageGroupDirectedToTests.swift
rtk proxy git commit -m "feat(sp3): MessageGroup.directedTo derivation + isMultiParticipant + tests"
```

---

### Task SP3-5: `ParticipantByline` shared view + render/smoke

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/UI/Shared/ParticipantByline.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/ParticipantBylineRenderTests.swift` (create)

- [ ] **Step 1: Write the failing render/smoke test**

Create `ParticipantBylineRenderTests.swift`. It reuses the same dependency-free hosting harness shape as `ConversationSurfaceRenderTests`:

```swift
import XCTest
import SwiftUI
@testable import AINMMessenger

@MainActor
final class ParticipantBylineRenderTests: XCTestCase {

    @discardableResult
    func render<V: View>(_ view: V, scheme: ColorScheme,
                         size: CGSize = CGSize(width: 390, height: 80)) -> CGSize {
        let host = UIHostingController(rootView: view.environment(\.colorScheme, scheme))
        host.overrideUserInterfaceStyle = (scheme == .dark) ? .dark : .light
        host.view.frame = CGRect(origin: .zero, size: size)
        let window = UIWindow(frame: host.view.frame)
        window.rootViewController = host
        window.isHidden = false
        host.view.setNeedsLayout()
        host.view.layoutIfNeeded()
        return host.view.bounds.size
    }

    func renderBothSchemes<V: View>(_ view: V, file: StaticString = #filePath, line: UInt = #line) {
        for scheme in [ColorScheme.light, .dark] {
            let s = render(view, scheme: scheme)
            XCTAssertGreaterThan(s.width, 0, "zero width in \(scheme)", file: file, line: line)
            XCTAssertGreaterThan(s.height, 0, "zero height in \(scheme)", file: file, line: line)
        }
    }

    func test_byline_calm1to1_noTagNoHost() {
        renderBothSchemes(ParticipantByline(
            senderAddress: "assistant@localhost", senderName: "Assistant",
            directedTo: nil, isMultiParticipant: false, homeHost: "localhost"))
    }

    func test_byline_multiAgent_identityTintedName() {
        renderBothSchemes(ParticipantByline(
            senderAddress: "researcher@localhost", senderName: "Researcher",
            directedTo: nil, isMultiParticipant: true, homeHost: "localhost"))
    }

    func test_byline_directional_targetTinted() {
        renderBothSchemes(ParticipantByline(
            senderAddress: "assistant@localhost", senderName: "Assistant",
            directedTo: "researcher@localhost", isMultiParticipant: true, homeHost: "localhost"))
    }

    func test_byline_foreignSender_hostSuffix() {
        renderBothSchemes(ParticipantByline(
            senderAddress: "analyst@acme.test", senderName: "Analyst",
            directedTo: nil, isMultiParticipant: true, homeHost: "localhost"))
    }

    func test_byline_foreignTarget_hostSuffix() {
        renderBothSchemes(ParticipantByline(
            senderAddress: "assistant@localhost", senderName: "Assistant",
            directedTo: "analyst@acme.test", isMultiParticipant: true, homeHost: "localhost"))
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/ParticipantBylineRenderTests
```
Expected: FAIL — `ParticipantByline` is undefined.

- [ ] **Step 3: Write the view**

Create `ios/AINMMessenger/AINMMessenger/UI/Shared/ParticipantByline.swift`:

```swift
import SwiftUI

/// Editorial Warm participant byline shared by settled (`AgentMessageView`) and live
/// (`StreamingMessageView`) agent messages so they can't drift. Renders:
///
///   [serif name]  [· host]?   →  [serif targetName] [· targetHost]?
///
/// - Name is identity-tinted when `isMultiParticipant`, else calm `inkPrimary`.
/// - `· host` suffixes appear only for foreign hosts (different from `homeHost`).
/// - The directional `→ target` tag appears only when `directedTo` is non-nil.
struct ParticipantByline: View {
    let senderAddress: String
    let senderName: String
    var directedTo: String? = nil
    var isMultiParticipant: Bool = false
    var homeHost: String? = nil

    var body: some View {
        HStack(spacing: Theme.Spacing.xs) {
            Text(senderName)
                .font(Theme.Typography.senderName())
                .foregroundStyle(senderColor)

            if let host = senderHostSuffix {
                Text("· \(host)")
                    .font(Theme.Typography.caption())
                    .foregroundStyle(Theme.Color.inkSecondary)
            }

            if let target = directedTo {
                Text("→")
                    .font(Theme.Typography.caption())
                    .foregroundStyle(Theme.Color.inkSecondary)
                Text(localPart(of: target))
                    .font(Theme.Typography.senderName())
                    .foregroundStyle(ParticipantColor.color(for: target))
                if let targetHost = hostSuffix(for: target) {
                    Text("· \(targetHost)")
                        .font(Theme.Typography.caption())
                        .foregroundStyle(Theme.Color.inkSecondary)
                }
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(accessibilityText)
    }

    // MARK: - Derivations

    private var senderColor: Color {
        isMultiParticipant ? ParticipantColor.color(for: senderAddress) : Theme.Color.inkPrimary
    }

    private var senderHostSuffix: String? {
        hostSuffix(for: senderAddress)
    }

    /// Foreign-only host suffix for any address (nil when local or hostless).
    private func hostSuffix(for address: String) -> String? {
        ParticipantProvenance.isForeignHost(address, homeHost: homeHost)
            ? ParticipantProvenance.hostSuffix(of: address)
            : nil
    }

    private func localPart(of address: String) -> String {
        address.components(separatedBy: "@").first ?? address
    }

    /// One spoken phrase: "Researcher, on acme.test, to Assistant, on home.test".
    private var accessibilityText: String {
        var parts = [senderName]
        if let host = senderHostSuffix { parts.append("on \(host)") }
        if let target = directedTo {
            parts.append("to \(localPart(of: target))")
            if let targetHost = hostSuffix(for: target) { parts.append("on \(targetHost)") }
        }
        return parts.joined(separator: ", ")
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run the Step 2 command. Expected: PASS (5 tests). Revert any pbxproj rewrite.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/Shared/ParticipantByline.swift \
  ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/ParticipantBylineRenderTests.swift
rtk proxy git commit -m "feat(sp3): ParticipantByline shared view + render/smoke (light+dark)"
```

---

### Task SP3-6: Wire `ParticipantByline` into the settled `AgentMessageView`

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/AgentMessageView.swift`
- Modify: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageGroupView.swift`
- Modify: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift`
- Test: extend `ParticipantBylineRenderTests.swift` (the existing `ConversationSurfaceRenderTests.test_agentMessage_rendersAiry` stays green; we add a directional-agent render below)

- [ ] **Step 1: Write the failing test**

Append to `ParticipantBylineRenderTests.swift` (inside the class):

```swift
    func test_agentMessage_directionalHeaderRenders() {
        let blocks: [ContentBlock] = [.text(.init(
            version: "1.0", source: "agent", text: "On it.", mentions: nil, fallback: "On it."))]
        let view = AgentMessageView(
            blocks: blocks, senderName: "Assistant", senderType: "agent",
            senderAddress: "assistant@localhost", showHeader: true, timestamp: "9:41 AM",
            directedTo: "researcher@acme.test", isMultiParticipant: true, homeHost: "localhost")
        renderBothSchemes(view)
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run:
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/ParticipantBylineRenderTests
```
Expected: FAIL — `AgentMessageView` has no `directedTo` / `isMultiParticipant` / `homeHost` arguments.

- [ ] **Step 3: Add params + use `ParticipantByline` in `AgentMessageView`**

In `AgentMessageView.swift`, add three properties after `var senderAddress: String? = nil` (line 10):

```swift
    var senderAddress: String? = nil
    var directedTo: String? = nil
    var isMultiParticipant: Bool = false
    var homeHost: String? = nil
```

Replace the header `HStack` (lines 18–28) with:

```swift
                HStack(spacing: Theme.Spacing.sm) {
                    AvatarView(
                        participantType: senderType,
                        size: 30,
                        address: senderAddress,
                        displayName: senderName
                    )
                    ParticipantByline(
                        senderAddress: senderAddress ?? senderName,
                        senderName: senderName,
                        directedTo: directedTo,
                        isMultiParticipant: isMultiParticipant,
                        homeHost: homeHost
                    )
                }
```

- [ ] **Step 4: Thread the params through `MessageGroupView`**

In `MessageGroupView.swift`, add two thread-level properties after `var onBranchThread: ((_ messageId: String) -> Void)? = nil` (line 17):

```swift
    var onBranchThread: ((_ messageId: String) -> Void)? = nil
    /// Thread-level: more than one non-current-user participant (drives name-tinting).
    var isMultiParticipant: Bool = false
    /// The current user's home host, for foreign-host provenance suffixes.
    var homeHost: String? = nil
```

In the `AgentMessageView(...)` call (lines 36–46), add the three byline params:

```swift
                        AgentMessageView(
                            blocks: blocks,
                            senderName: group.senderName,
                            senderType: group.senderType,
                            senderAddress: group.senderAddress,
                            showHeader: isFirst,
                            timestamp: isLast ? timestampFormatter(message.timestamp) : nil,
                            directedTo: group.directedTo,
                            isMultiParticipant: isMultiParticipant,
                            homeHost: homeHost,
                            onAction: { blockIndex, actionId, data in
                                onAction?(message.id, blockIndex, actionId, data)
                            }
                        )
```

> Note: `directedTo`/`isMultiParticipant`/`homeHost` are declared **before** the trailing `onAction` closure parameter in `AgentMessageView`'s property list, so this call-site order matches the declared order. (Swift requires arguments in declared order for memberwise-style view initializers.)

- [ ] **Step 5: Compute and pass `homeHost` + `isMultiParticipant` in `MessageThreadView`**

In `MessageThreadView.swift`, add a computed property just inside the struct, right after the `@State private var showMessageNotFound = false` line (line 15):

```swift
    @State private var showMessageNotFound = false

    /// The current user's home host (for foreign-host provenance). nil when not connected.
    private var homeHost: String? {
        guard let identity = appState.connectedIdentity else { return nil }
        return ParticipantProvenance.homeHost(from: identity)
    }
```

In the `MessageGroupView(...)` call (lines 42–59), add the two new params after `onBranchThread:`:

```swift
                                onBranchThread: { messageId in
                                    Task { await branchThread(fromMessageId: messageId) }
                                },
                                isMultiParticipant: viewModel.isMultiParticipant,
                                homeHost: homeHost
```

- [ ] **Step 6: Run tests to verify they pass**

Run the full DesignSystem render suite and confirm the app still builds:
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/ParticipantBylineRenderTests \
       -only-testing:AINMMessengerTests/ConversationSurfaceRenderTests
```
Expected: PASS (all). Revert any pbxproj rewrite.

- [ ] **Step 7: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/MessageThread/AgentMessageView.swift \
  ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageGroupView.swift \
  ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift \
  ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/ParticipantBylineRenderTests.swift
rtk proxy git commit -m "feat(sp3): wire ParticipantByline into settled AgentMessageView (tint+host+directional)"
```

---

### Task SP3-7: Wire `ParticipantByline` into `StreamingMessageView` (name tint + provenance only)

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/StreamingMessageView.swift`
- Modify: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift`
- Test: extend `ParticipantBylineRenderTests.swift`

- [ ] **Step 1: Write the failing test**

Append to `ParticipantBylineRenderTests.swift` (inside the class):

```swift
    func test_streamingMessage_tintedBylineNoDirectionalTag() {
        let state = StreamingViewModel.StreamingMessageState(
            id: "s3", messageId: "m9", from: "researcher@acme.test", threadId: "t1",
            blocks: [.init(type: "text", content: "Pulling the numbers…")],
            status: .streaming, finalizingStartTime: nil)
        let view = StreamingMessageView(state: state, isMultiParticipant: true, homeHost: "localhost")
        renderBothSchemes(view)
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run:
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/ParticipantBylineRenderTests
```
Expected: FAIL — `StreamingMessageView` has no `isMultiParticipant` / `homeHost` arguments.

- [ ] **Step 3: Add params + use `ParticipantByline` in `StreamingMessageView`**

In `StreamingMessageView.swift`, add two properties after `let state: StreamingViewModel.StreamingMessageState` (line 9):

```swift
    let state: StreamingViewModel.StreamingMessageState
    var isMultiParticipant: Bool = false
    var homeHost: String? = nil
```

Replace the name `Text` (lines 22–24) with a byline. `directedTo` is intentionally `nil` — the streaming state carries no persisted `to`, and parsing a target from partial streamed text would flicker mid-stream; the directional tag appears once the message settles into `AgentMessageView`:

```swift
                ParticipantByline(
                    senderAddress: state.from,
                    senderName: senderName,
                    directedTo: nil,
                    isMultiParticipant: isMultiParticipant,
                    homeHost: homeHost
                )
```

- [ ] **Step 4: Pass the params from `MessageThreadView`**

In `MessageThreadView.swift`, update the streaming `ForEach` (lines 74–76):

```swift
                        ForEach(appState.streamingViewModel.activeStreamsForThread(threadId)) { stream in
                            StreamingMessageView(
                                state: stream,
                                isMultiParticipant: viewModel.isMultiParticipant,
                                homeHost: homeHost
                            )
                        }
```

- [ ] **Step 5: Run tests to verify they pass**

```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/ParticipantBylineRenderTests \
       -only-testing:AINMMessengerTests/ConversationSurfaceRenderTests
```
Expected: PASS (all). Revert any pbxproj rewrite.

- [ ] **Step 6: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/MessageThread/StreamingMessageView.swift \
  ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift \
  ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/ParticipantBylineRenderTests.swift
rtk proxy git commit -m "feat(sp3): streaming byline name-tint + provenance (directedTo nil; tag on settle)"
```

---

### Task SP3-8: Folded SP2 follow-ups — `rowAddress` + memory-pill tokenization

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/Persistence/ThreadRepository.swift`
- Modify: `ios/AINMMessenger/AINMMessenger/UI/ConversationList/ConversationRowView.swift`
- Modify: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift`
- Test: extend `ParticipantBylineRenderTests.swift` (row-with-agent-address render)

- [ ] **Step 1: Write the failing test**

Append to `ParticipantBylineRenderTests.swift` (inside the class):

```swift
    func test_conversationRow_usesAgentAddressForAvatar() {
        let row = ConversationRow(
            id: "t-9", title: "Assistant", createdAt: "2026-06-08T09:41:00Z",
            createdBy: "me@localhost", hasUnread: false,
            lastMessageTimestamp: "2026-06-08T09:41:00Z",
            lastMessageContent: nil, lastMessageFrom: nil,
            agentAddress: "assistant@localhost")
        renderBothSchemes(ConversationRowView(
            conversation: row, previewText: "On it", relativeTime: "9:41 AM"))
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run:
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/ParticipantBylineRenderTests
```
Expected: FAIL — `ConversationRow` has no `agentAddress` member.

- [ ] **Step 3: Add `agentAddress` to `ConversationRow` + SQL mapping**

In `ThreadRepository.swift`, add the field to the `ConversationRow` struct (after `var lastMessageFrom: String?`, line 14):

```swift
    var lastMessageFrom: String?
    var agentAddress: String? = nil   // defaulted so existing call sites/tests compile unchanged
```

The list query already selects `t.*` (which now includes the `agentAddress` column from migration v7). Map it in the `.map { row in ConversationRow(...) }` closure — add as the last argument after `lastMessageFrom:`:

```swift
                ConversationRow(
                    id: row["id"],
                    title: row["title"],
                    createdAt: row["createdAt"],
                    createdBy: row["createdBy"],
                    hasUnread: row["hasUnread"],
                    lastMessageTimestamp: row["lastMessageTimestamp"],
                    lastMessageContent: row["lastMessageContent"],
                    lastMessageFrom: row["lastMessageFrom"],
                    agentAddress: row["agentAddress"]
                )
```

- [ ] **Step 4: Use `agentAddress` as the row avatar address**

In `ConversationRowView.swift`, replace the `rowAddress` computed property (lines 52–55):

```swift
    /// Address driving the identity avatar: the thread's primary agent address
    /// (LOOP-1 `thread.agentAddress`). nil → legacy avatar fallback (no regression).
    private var rowAddress: String? {
        conversation.agentAddress
    }
```

- [ ] **Step 5: Tokenize the memory-context toolbar pill**

In `MessageThreadView.swift`, in the memory-context `Button` (lines 162–166), replace the two `Color.accentColor` uses with Editorial Warm tokens:

```swift
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                            .background(Theme.Color.accentSoft)
                            .foregroundStyle(Theme.Color.accent)
                            .clipShape(RoundedRectangle(cornerRadius: 6))
```

- [ ] **Step 6: Run tests + verify no `Color.accentColor` remains in this file**

```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/ParticipantBylineRenderTests \
       -only-testing:AINMMessengerTests/ConversationSurfaceRenderTests
grep -n "Color.accentColor" ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift || echo "clean: no Color.accentColor"
```
Expected: PASS (all); grep prints `clean: no Color.accentColor`. Revert any pbxproj rewrite.

> Note: `Theme.Color.accentSoft` and `Theme.Color.accent` exist (SP1). If `accentSoft` is missing, use `Theme.Color.accent.opacity(0.12)` for the background to match the prior visual weight.

- [ ] **Step 7: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Persistence/ThreadRepository.swift \
  ios/AINMMessenger/AINMMessenger/UI/ConversationList/ConversationRowView.swift \
  ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift \
  ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/ParticipantBylineRenderTests.swift
rtk proxy git commit -m "feat(sp3): folded SP2 follow-ups — conversation-row agent avatar + tokenized memory pill"
```

---

### Task SP3-9: Reduce-motion parity + full suites + DoD audit

**Files:**
- Test: extend `ParticipantBylineRenderTests.swift` (reduce-motion render)
- No production files (verification task)

- [ ] **Step 1: Add the reduce-motion parity render test**

Append to `ParticipantBylineRenderTests.swift` (inside the class). Mirrors the SP2 approach — `\.accessibilityReduceMotion` is read-only in Swift 6, so disable UIView animations as the faithful harness equivalent:

```swift
    func test_byline_rendersUnderReduceMotion() {
        UIView.setAnimationsEnabled(false)
        defer { UIView.setAnimationsEnabled(true) }
        let stack = VStack(alignment: .leading) {
            ParticipantByline(
                senderAddress: "assistant@localhost", senderName: "Assistant",
                directedTo: "analyst@acme.test", isMultiParticipant: true, homeHost: "localhost")
            ParticipantByline(
                senderAddress: "analyst@acme.test", senderName: "Analyst",
                directedTo: nil, isMultiParticipant: true, homeHost: "localhost")
        }
        renderBothSchemes(stack)
    }
```

- [ ] **Step 2: Run the new test**

```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/ParticipantBylineRenderTests
```
Expected: PASS (all byline render tests, including reduce-motion).

- [ ] **Step 3: Run the full unit suite**

```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test
```
Expected: full `AINMMessengerTests` suite green. If any pre-existing test asserts an avatar/byline accessibility label that legitimately changed (identity wiring / direction), update **only** that specific assertion to match the new combined label — do not weaken unrelated assertions.

- [ ] **Step 4: Run the UITest suite**

```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessengerUITests \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test
```
Expected: green. (UITests use the `AINMMessengerUITests` scheme — wrong scheme fails deterministically.)

- [ ] **Step 5: DoD audit against the spec**

Confirm each spec §8 item:
1. `toAddress` persisted via migration v9; `MessageRecord` round-trips it; missing → nil (SP3-1 tests).
2. Directional `→ @name` byline renders only when `directedTo != nil` after suppression (SP3-4 + SP3-5/6).
3. Foreign-only `· host` suffix on sender + target names; same-host shows none (SP3-2 + SP3-5).
4. Multi-participant name-tinting; 1:1 stays `inkPrimary` (SP3-5 calm test + `isMultiParticipant`).
5. `ParticipantByline` is the single shared component for settled + streaming (SP3-6, SP3-7).
6. Folded follow-ups done: row uses agent address; memory pill tokenized (SP3-8).
7. Pure helpers unit-tested; render/smoke covers byline variants in light + dark + reduce-motion.
8. Full unit + UITest suites green; clean build; committed to `main`.

Run a final clean build to confirm no warnings introduced:
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build
```
Expected: BUILD SUCCEEDED. Revert any pbxproj rewrite.

- [ ] **Step 6: Commit (only if Step 1's test file changed since SP3-8)**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/ParticipantBylineRenderTests.swift
rtk proxy git commit -m "test(sp3): reduce-motion byline parity; full suites green (SP3 DoD)"
```

---

## Self-Review

**Spec coverage (each §8 DoD → task):**
- DoD 1 (persist `toAddress`) → SP3-1. ✓
- DoD 2 (directional tag, suppression) → SP3-4 (derivation) + SP3-5/SP3-6 (render). ✓
- DoD 3 (foreign-only provenance) → SP3-2 + SP3-5. ✓
- DoD 4 (multi-participant tinting, calm 1:1) → `isMultiParticipant` (SP3-4) + SP3-5. ✓
- DoD 5 (single shared byline) → SP3-5 component, consumed by SP3-6 + SP3-7. ✓
- DoD 6 (folded follow-ups) → SP3-8. ✓
- DoD 7 (pure tests + render/smoke incl. reduce-motion) → SP3-2/3/4 + SP3-5/9. ✓
- DoD 8 (suites green, committed to main) → SP3-9. ✓
- Build order in spec §9 (data → pure helpers → group derivation → byline → settled → streaming → follow-ups → verify) is followed exactly by SP3-1…SP3-9. ✓

**Type consistency:**
- `MessageGroup` gains `directedTo: String?` (SP3-4) and is constructed only in `makeGroup` (updated same task); SP3-5/6 read `group.directedTo`. ✓
- `ParticipantByline` signature `(senderAddress:senderName:directedTo:isMultiParticipant:homeHost:)` is identical in SP3-5 (definition), SP3-6 (AgentMessageView), SP3-7 (StreamingMessageView). ✓
- `AgentMessageView` new params declared in order `directedTo, isMultiParticipant, homeHost` **before** the trailing `onAction` closure; SP3-6 call site matches that order. ✓
- `ParticipantProvenance.homeHost(from:)`, `hostSuffix(of:)`, `isForeignHost(_:homeHost:)` names are stable across SP3-2 (def), SP3-5 (byline), SP3-6 (`MessageThreadView.homeHost`). ✓
- `MessageRecord` memberwise init gains `toAddress: String? = nil` as the **trailing** defaulted param → existing `serverTimestamp: nil` call sites (ScenarioFixture, AppDatabaseTests) compile unchanged. ✓
- `ConversationRow.agentAddress` (SP3-8) defaulted to keep the SP2 render test (`ConversationSurfaceRenderTests.test_conversationRow_*`) compiling without edits, since it constructs `ConversationRow` without that argument. ✓

**Placeholder scan:** No TBD/TODO; every code step shows complete code and exact commands with expected output.

**Risk note flagged inline:** `Theme.Color.accentSoft` fallback documented in SP3-8 Step 6 in case the token name differs.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-08-a2a-multiparticipant-expression.md`. Per your instruction, proceeding with **Subagent-Driven execution** (superpowers:subagent-driven-development): a fresh implementer subagent per task (SP3-1…SP3-9), spec + quality review between tasks, commits to `main` via `rtk proxy git`. Pushing to origin remains a separate, user-gated step after all tasks complete.
