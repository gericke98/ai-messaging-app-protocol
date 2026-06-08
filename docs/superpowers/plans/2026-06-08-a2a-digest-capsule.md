# A2A Digest Capsule (SP4) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Collapse a finished burst of agent-to-agent (A2A) chatter in a multi-participant thread into one expandable "digest capsule" (bordered card), so a human scanning the thread isn't drowned by inter-agent back-and-forth.

**Architecture:** `rebuildGroups()` keeps producing `[MessageGroup]`. A new **pure** `A2ACapsuleFolder.fold(...)` turns that into a `[ThreadItem]` (`.group` | `.capsule`) render list. `MessageThreadView` renders `threadItems`, switching a `.capsule` to a new `A2ACapsuleView` (collapsed card ↔ expanded-in-place, reusing `MessageGroupView`). Display-only — no DB/schema/protocol/network change.

**Tech Stack:** Swift 6 / SwiftUI (iOS 18+), GRDB 7.x, XCTest. Build/test via `xcodebuild`. iOS project at `ios/AINMMessenger`.

---

## Conventions (read once before starting)

- **Git:** ALL git via `rtk proxy git ...` (never bare `git`). Commit directly to `main` (project pattern — no branch). Do **not** push to origin; that's a separate user-gated step after the plan completes.
- **Build (unit tests):**
  ```
  xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
    -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test
  ```
  Scope a class with `-only-testing:AINMMessengerTests/<ClassName>`. UITests use `-scheme AINMMessengerUITests`.
- **pbxproj hygiene:** New `.swift` files under `AINMMessenger/` and `AINMMessengerTests/` auto-include via the synchronized folder group — **zero** pbxproj edits. After any `xcodebuild` run, if `project.pbxproj` or any `*.xcscheme` was rewritten, revert it: `rtk proxy git checkout -- <path>`. Never commit those.
- **SourceKit noise:** in-editor "Cannot find 'X' in scope" / "No such module 'GRDB'/'XCTest'" are stale-index false positives. Trust only `xcodebuild`.
- **Existing shapes (do not redefine):**
  - `MessageGroup` (`UI/MessageThread/MessageThreadViewModel.swift`): synthesized memberwise init order
    `(id:senderAddress:senderType:senderName:messages:isFromCurrentUser:directedTo:)`. `messages: [MessageRecord]`.
  - `MessageThreadViewModel`: `@MainActor @Observable final class`; has `var messageGroups: [MessageGroup]`,
    `let currentUserIdentity: String`, `var isMultiParticipant: Bool` (computed), `contentBlocks(for:)`,
    `formatTimestamp(_:)`, `subThreadInfo(forMessageId:)`. `rebuildGroups()` sets `messageGroups = groups` at its end.
  - `MessageGroupView` init params (all must be passed in the same order they appear): `group`,
    `contentBlocksProvider`, `timestampFormatter`, `onAction`, `isInSubThread`, `subThreadInfo`,
    `onNavigateToSubThread`, `onBranchThread`, `isMultiParticipant`, `homeHost`.
  - `AvatarView(participantType:size:address:displayName:showAgentHalo:)` — identity-colored when `address` set.
  - `ParticipantColor.color(for: String) -> Color`. `Theme.Color.{surface,surfaceBorder,inkPrimary,inkSecondary,paper}`,
    `Theme.Typography.{senderName(),caption()}`, `Theme.Spacing.{xs,sm,md,lg}`, `Theme.Radius.card12`, `Theme.Motion`.
  - `Haptics.select()` (`@MainActor`).
  - `MessageRecord` memberwise init: `(id:threadId:fromAddress:timestamp:type:contentJson:signature:serverTimestamp:toAddress:)` (`toAddress` defaulted).

---

## File Structure

**New files:**
- `ios/AINMMessenger/AINMMessenger/UI/MessageThread/ThreadItem.swift` — `enum ThreadItem` + `struct A2ACapsule`.
- `ios/AINMMessenger/AINMMessenger/UI/MessageThread/A2ACapsuleFolder.swift` — pure fold.
- `ios/AINMMessenger/AINMMessenger/UI/MessageThread/A2ACapsuleView.swift` — collapsed/expanded card.
- `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/A2ACapsuleFolderTests.swift`
- `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/A2ACapsuleRenderTests.swift`

**Modified files:**
- `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadViewModel.swift` — `threadItems` + folder call + expansion state.
- `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift` — render `threadItems`; extract `messageGroupView(for:viewModel:)`.

---

### Task DC-1: `ThreadItem` + `A2ACapsule` models

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/ThreadItem.swift`

- [ ] **Step 1: Write the file** (no separate test — exercised by DC-2's folder tests):

```swift
import Foundation

/// One renderable row in the message thread: either a normal sender group or a
/// collapsed A2A digest capsule. Identifiable with prefixed ids so a capsule (whose
/// id equals its first group's id) never collides with that group's own id in a ForEach.
enum ThreadItem: Identifiable {
    case group(MessageGroup)
    case capsule(A2ACapsule)

    var id: String {
        switch self {
        case .group(let g):   return "g-\(g.id)"
        case .capsule(let c): return "c-\(c.id)"
        }
    }
}

/// A folded run of consecutive agent-to-agent message groups (≥3 messages, ≥2 distinct
/// agents, in a multi-participant thread, bounded by a non-A2A item). Display-only.
struct A2ACapsule: Identifiable {
    let id: String              // == first folded group's id (stable across rebuilds)
    let groups: [MessageGroup]  // the folded groups, in order
    let agents: [String]        // distinct agent addresses, first-seen order
    let messageCount: Int       // total MessageRecords across the run
}
```

- [ ] **Step 2: Build to verify it compiles**

Run:
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build
```
Expected: BUILD SUCCEEDED. Revert any pbxproj rewrite.

- [ ] **Step 3: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/MessageThread/ThreadItem.swift
rtk proxy git commit -m "feat(sp4): ThreadItem + A2ACapsule render-list models"
```

---

### Task DC-2: `A2ACapsuleFolder` (pure) + tests

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/A2ACapsuleFolder.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/A2ACapsuleFolderTests.swift`

- [ ] **Step 1: Write the failing test**

```swift
import XCTest
@testable import AINMMessenger

final class A2ACapsuleFolderTests: XCTestCase {

    let me = "me@localhost"
    let assistant = "assistant@localhost"
    let researcher = "researcher@localhost"

    /// Build a MessageGroup with `msgCount` messages from `sender`.
    private func group(id: String, sender: String, type: String, fromUser: Bool,
                       directedTo: String?, msgCount: Int) -> MessageGroup {
        let msgs = (0..<msgCount).map { k in
            MessageRecord(id: "\(id)-m\(k)", threadId: "t", fromAddress: sender,
                          timestamp: "2026-06-08T10:00:0\(k).000Z", type: "message",
                          contentJson: "[]", signature: "s", serverTimestamp: nil)
        }
        return MessageGroup(id: id, senderAddress: sender, senderType: type,
                            senderName: sender.components(separatedBy: "@").first ?? sender,
                            messages: msgs, isFromCurrentUser: fromUser, directedTo: directedTo)
    }
    private func userGroup(_ id: String) -> MessageGroup {
        group(id: id, sender: me, type: "human", fromUser: true, directedTo: nil, msgCount: 1)
    }
    private func agentGroup(_ id: String, _ who: String, directedTo: String? = nil, msgCount: Int = 1) -> MessageGroup {
        group(id: id, sender: who, type: "agent", fromUser: false, directedTo: directedTo, msgCount: msgCount)
    }

    private func capsules(_ items: [ThreadItem]) -> [A2ACapsule] {
        items.compactMap { if case .capsule(let c) = $0 { return c } else { return nil } }
    }

    func test_boundedRunOf3WithTwoAgents_folds() {
        let groups = [
            userGroup("u1"),
            agentGroup("a1", assistant), agentGroup("a2", researcher), agentGroup("a3", assistant),
            agentGroup("a4", assistant, directedTo: me)   // to-human → bounds + stays inline
        ]
        let items = A2ACapsuleFolder.fold(groups: groups, currentUserIdentity: me, isMultiParticipant: true)
        let caps = capsules(items)
        XCTAssertEqual(caps.count, 1)
        XCTAssertEqual(caps[0].id, "a1")
        XCTAssertEqual(caps[0].messageCount, 3)
        XCTAssertEqual(caps[0].agents, [assistant, researcher])
        // shape: .group(u1), .capsule, .group(a4)
        XCTAssertEqual(items.count, 3)
    }

    func test_runOf2_doesNotFold() {
        let groups = [userGroup("u1"), agentGroup("a1", assistant), agentGroup("a2", researcher), userGroup("u2")]
        let items = A2ACapsuleFolder.fold(groups: groups, currentUserIdentity: me, isMultiParticipant: true)
        XCTAssertTrue(capsules(items).isEmpty)
    }

    func test_singleAgentRun_doesNotFold() {
        let groups = [userGroup("u1"),
                      agentGroup("a1", assistant, msgCount: 3),   // 3 msgs but one distinct agent
                      userGroup("u2")]
        let items = A2ACapsuleFolder.fold(groups: groups, currentUserIdentity: me, isMultiParticipant: true)
        XCTAssertTrue(capsules(items).isEmpty)
    }

    func test_notMultiParticipant_neverFolds() {
        let groups = [userGroup("u1"),
                      agentGroup("a1", assistant), agentGroup("a2", researcher), agentGroup("a3", assistant),
                      userGroup("u2")]
        let items = A2ACapsuleFolder.fold(groups: groups, currentUserIdentity: me, isMultiParticipant: false)
        XCTAssertTrue(capsules(items).isEmpty)
        XCTAssertEqual(items.count, groups.count)
    }

    func test_toHumanAgentMessageBoundsRun() {
        // a3 is directed at the human → not A2A → the A2A run is only a1,a2 (=2) → no capsule
        let groups = [userGroup("u1"),
                      agentGroup("a1", assistant), agentGroup("a2", researcher),
                      agentGroup("a3", assistant, directedTo: me),
                      userGroup("u2")]
        let items = A2ACapsuleFolder.fold(groups: groups, currentUserIdentity: me, isMultiParticipant: true)
        XCTAssertTrue(capsules(items).isEmpty)
    }

    func test_trailingUnboundedRun_staysInline() {
        // A2A run reaches the end of the thread (no closing non-A2A group) → inline, not folded
        let groups = [userGroup("u1"),
                      agentGroup("a1", assistant), agentGroup("a2", researcher), agentGroup("a3", assistant)]
        let items = A2ACapsuleFolder.fold(groups: groups, currentUserIdentity: me, isMultiParticipant: true)
        XCTAssertTrue(capsules(items).isEmpty)
        XCTAssertEqual(items.count, 4)
    }

    func test_twoSeparateBursts_foldIndependently() {
        let groups = [
            userGroup("u1"),
            agentGroup("a1", assistant), agentGroup("a2", researcher), agentGroup("a3", assistant),
            userGroup("u2"),
            agentGroup("b1", researcher), agentGroup("b2", assistant), agentGroup("b3", researcher),
            userGroup("u3")
        ]
        let items = A2ACapsuleFolder.fold(groups: groups, currentUserIdentity: me, isMultiParticipant: true)
        let caps = capsules(items)
        XCTAssertEqual(caps.count, 2)
        XCTAssertEqual(caps[0].id, "a1")
        XCTAssertEqual(caps[1].id, "b1")
    }
}
```

- [ ] **Step 2: Run; expect FAIL** (`A2ACapsuleFolder` undefined)

```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/A2ACapsuleFolderTests
```

- [ ] **Step 3: Write the implementation**

```swift
import Foundation

/// Pure folding pass: turns the sender-grouped thread into a render list where finished
/// agent-to-agent bursts become collapsed capsules. Trigger (all required):
/// multi-participant thread, a *bounded* run of consecutive A2A groups (an A2A group is
/// agent-sent, not from the current user, and not directed at the current user), ≥3 messages
/// total, ≥2 distinct agent senders. The trailing/unbounded run stays inline. Host-free.
enum A2ACapsuleFolder {

    static func fold(
        groups: [MessageGroup],
        currentUserIdentity: String,
        isMultiParticipant: Bool
    ) -> [ThreadItem] {
        guard isMultiParticipant else { return groups.map { .group($0) } }

        func isA2A(_ g: MessageGroup) -> Bool {
            g.senderType == "agent" && !g.isFromCurrentUser && g.directedTo != currentUserIdentity
        }

        var items: [ThreadItem] = []
        var i = 0
        let n = groups.count
        while i < n {
            guard isA2A(groups[i]) else {
                items.append(.group(groups[i]))
                i += 1
                continue
            }
            // Collect the maximal consecutive A2A run [i, j).
            var j = i
            while j < n, isA2A(groups[j]) { j += 1 }
            let run = Array(groups[i..<j])
            let bounded = j < n               // followed by a non-A2A group (by construction)
            let messageCount = run.reduce(0) { $0 + $1.messages.count }
            let agents = orderedDistinct(run.map(\.senderAddress))

            if bounded, messageCount >= 3, agents.count >= 2 {
                items.append(.capsule(A2ACapsule(
                    id: run[0].id, groups: run, agents: agents, messageCount: messageCount)))
            } else {
                items.append(contentsOf: run.map { .group($0) })
            }
            i = j
        }
        return items
    }

    private static func orderedDistinct(_ xs: [String]) -> [String] {
        var seen = Set<String>()
        var out: [String] = []
        for x in xs where seen.insert(x).inserted { out.append(x) }
        return out
    }
}
```

- [ ] **Step 4: Run; expect PASS (7 tests).** Revert any pbxproj rewrite.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/MessageThread/A2ACapsuleFolder.swift \
  ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/A2ACapsuleFolderTests.swift
rtk proxy git commit -m "feat(sp4): pure A2ACapsuleFolder (bounded >=3 / >=2-agent A2A runs) + tests"
```

---

### Task DC-3: View-model `threadItems` + expansion state

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadViewModel.swift`

- [ ] **Step 1: Add stored state.** After `var messageGroups: [MessageGroup] = []` (top of the class), add:

```swift
    var messageGroups: [MessageGroup] = []
    /// Render list derived from messageGroups: normal groups + collapsed A2A capsules.
    var threadItems: [ThreadItem] = []
    /// Capsule ids (== first folded group id) the user has expanded this session. Default collapsed.
    var expandedCapsuleIDs: Set<String> = []
```

- [ ] **Step 2: Add the toggle method** (anywhere in the class body, e.g. after `stopObserving()`):

```swift
    /// Toggle a capsule's expansion. Id is the capsule's first group id (stable across rebuilds).
    func toggleCapsule(_ id: String) {
        if expandedCapsuleIDs.contains(id) {
            expandedCapsuleIDs.remove(id)
        } else {
            expandedCapsuleIDs.insert(id)
        }
    }
```

- [ ] **Step 3: Rebuild `threadItems` in `rebuildGroups()`.** At the END of `rebuildGroups()`, the last line is `messageGroups = groups`. Append immediately after it:

```swift
        messageGroups = groups
        threadItems = A2ACapsuleFolder.fold(
            groups: groups,
            currentUserIdentity: currentUserIdentity,
            isMultiParticipant: isMultiParticipant)
```

- [ ] **Step 4: Build to verify**

```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build
```
Expected: BUILD SUCCEEDED. Revert any pbxproj rewrite.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadViewModel.swift
rtk proxy git commit -m "feat(sp4): MessageThreadViewModel.threadItems (folded) + per-capsule expansion state"
```

---

### Task DC-4: `A2ACapsuleView` (collapsed card ↔ expanded) + render/smoke

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/A2ACapsuleView.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/A2ACapsuleRenderTests.swift`

- [ ] **Step 1: Write the failing render/smoke test**

```swift
import XCTest
import SwiftUI
@testable import AINMMessenger

@MainActor
final class A2ACapsuleRenderTests: XCTestCase {

    @discardableResult
    func render<V: View>(_ view: V, scheme: ColorScheme,
                         size: CGSize = CGSize(width: 390, height: 300)) -> CGSize {
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

    private func msg(_ id: String, _ from: String) -> MessageRecord {
        MessageRecord(id: id, threadId: "t", fromAddress: from, timestamp: "2026-06-08T10:00:00.000Z",
                      type: "message", contentJson: "[]", signature: "s", serverTimestamp: nil)
    }
    private func grp(_ id: String, _ who: String) -> MessageGroup {
        MessageGroup(id: id, senderAddress: who, senderType: "agent",
                     senderName: who.components(separatedBy: "@").first ?? who,
                     messages: [msg("\(id)-m", who)], isFromCurrentUser: false, directedTo: nil)
    }
    private func capsule(_ agents: [String]) -> A2ACapsule {
        let groups = agents.enumerated().map { grp("g\($0.offset)", $0.element) }
        return A2ACapsule(id: "g0", groups: groups, agents: agents, messageCount: max(3, agents.count))
    }

    func test_collapsed_twoAgents() {
        renderBothSchemes(A2ACapsuleView(
            capsule: capsule(["assistant@localhost", "researcher@localhost"]),
            isExpanded: false, homeHost: "localhost", onToggle: {},
            groupContent: { Text($0.senderName) }))
    }

    func test_collapsed_threeAgentsShowsOverflow() {
        renderBothSchemes(A2ACapsuleView(
            capsule: capsule(["assistant@localhost", "researcher@localhost", "analyst@acme.test", "scribe@localhost"]),
            isExpanded: false, homeHost: "localhost", onToggle: {},
            groupContent: { Text($0.senderName) }))
    }

    func test_expanded_rendersGroupContent() {
        renderBothSchemes(A2ACapsuleView(
            capsule: capsule(["assistant@localhost", "researcher@localhost"]),
            isExpanded: true, homeHost: "localhost", onToggle: {},
            groupContent: { Text($0.senderName).font(.body) }))
    }

    func test_capsule_rendersUnderReduceMotion() {
        UIView.setAnimationsEnabled(false)
        defer { UIView.setAnimationsEnabled(true) }
        renderBothSchemes(A2ACapsuleView(
            capsule: capsule(["assistant@localhost", "researcher@localhost"]),
            isExpanded: false, homeHost: "localhost", onToggle: {},
            groupContent: { Text($0.senderName) }))
    }
}
```

- [ ] **Step 2: Run; expect FAIL** (`A2ACapsuleView` undefined)

```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/A2ACapsuleRenderTests
```

- [ ] **Step 3: Write the view** — `A2ACapsuleView.swift`:

```swift
import SwiftUI

/// Treatment B: a finished A2A burst as a bordered "digest" card. Collapsed it shows overlapped
/// agent avatars + identity-tinted names + a message count; tapping expands it in place to the
/// full run of message groups (provided by `groupContent`, normally `MessageGroupView`), with a
/// quiet collapse control. Display-only; reuses Editorial Warm tokens.
struct A2ACapsuleView<GroupContent: View>: View {
    let capsule: A2ACapsule
    let isExpanded: Bool
    let homeHost: String?
    let onToggle: () -> Void
    @ViewBuilder let groupContent: (MessageGroup) -> GroupContent

    private let maxAvatars = 3

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.Spacing.sm) {
            header
            if isExpanded {
                VStack(alignment: .leading, spacing: Theme.Spacing.lg) {
                    ForEach(capsule.groups) { group in
                        groupContent(group)
                    }
                }
                .padding(.leading, Theme.Spacing.sm)
                Button(action: { Haptics.select(); onToggle() }) {
                    HStack(spacing: Theme.Spacing.xs) {
                        Image(systemName: "chevron.up")
                        Text("Collapse")
                    }
                    .font(Theme.Typography.caption())
                    .foregroundStyle(Theme.Color.inkSecondary)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(Theme.Spacing.md)
        .background(Theme.Color.surface)
        .overlay(
            RoundedRectangle(cornerRadius: Theme.Radius.card12)
                .strokeBorder(Theme.Color.surfaceBorder, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: Theme.Radius.card12))
        .padding(.horizontal, Theme.Spacing.lg)
        .animation(Theme.Motion.messageAppear, value: isExpanded)
        .contentShape(Rectangle())
        .accessibilityElement(children: isExpanded ? .contain : .ignore)
        .accessibilityLabel(isExpanded ? "Agent deliberation, expanded" : accessibilityLabel)
        .accessibilityHint(isExpanded ? "" : "Double-tap to expand")
    }

    // MARK: - Header (the always-visible digest row)

    private var header: some View {
        Button(action: { Haptics.select(); onToggle() }) {
            HStack(spacing: Theme.Spacing.sm) {
                avatarStack
                namesAndCount
                Spacer(minLength: 0)
                Image(systemName: isExpanded ? "chevron.up" : "chevron.down")
                    .font(Theme.Typography.caption())
                    .foregroundStyle(Theme.Color.inkSecondary)
            }
        }
        .buttonStyle(.plain)
    }

    private var avatarStack: some View {
        let shown = Array(capsule.agents.prefix(maxAvatars))
        return HStack(spacing: -8) {
            ForEach(Array(shown.enumerated()), id: \.offset) { _, address in
                AvatarView(participantType: "agent", size: 24, address: address,
                           displayName: localPart(address))
                    .overlay(Circle().strokeBorder(Theme.Color.surface, lineWidth: 1.5))
            }
            if capsule.agents.count > maxAvatars {
                Text("+\(capsule.agents.count - maxAvatars)")
                    .font(Theme.Typography.caption())
                    .foregroundStyle(Theme.Color.inkSecondary)
                    .padding(.leading, Theme.Spacing.sm)
            }
        }
    }

    private var namesAndCount: some View {
        VStack(alignment: .leading, spacing: 1) {
            HStack(spacing: Theme.Spacing.xs) {
                let names = Array(capsule.agents.prefix(2))
                ForEach(Array(names.enumerated()), id: \.offset) { idx, address in
                    if idx > 0 {
                        Text(capsule.agents.count == 2 ? "↔" : ",")
                            .font(Theme.Typography.caption())
                            .foregroundStyle(Theme.Color.inkSecondary)
                    }
                    Text(localPart(address))
                        .font(Theme.Typography.senderName())
                        .foregroundStyle(ParticipantColor.color(for: address))
                }
                if capsule.agents.count > 2 {
                    Text("+\(capsule.agents.count - 2)")
                        .font(Theme.Typography.caption())
                        .foregroundStyle(Theme.Color.inkSecondary)
                }
            }
            Text("\(capsule.messageCount) messages")
                .font(Theme.Typography.caption())
                .foregroundStyle(Theme.Color.inkSecondary)
        }
    }

    // MARK: - Helpers

    private func localPart(_ address: String) -> String {
        address.components(separatedBy: "@").first ?? address
    }

    private var accessibilityLabel: String {
        let names = capsule.agents.map(localPart).joined(separator: ", ")
        return "Agent deliberation, \(names), \(capsule.messageCount) messages, collapsed"
    }
}
```

> If `Theme.Motion.messageAppear` is not the exact token name, use the existing motion token the SP2 living-message used (grep `Theme.Motion` in `StreamingMessageView`/`Theme.swift`); the behavior (a subtle spring on `isExpanded`) is what matters. Under reduce-motion the test disables UIView animations, matching SP2/SP3.

- [ ] **Step 4: Run; expect PASS (4 tests).** Revert any pbxproj rewrite.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/MessageThread/A2ACapsuleView.swift \
  ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/A2ACapsuleRenderTests.swift
rtk proxy git commit -m "feat(sp4): A2ACapsuleView bordered card (collapsed/expanded) + render/smoke"
```

---

### Task DC-5: Render `threadItems` in `MessageThreadView`

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift`

- [ ] **Step 1: Extract the group-row builder.** Add this private method to the `MessageThreadView` struct (e.g. just before `handleAction`):

```swift
    /// Builds the standard message-group row. Reused by both the `.group` render case and the
    /// expanded body of an A2A capsule, so a capsule's messages render identically to inline ones.
    @ViewBuilder
    private func messageGroupView(for group: MessageGroup, viewModel: MessageThreadViewModel) -> some View {
        MessageGroupView(
            group: group,
            contentBlocksProvider: { viewModel.contentBlocks(for: $0) },
            timestampFormatter: { viewModel.formatTimestamp($0) },
            onAction: { messageId, blockIndex, actionId, data in
                handleAction(messageId: messageId, blockIndex: blockIndex, actionId: actionId, data: data)
            },
            isInSubThread: parentThreadInfo != nil,
            subThreadInfo: { messageId in
                viewModel.subThreadInfo(forMessageId: messageId)
            },
            onNavigateToSubThread: { subThreadId in
                appState.selectedThreadId = subThreadId
            },
            onBranchThread: { messageId in
                Task { await branchThread(fromMessageId: messageId) }
            },
            isMultiParticipant: viewModel.isMultiParticipant,
            homeHost: homeHost
        )
    }
```

- [ ] **Step 2: Replace the `ForEach(viewModel.messageGroups)` block** (currently lines ~47-68, the entire `ForEach(viewModel.messageGroups) { group in MessageGroupView(...) }`) with a switch over `threadItems`:

```swift
                        ForEach(viewModel.threadItems) { item in
                            switch item {
                            case .group(let group):
                                messageGroupView(for: group, viewModel: viewModel)
                            case .capsule(let capsule):
                                A2ACapsuleView(
                                    capsule: capsule,
                                    isExpanded: viewModel.expandedCapsuleIDs.contains(capsule.id),
                                    homeHost: homeHost,
                                    onToggle: { viewModel.toggleCapsule(capsule.id) },
                                    groupContent: { messageGroupView(for: $0, viewModel: viewModel) }
                                )
                            }
                        }
```

(Everything else in the `LazyVStack` — `isLoadingMore`, `ToolCallGroupView`, the streaming `ForEach`, typing indicators — is unchanged.)

- [ ] **Step 3: Build + run the full design-system render suites + folder tests**

```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/A2ACapsuleFolderTests \
       -only-testing:AINMMessengerTests/A2ACapsuleRenderTests \
       -only-testing:AINMMessengerTests/ConversationSurfaceRenderTests
```
Expected: all pass; the app compiles (the render tests force the full app target to build). Revert any pbxproj rewrite.

- [ ] **Step 4: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift
rtk proxy git commit -m "feat(sp4): render threadItems — switch group vs A2A capsule; reuse group-row builder"
```

---

### Task DC-6: Full suites + DoD audit

**Files:** none (verification).

- [ ] **Step 1: Full unit suite**
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test
```
Expected: entire `AINMMessengerTests` suite green. If a pre-existing test that iterates `messageGroups` is unaffected (we retained `messageGroups`), it should still pass. Any failure that is a genuine regression: STOP and report BLOCKED with the test name + output.

- [ ] **Step 2: UITest suite**
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessengerUITests \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test
```
Expected: green. (Existing threads in fixtures are 1:1 or lack ≥3-message ≥2-agent bounded bursts, so no capsule appears and message-row identifiers are unchanged. If a UITest asserts on a thread that now capsules, that's a real signal — report it.)

- [ ] **Step 3: Clean build**
```
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build
```
Expected: BUILD SUCCEEDED, no new warnings.

- [ ] **Step 4: DoD audit (report each PASS/FAIL with evidence)**
1. `A2ACapsuleFolder.fold` implements the trigger (≥3, ≥2 agents, multi-participant, bounded); pure + unit-tested (DC-2).
2. `ThreadItem` + `A2ACapsule` models; `threadItems` rebuilt via folder; `messageGroups` retained (DC-1, DC-3).
3. `A2ACapsuleView` treatment B: collapsed card (overlapped avatars + tinted names + count) ↔ expanded run + collapse; `Haptics.select()` (DC-4).
4. Per-capsule expansion via `expandedCapsuleIDs` (stable id), default collapsed, preserved across rebuilds (DC-3).
5. 1:1 threads, ≤2 runs, single-agent runs, and the unbounded tail never capsule (DC-2 tests).
6. `MessageThreadView` renders `threadItems`; render/smoke covers collapsed/expanded in light + dark + reduce-motion (DC-4, DC-5).
7. Full unit + UITest green; clean build; committed to `main`.

- [ ] **Step 5: Commit (only if a verification-only fix was needed)** — otherwise nothing to commit. Revert any pbxproj rewrite.

---

## Self-Review

**Spec coverage (each §8 DoD → task):** DoD 1→DC-2; DoD 2→DC-1/DC-3; DoD 3→DC-4; DoD 4→DC-3; DoD 5→DC-2; DoD 6→DC-4/DC-5; DoD 7→DC-6. Build order (§9: models → folder → VM → view → render switch → verify) = DC-1…DC-6 exactly. ✓

**Type consistency:**
- `A2ACapsuleFolder.fold(groups:currentUserIdentity:isMultiParticipant:) -> [ThreadItem]` — identical in DC-2 (def), DC-3 (VM call). ✓
- `MessageGroup` memberwise init order `(id:senderAddress:senderType:senderName:messages:isFromCurrentUser:directedTo:)` used consistently in DC-2 and DC-4 test helpers. ✓
- `A2ACapsule(id:groups:agents:messageCount:)` consistent across DC-1 (def), DC-2 (folder), DC-4 (test). ✓
- `A2ACapsuleView(capsule:isExpanded:homeHost:onToggle:groupContent:)` generic over `GroupContent: View` — same call in DC-4 tests and DC-5 (`groupContent: { messageGroupView(for: $0, viewModel: viewModel) }`). ✓
- `ThreadItem.id` prefixes (`g-`/`c-`) prevent ForEach id collision between a capsule and its first group. ✓
- `messageGroupView(for:viewModel:)` passes `MessageGroupView`'s params in declared order; reused by both `.group` and the capsule's `groupContent`. ✓

**Placeholder scan:** none — every step has complete code + exact commands. The one engineer-note (`Theme.Motion.messageAppear` token-name fallback) flags a real lookup with a concrete resolution.

**No-regression:** `messageGroups` retained (so `handleScrollToMessage` and any other consumer is untouched); capsules only appear in multi-participant threads with a bounded ≥3/≥2-agent burst, which existing fixtures don't contain → UITests unaffected.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-08-a2a-digest-capsule.md`. Per the established pattern, proceeding with **Subagent-Driven execution** (superpowers:subagent-driven-development): a fresh implementer subagent per task (DC-1…DC-6), build/test verification between tasks, commits to `main` via `rtk proxy git`. Pushing to origin remains a separate, user-gated step after all tasks complete.
