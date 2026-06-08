# Conversation Surface Polish (SP2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply the SP1 Editorial Warm design system (`Theme`, `ParticipantColor`, `AvatarView`, `Haptics`) to every everyday conversation surface, add the living agent message (haloed streaming avatar + identity caret + status line), wire haptics, and resolve the three SP1 carried follow-ups.

**Architecture:** Pure SwiftUI view modification. No new architectural layers, no view-model/network/persistence changes. Views read `Theme`/`ParticipantColor` tokens directly and thread identity (`address`/`displayName`) into `AvatarView`/`CursorView`. Streamed and settled renderings of the same agent read the same identity inputs, guaranteeing visual continuity.

**Tech Stack:** SwiftUI (iOS 18+), Swift 6, XCTest (app-hosted test target with `@testable import AINMMessenger`), optional `pointfreeco/swift-snapshot-testing` (test-only SPM dep, with a dependency-free render/smoke fallback).

---

## Conventions for every task

- **Git:** commit with `rtk proxy git` (NEVER bare `git`). Commit directly to `main` (project pattern, no branches). Do **NOT** push to origin — pushing is a separate user-gated step after the whole plan completes.
- **Do NOT commit Xcode auto-rewrites** of `ios/AINMMessenger/AINMMessenger.xcodeproj/project.pbxproj` or `*.xcscheme`. After any `xcodebuild` run, check `rtk proxy git status`; if only those files are dirty from normalization, `rtk proxy git checkout --` them before committing. (Exception: CSP-2 may deliberately commit a **minimal** package-reference addition to `project.pbxproj` — see that task.)
- **New `.swift` files** dropped under `ios/AINMMessenger/AINMMessenger/` or `…/AINMMessengerTests/` auto-include via synchronized folder groups (`PBXFileSystemSynchronizedRootGroup`) — **zero pbxproj editing** for source files.
- **Build (compile check):**
  `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build`
- **Unit test (targeted):**
  `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test -only-testing:AINMMessengerTests/<Class>`
- **Full unit suite:** same as above without `-only-testing`.
- **UITests** (only when an accessibility identifier/label is touched): `-scheme AINMMessengerUITests`.
- **SourceKit false positives** ("No such module 'XCTest'/'UIKit'", "Cannot find 'Theme'/'ParticipantColor' in scope") are stale-index noise — trust the `xcodebuild` result, not the editor.

## Token quick-reference (from SP1 — do not redefine)

- Colors: `Theme.Color.paper`, `.surface`, `.surfaceBorder`, `.inkPrimary`, `.inkSecondary`, `.humanBubble`, `.humanBubbleText`, `.accent`, `.accentSoft`, `.separator`. Identity: `ParticipantColor.color(for: address)`.
- Typography (functions, call with `()`): `Theme.Typography.displayLarge()`, `.displayTitle()`, `.senderName()` (serif), `.agentProse()` (serif), `.body()`, `.callout()`, `.label()`, `.caption()`.
- Spacing: `Theme.Spacing.xs`=4, `.sm`=8, `.md`=12, `.lg`=16, `.xl`=20, `.xxl`=24.
- Radius: `Theme.Radius.chip`=8, `.card`=12, `.bubble`=16.
- Haptics (`@MainActor`): `Haptics.send()`, `Haptics.receive()`.

---

## Render/smoke test harness (shared, dependency-free)

Several tasks add render/smoke tests. They all use one helper. **CSP-2 creates this file**; later tasks append `func test…` methods to it.

File: `ios/AINMMessenger/AINMMessenger/../AINMMessengerTests/DesignSystemTests/ConversationSurfaceRenderTests.swift`

```swift
import XCTest
import SwiftUI
@testable import AINMMessenger

/// Dependency-free render/smoke tests: instantiate each restyled conversation surface,
/// force a layout pass in both color schemes, and assert it neither crashes nor produces
/// a zero-size layout. Catches compile breaks, token-resolution failures, and layout traps
/// without the fragility of cross-machine image diffing.
@MainActor
final class ConversationSurfaceRenderTests: XCTestCase {

    /// Hosts `view` in a real window, forces layout in the given scheme, returns laid-out size.
    @discardableResult
    func render<V: View>(_ view: V, scheme: ColorScheme, size: CGSize = CGSize(width: 390, height: 240)) -> CGSize {
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

    /// Render in both light and dark; assert non-zero size each time.
    func renderBothSchemes<V: View>(_ view: V, file: StaticString = #filePath, line: UInt = #line) {
        for scheme in [ColorScheme.light, .dark] {
            let s = render(view, scheme: scheme)
            XCTAssertGreaterThan(s.width, 0, "zero width in \(scheme)", file: file, line: line)
            XCTAssertGreaterThan(s.height, 0, "zero height in \(scheme)", file: file, line: line)
        }
    }

    /// Convenience: a one-text-block agent/human content array.
    func textBlocks(_ s: String, source: String) -> [ContentBlock] {
        [.text(ContentBlock.TextContent(version: "1.0", source: source, text: s, mentions: nil, fallback: s))]
    }
}
```

---

### Task CSP-1: Recolor `Identity0` distinct from `Accent` (follow-up #1)

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/DesignSystem/Assets.xcassets/Identity0.colorset/Contents.json`
- Test (existing, must stay green): `AINMMessengerTests/DesignSystemTests/ParticipantColorTests.swift`

`Identity0` currently equals `Accent` (burnt orange `#C2410C` light / `#E8743A` dark). Recolor it to a distinct indigo so no participant shares the brand accent. Palette stays 10 entries; `ParticipantColor` golden indices map to slots, not hues, so they are unaffected.

- [ ] **Step 1: Replace the colorset contents** with a distinct indigo (light `#4F46E5`, dark `#818CF8` — WCAG AA with white initials: ~5.9:1 light, ~3.6:1 dark large-text/semibold OK; matches the palette's saturated-but-legible character).

`Identity0.colorset/Contents.json`:
```json
{
  "colors" : [
    {
      "color" : {
        "color-space" : "srgb",
        "components" : { "alpha" : "1.000", "blue" : "0xE5", "green" : "0x46", "red" : "0x4F" }
      },
      "idiom" : "universal"
    },
    {
      "appearances" : [ { "appearance" : "luminosity", "value" : "dark" } ],
      "color" : {
        "color-space" : "srgb",
        "components" : { "alpha" : "1.000", "blue" : "0xF8", "green" : "0x8C", "red" : "0x81" }
      },
      "idiom" : "universal"
    }
  ],
  "info" : { "author" : "xcode", "version" : 1 }
}
```

- [ ] **Step 2: Run the ParticipantColor golden tests** — they must still pass (indices are hue-independent).

Run: `xcodebuild … test -only-testing:AINMMessengerTests/ParticipantColorTests`
Expected: PASS (golden indices `assistant@localhost`→8, `researcher@acme.test`→5 unchanged).

- [ ] **Step 3: Verify pbxproj/scheme not polluted**, then commit.

```bash
rtk proxy git status   # if only .pbxproj/.xcscheme dirtied by xcodebuild, revert them:
# rtk proxy git checkout -- ios/AINMMessenger/AINMMessenger.xcodeproj/project.pbxproj
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/DesignSystem/Assets.xcassets/Identity0.colorset/Contents.json
rtk proxy git commit -m "fix(ui): recolor Identity0 distinct from Accent (SP2 follow-up #1)"
```

---

### Task CSP-2: Test harness — render/smoke file + attempt snapshot dependency

**Files:**
- Create: `AINMMessengerTests/DesignSystemTests/ConversationSurfaceRenderTests.swift` (full content in the "Render/smoke test harness" section above)
- Possibly modify (deliberate, minimal): `ios/AINMMessenger/AINMMessenger.xcodeproj/project.pbxproj`

This task establishes testing for the visual work. The dependency-free render harness is the **guaranteed** path; `swift-snapshot-testing` image diffing is an **optional enhancement** attempted here.

- [ ] **Step 1: Create the render/smoke harness file** exactly as given in the "Render/smoke test harness" section above. It compiles against today's pbxproj (synchronized folder group auto-includes it; XCTest already wired for `DesignSystemTests`).

- [ ] **Step 2: Verify the harness compiles and runs** (it has only helper methods + no `test…` yet, so it's an empty-but-valid test class).

Run: `xcodebuild … test -only-testing:AINMMessengerTests/ConversationSurfaceRenderTests`
Expected: PASS (0 tests run, class compiles).

- [ ] **Step 3: Attempt to add `pointfreeco/swift-snapshot-testing` to the `AINMMessengerTests` target.**

Add it the same way GRDB/KeychainAccess are referenced. This requires three minimal additions to `project.pbxproj`:
  1. an `XCRemoteSwiftPackageReference "swift-snapshot-testing"` (URL `https://github.com/pointfreeco/swift-snapshot-testing`, `minimumVersion` `1.17.0`),
  2. an `XCSwiftPackageProductDependency` for product `SnapshotTesting`,
  3. that product in the **`AINMMessengerTests`** target's `packageProductDependencies` + its Frameworks build phase, and the package reference in the project's `packageReferences`.

Then resolve packages:
`xcodebuild … -resolvePackageDependencies`

- [ ] **Step 4: DECISION GATE — is the pbxproj still well-formed and stable?**

Run the full unit suite once: `xcodebuild … test`.
  - **If it builds, resolves, and the only pbxproj changes are the three deliberate package-reference additions** (no wholesale normalization of unrelated sections): keep them. Commit the dependency. Snapshot image tests are available; CSP-12 will use them.
  - **If Xcode normalizes the whole hand-authored pbxproj, the build breaks, or package resolution destabilizes the project:** `rtk proxy git checkout -- …/project.pbxproj`, abandon the dependency, and proceed with **render/smoke only** (the harness from Step 1). This is the sanctioned fallback (spec §7).

Report in the task summary which path was taken: **`SNAPSHOT_LIB`** or **`RENDER_SMOKE`**. CSP-4..CSP-12 honor this verdict.

- [ ] **Step 5: Commit.**

```bash
rtk proxy git status
# SNAPSHOT_LIB path: add the render harness + the minimal pbxproj package-reference diff.
# RENDER_SMOKE path: add only the render harness; ensure pbxproj is reverted to hand-authored form.
rtk proxy git add AINMMessengerTests/DesignSystemTests/ConversationSurfaceRenderTests.swift
# (SNAPSHOT_LIB only) rtk proxy git add ios/AINMMessenger/AINMMessenger.xcodeproj/project.pbxproj
rtk proxy git commit -m "test(ui): SP2 render/smoke harness (+ snapshot-testing dep if stable)"
```

---

### Task CSP-3: `CursorView` — identity/accent caret color + reduce-motion (already respected)

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/Shared/CursorView.swift`

Add a `color` parameter (default `Theme.Color.accent`) so streaming can pass the message's identity color. Reduce-motion is already honored (stays solid). Replace the hard-coded `Color.accentColor`.

- [ ] **Step 1: Replace the file body.**

```swift
import SwiftUI

/// Blinking caret for streaming text (UI-SPEC Component 2).
/// 2pt-wide rounded rectangle that blinks at a 0.5s cycle in the supplied color
/// (the message's identity color, or Accent by default). Stays solid under
/// `accessibilityReduceMotion`.
struct CursorView: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var opacity: Double = 1.0
    var height: CGFloat = 12  // matches body cap height
    var color: Color = Theme.Color.accent

    var body: some View {
        RoundedRectangle(cornerRadius: 1)
            .fill(color)
            .frame(width: 2, height: height)
            .opacity(opacity)
            .onAppear {
                if !reduceMotion {
                    withAnimation(.easeInOut(duration: 0.5).repeatForever(autoreverses: true)) {
                        opacity = 0.0
                    }
                }
            }
            .accessibilityHidden(true)
    }
}
```

- [ ] **Step 2: Build to confirm it compiles** (existing call sites use the defaulted initializer, so they are unaffected).

Run: `xcodebuild … build`
Expected: BUILD SUCCEEDED.

- [ ] **Step 3: Revert any pbxproj/scheme pollution; commit.**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/Shared/CursorView.swift
rtk proxy git commit -m "feat(ui): CursorView accepts identity caret color (SP2)"
```

---

### Task CSP-4: `AvatarView` halo — clipping-safe stroke (follow-up #3) + render test

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/Shared/AvatarView.swift`
- Test: append to `ConversationSurfaceRenderTests.swift`

The halo currently uses `Circle().stroke(…).scaleEffect(1.18)`, which draws **outside** the avatar bounds and gets clipped by clipped parents. Replace it with an inner/padded ring drawn **within** the avatar's own frame: reserve padding and stroke a slightly inset circle so the ring always survives.

- [ ] **Step 1: Replace `identityAvatar(address:)`** in `AvatarView.swift` with a clipping-safe halo. The avatar keeps its `size` footprint; when haloed, the colored disc is inset and the ring is drawn in the reserved margin.

Replace the existing `identityAvatar` function with:
```swift
    @ViewBuilder
    private func identityAvatar(address: String) -> some View {
        let fill = ParticipantColor.color(for: address)
        let initials = ParticipantColor.initials(for: displayName ?? address)
        let haloed = showAgentHalo && isAgent
        // Reserve a margin for the ring so it never draws outside `size` (clip-safe).
        let ringWidth: CGFloat = max(2, size * 0.06)
        let margin: CGFloat = haloed ? ringWidth + 2 : 0
        let discSize = size - margin * 2

        ZStack {
            if haloed {
                Circle()
                    .strokeBorder(fill.opacity(0.35), lineWidth: ringWidth)
                    .frame(width: size, height: size)
            }
            Circle()
                .fill(fill)
                .frame(width: discSize, height: discSize)
                .overlay {
                    Text(initials)
                        .font(.system(size: discSize * 0.42, weight: .semibold, design: .rounded))
                        .foregroundStyle(.white)
                }
        }
        .frame(width: size, height: size)
    }
```

(`strokeBorder` strokes inward from the edge, so even at the outer `size` frame the ring stays inside bounds. The disc shrinks by the reserved margin only when haloed, so non-haloed avatars are pixel-identical to before.)

- [ ] **Step 2: Append a halo render test** to `ConversationSurfaceRenderTests.swift` (covers follow-up #3 — no clipping in a clipped parent):

```swift
    func test_haloedAvatar_rendersInsideClippedParent() {
        let avatar = AvatarView(
            participantType: "agent", size: 36,
            address: "assistant@localhost", displayName: "Assistant",
            showAgentHalo: true
        )
        // Clip to exactly the avatar's footprint: a clip-unsafe ring would be cut off,
        // but rendering must still succeed and lay out at full size.
        let clipped = avatar.frame(width: 36, height: 36).clipped()
        renderBothSchemes(clipped)
    }
```

- [ ] **Step 3: Run the test.**

Run: `xcodebuild … test -only-testing:AINMMessengerTests/ConversationSurfaceRenderTests`
Expected: PASS.

- [ ] **Step 4: Revert pbxproj/scheme pollution; commit.**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/Shared/AvatarView.swift AINMMessengerTests/DesignSystemTests/ConversationSurfaceRenderTests.swift
rtk proxy git commit -m "fix(ui): clipping-safe agent halo + render test (SP2 follow-up #3)"
```

---

### Task CSP-5: `StreamingMessageView` — living agent message (follow-up #2 for streaming)

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/StreamingMessageView.swift`
- Test: append to `ConversationSurfaceRenderTests.swift`

Make the streaming message render the agent **identically** to the settled `AgentMessageView` except for live affordances: identity avatar (haloed while streaming), serif name, serif prose on paper (no grey badge), identity-colored caret, and a quiet `✺ thinking…` status line. This eliminates the orange→identity avatar flip (follow-up #2) because the avatar now uses `address: state.from`.

- [ ] **Step 1: Replace the file body.**

```swift
import SwiftUI

/// In-progress streaming message (UI-SPEC Component 1), rendered as the "living"
/// counterpart of a settled AgentMessageView: identity avatar with a halo while
/// streaming, serif name + prose laid open on the paper, an identity-colored caret
/// at the active block, and a quiet "✺ thinking…" status line. A "Not verified" badge
/// appears if the signed message does not arrive within 5s of stream.end.
struct StreamingMessageView: View {
    let state: StreamingViewModel.StreamingMessageState

    var body: some View {
        HStack(alignment: .top, spacing: Theme.Spacing.sm) {
            AvatarView(
                participantType: "agent",
                size: 30,
                address: state.from,
                displayName: senderName,
                showAgentHalo: showCursor
            )

            VStack(alignment: .leading, spacing: Theme.Spacing.xs) {
                Text(senderName)
                    .font(Theme.Typography.senderName())
                    .foregroundStyle(Theme.Color.inkPrimary)

                if showCursor {
                    Text("✺ thinking…")
                        .font(Theme.Typography.caption())
                        .foregroundStyle(Theme.Color.inkSecondary)
                        .accessibilityLabel("Agent is thinking")
                }

                VStack(alignment: .leading, spacing: Theme.Spacing.sm) {
                    ForEach(Array(state.blocks.enumerated()), id: \.offset) { index, block in
                        streamingBlockView(block: block, isLast: index == state.blocks.count - 1)
                    }
                }

                if state.status == .notVerified {
                    HStack(spacing: Theme.Spacing.xs) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .font(Theme.Typography.caption())
                        Text("Not verified")
                            .font(Theme.Typography.caption())
                    }
                    .foregroundStyle(Theme.Color.inkSecondary)
                    .accessibilityLabel("Message not verified")
                    .accessibilityHint("The signed message was not received from the server within the expected time.")
                }
            }
        }
        .padding(.horizontal, Theme.Spacing.lg)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(state.status == .streaming ? "Agent is generating a response" : "Agent response")
    }

    // MARK: - Private

    private var senderName: String {
        state.from.components(separatedBy: "@").first ?? state.from
    }

    private var caretColor: Color {
        ParticipantColor.color(for: state.from)
    }

    @ViewBuilder
    private func streamingBlockView(
        block: StreamingViewModel.StreamingBlockState,
        isLast: Bool
    ) -> some View {
        if block.type == "code" {
            VStack(alignment: .leading, spacing: Theme.Spacing.xs) {
                if let lang = block.language {
                    Text(lang)
                        .font(Theme.Typography.caption())
                        .foregroundStyle(Theme.Color.inkSecondary)
                }
                HStack(spacing: 0) {
                    Text(block.content)
                        .font(.system(.body, design: .monospaced))
                        .foregroundStyle(Theme.Color.inkPrimary)
                    if isLast && showCursor {
                        CursorView(height: 10, color: caretColor)
                    }
                }
            }
            .padding(Theme.Spacing.md)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.Color.surface)
            .clipShape(RoundedRectangle(cornerRadius: Theme.Radius.chip))
        } else {
            HStack(alignment: .firstTextBaseline, spacing: 0) {
                Text(block.content)
                    .font(Theme.Typography.agentProse())
                    .foregroundStyle(Theme.Color.inkPrimary)
                if isLast && showCursor {
                    CursorView(height: 12, color: caretColor)
                }
            }
        }
    }

    private var showCursor: Bool {
        state.status == .streaming
    }
}
```

- [ ] **Step 2: Append a streaming render test** (caret + halo + status line, both schemes):

```swift
    func test_streamingMessage_rendersLivingTreatment() {
        let state = StreamingViewModel.StreamingMessageState(
            id: "s1", messageId: "m1", from: "assistant@localhost", threadId: "t1",
            blocks: [.init(type: "text", content: "Monday is your clearest block")],
            status: .streaming, finalizingStartTime: nil
        )
        renderBothSchemes(StreamingMessageView(state: state))
    }
```

- [ ] **Step 3: Run the test.**

Run: `xcodebuild … test -only-testing:AINMMessengerTests/ConversationSurfaceRenderTests`
Expected: PASS.

- [ ] **Step 4: Revert pbxproj/scheme pollution; commit.**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/MessageThread/StreamingMessageView.swift AINMMessengerTests/DesignSystemTests/ConversationSurfaceRenderTests.swift
rtk proxy git commit -m "feat(ui): living streaming agent message — identity avatar+halo, caret, status (SP2 follow-up #2)"
```

---

### Task CSP-6: `TypingIndicatorView` — identity avatar + status-line tokens (follow-up #2 for typing)

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/TypingIndicatorView.swift`
- Modify: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift` (pass `address`)
- Test: append to `ConversationSurfaceRenderTests.swift`

Add an `address` parameter so the typing avatar uses the participant's identity color (no legacy orange). Tokenize dots + label. Reduce-motion is already respected.

- [ ] **Step 1: Replace `TypingIndicatorView.swift` body.**

```swift
import SwiftUI

/// Typing/processing indicator with bouncing dots (UI-SPEC Component 4).
/// Uses the participant's identity-colored avatar (agents get a halo) and the
/// Editorial Warm tokens. Respects `accessibilityReduceMotion`.
struct TypingIndicatorView: View {
    let name: String
    let isAgent: Bool
    var address: String? = nil
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var dotOffsets: [CGFloat] = [0, 0, 0]

    var body: some View {
        HStack(alignment: .center, spacing: Theme.Spacing.sm) {
            AvatarView(
                participantType: isAgent ? "agent" : "human",
                size: 24,
                address: address,
                displayName: name,
                showAgentHalo: isAgent
            )

            HStack(spacing: Theme.Spacing.xs) {
                ForEach(0..<3, id: \.self) { index in
                    Circle()
                        .fill(Theme.Color.inkSecondary)
                        .frame(width: 6, height: 6)
                        .offset(y: dotOffsets[index])
                }
            }

            Text("\(name) is \(isAgent ? "processing" : "typing")...")
                .font(Theme.Typography.caption())
                .foregroundStyle(Theme.Color.inkSecondary)
        }
        .padding(.vertical, Theme.Spacing.sm)
        .padding(.horizontal, Theme.Spacing.lg)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(name) is \(isAgent ? "processing" : "typing")")
        .onAppear {
            guard !reduceMotion else { return }
            for i in 0..<3 {
                withAnimation(
                    .easeInOut(duration: 0.4)
                    .repeatForever(autoreverses: true)
                    .delay(Double(i) * 0.15)
                ) {
                    dotOffsets[i] = -4
                }
            }
        }
    }
}
```

- [ ] **Step 2: Pass the identity address from `MessageThreadView`.** In `MessageThreadView.swift`, the presence loop currently reads:

```swift
                        ForEach(appState.presenceEntries(forThread: threadId)) { entry in
                            TypingIndicatorView(name: entry.name, isAgent: entry.isAgent)
                        }
```

Replace with (`PresenceEntry.id` is the participant identity/address):

```swift
                        ForEach(appState.presenceEntries(forThread: threadId)) { entry in
                            TypingIndicatorView(name: entry.name, isAgent: entry.isAgent, address: entry.id)
                        }
```

- [ ] **Step 3: Append a typing render test:**

```swift
    func test_typingIndicator_rendersIdentityAvatar() {
        renderBothSchemes(TypingIndicatorView(name: "Assistant", isAgent: true, address: "assistant@localhost"))
    }
```

- [ ] **Step 4: Run the test + build.**

Run: `xcodebuild … test -only-testing:AINMMessengerTests/ConversationSurfaceRenderTests`
Expected: PASS.

- [ ] **Step 5: Revert pbxproj/scheme pollution; commit.**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/MessageThread/TypingIndicatorView.swift ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift AINMMessengerTests/DesignSystemTests/ConversationSurfaceRenderTests.swift
rtk proxy git commit -m "feat(ui): typing indicator uses identity avatar + warm tokens (SP2 follow-up #2)"
```

---

### Task CSP-7: `AgentMessageView` airy layout + serif prose (`TextBlockView`)

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/AgentMessageView.swift`
- Modify: `ios/AINMMessenger/AINMMessenger/UI/ContentBlocks/TextBlockView.swift`
- Test: append to `ConversationSurfaceRenderTests.swift`

Drop the grey card and "Agent" badge; lay the agent message open on the paper with an identity avatar, serif sender name, and serif prose. `TextBlockView` is shared with the human bubble, so set **both** branches: agent → serif `inkPrimary`; human → SF `humanBubbleText`.

- [ ] **Step 1: Replace `AgentMessageView.swift` body.**

```swift
import SwiftUI

/// Left-aligned agent / other-participant message (UI-SPEC D-01, D-02), Editorial Warm.
/// Airy: no wrapping card. Identity avatar + serif sender name on the first message of a
/// run; serif prose laid open on the paper. Rich blocks keep their own tokenized container.
struct AgentMessageView: View {
    let blocks: [ContentBlock]
    let senderName: String
    let senderType: String
    var senderAddress: String? = nil
    var showHeader: Bool = false
    var timestamp: String?
    var onAction: ((_ blockIndex: Int, _ actionId: String, _ data: [String: Any]) -> Void)? = nil

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.Spacing.xs) {
            if showHeader {
                HStack(spacing: Theme.Spacing.sm) {
                    AvatarView(
                        participantType: senderType,
                        size: 30,
                        address: senderAddress,
                        displayName: senderName
                    )
                    Text(senderName)
                        .font(Theme.Typography.senderName())
                        .foregroundStyle(Theme.Color.inkPrimary)
                }
            }
            ForEach(Array(blocks.enumerated()), id: \.offset) { index, block in
                ContentBlockRenderer(block: block, isHumanBubble: false, onAction: { actionId, data in
                    onAction?(index, actionId, data)
                })
            }
            if let timestamp {
                Text(timestamp)
                    .font(Theme.Typography.caption())
                    .foregroundStyle(Theme.Color.inkSecondary)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
```

(No `.background`/`.clipShape` — the airy treatment. `frame(maxWidth: .infinity, alignment: .leading)` keeps prose left-aligned and full-width.)

- [ ] **Step 2: Update `TextBlockView.swift`** so agent prose is serif `inkPrimary` and human stays SF on `humanBubbleText`.

```swift
import SwiftUI

/// Renders a TextBlock with Markdown (bold/italic/code/links) via AttributedString.
/// Agent prose uses the editorial serif on ink; human-bubble text uses SF on the
/// human-bubble text color.
struct TextBlockView: View {
    let content: ContentBlock.TextContent
    var isHumanBubble: Bool = false

    private var proseFont: Font { isHumanBubble ? Theme.Typography.body() : Theme.Typography.agentProse() }
    private var proseColor: Color { isHumanBubble ? Theme.Color.humanBubbleText : Theme.Color.inkPrimary }

    var body: some View {
        Group {
            if let attributed = try? AttributedString(
                markdown: content.text,
                options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
            ) {
                let styled = applyBubbleStyle(to: attributed)
                let highlighted = MentionTextRenderer.highlightMentions(
                    in: styled,
                    mentions: content.mentions,
                    isHumanBubble: isHumanBubble
                )
                Text(highlighted)
                    .font(proseFont)
                    .foregroundStyle(proseColor)
            } else {
                Text(content.text)
                    .font(proseFont)
                    .foregroundStyle(proseColor)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func applyBubbleStyle(to attributed: AttributedString) -> AttributedString {
        var result = attributed
        result.foregroundColor = isHumanBubble ? Theme.Color.humanBubbleText : Theme.Color.inkPrimary
        return result
    }
}
```

- [ ] **Step 3: Append an agent render test:**

```swift
    func test_agentMessage_rendersAiry() {
        let view = AgentMessageView(
            blocks: textBlocks("Here's a plan. I checked your calendar.", source: "agent"),
            senderName: "Assistant", senderType: "agent",
            senderAddress: "assistant@localhost", showHeader: true, timestamp: "9:41 AM"
        )
        renderBothSchemes(view)
    }
```

- [ ] **Step 4: Run the test + full unit build** (TextBlockView is shared — confirm nothing else breaks).

Run: `xcodebuild … test -only-testing:AINMMessengerTests/ConversationSurfaceRenderTests`
Expected: PASS.

- [ ] **Step 5: Revert pbxproj/scheme pollution; commit.**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/MessageThread/AgentMessageView.swift ios/AINMMessenger/AINMMessenger/UI/ContentBlocks/TextBlockView.swift AINMMessengerTests/DesignSystemTests/ConversationSurfaceRenderTests.swift
rtk proxy git commit -m "feat(ui): airy agent message + serif prose (SP2)"
```

---

### Task CSP-8: `MessageBubbleView` — ink tokens + adaptive width

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageBubbleView.swift`
- Test: append to `ConversationSurfaceRenderTests.swift`

Apply the ink-bubble tokens and **fix the non-adaptive width**: replace `UIScreen.main.bounds.width * 0.75` with a max width derived from the actual container width (correct under split-view/iPad/rotation). Read the container width via a local background `GeometryReader` into `@State` and cap the bubble at 75% of it; content shorter than the cap still shrinks to fit.

- [ ] **Step 1: Replace the file body.**

```swift
import SwiftUI

/// Right-aligned human message bubble (UI-SPEC D-01), Editorial Warm.
/// Ink background, paper text, 16pt corners with a 4pt bottom-right tail.
/// Max width adapts to the real container width (split-view/iPad/rotation safe).
struct MessageBubbleView: View {
    let blocks: [ContentBlock]
    let senderName: String
    var showHeader: Bool = false
    var timestamp: String?
    /// Stable message id for UI test targeting (scenarios 5 + 6).
    var messageId: String = ""

    @State private var containerWidth: CGFloat = 0

    var body: some View {
        HStack {
            Spacer(minLength: Theme.Spacing.xl)
            VStack(alignment: .trailing, spacing: Theme.Spacing.xs) {
                if showHeader {
                    Text(senderName)
                        .font(Theme.Typography.label())
                        .foregroundStyle(Theme.Color.humanBubbleText)
                }
                ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in
                    ContentBlockRenderer(block: block, isHumanBubble: true)
                }
                if let timestamp {
                    Text(timestamp)
                        .font(Theme.Typography.caption())
                        .foregroundStyle(Theme.Color.humanBubbleText.opacity(0.7))
                }
            }
            .padding(.horizontal, Theme.Spacing.md)
            .padding(.vertical, Theme.Spacing.sm)
            .frame(maxWidth: containerWidth > 0 ? containerWidth * 0.75 : nil, alignment: .trailing)
            .background(Theme.Color.humanBubble)
            .clipShape(UnevenRoundedRectangle(
                topLeadingRadius: Theme.Radius.bubble,
                bottomLeadingRadius: Theme.Radius.bubble,
                bottomTrailingRadius: Theme.Spacing.xs,
                topTrailingRadius: Theme.Radius.bubble
            ))
            .accessibilityIdentifier(UITestID.SubThread.messageRow(messageId: messageId))
        }
        .background {
            GeometryReader { proxy in
                Color.clear
                    .onAppear { containerWidth = proxy.size.width }
                    .onChange(of: proxy.size.width) { _, w in containerWidth = w }
            }
        }
    }
}
```

(The background `GeometryReader` measures the full-width `HStack` = the container width. `bottomTrailingRadius: Theme.Spacing.xs` reproduces the original 4pt tail.)

- [ ] **Step 2: Append a human-bubble render test:**

```swift
    func test_humanBubble_rendersInkTokens() {
        let view = MessageBubbleView(
            blocks: textBlocks("Plan my week around the launch", source: "user"),
            senderName: "You", showHeader: false, timestamp: "9:40 AM", messageId: "m0"
        )
        renderBothSchemes(view, file: #filePath, line: #line)
    }
```

- [ ] **Step 3: Run the test.**

Run: `xcodebuild … test -only-testing:AINMMessengerTests/ConversationSurfaceRenderTests`
Expected: PASS.

- [ ] **Step 4: Revert pbxproj/scheme pollution; commit.**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageBubbleView.swift AINMMessengerTests/DesignSystemTests/ConversationSurfaceRenderTests.swift
rtk proxy git commit -m "feat(ui): ink human bubble + adaptive width (SP2)"
```

---

### Task CSP-9: `ToolCallIndicatorView` warm chip + content-block container tokenization

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/ToolVisibility/ToolCallIndicatorView.swift`
- Modify: `ios/AINMMessenger/AINMMessenger/UI/ContentBlocks/CodeBlockView.swift`

Tokenize the tool-call chip surfaces and the code-block container (surface/border/radius/label colors). Keep all accessibility identifiers and lifecycle status icons unchanged. **Do not** change the structural layout or the status-icon semantics (green/red/clock) — only swap raw `Color(.systemX)`/`Color.accentColor` for tokens.

- [ ] **Step 1: In `ToolCallIndicatorView.swift`**, tokenize the collapsed-row and expanded-area surfaces. Replace:
  - line `.foregroundStyle(Color(.label))` (tool name) → `.foregroundStyle(Theme.Color.inkPrimary)`
  - duration `.foregroundStyle(Color(.secondaryLabel))` → `.foregroundStyle(Theme.Color.inkSecondary)`
  - chevron `.foregroundStyle(Color(.tertiaryLabel))` → `.foregroundStyle(Theme.Color.inkSecondary)`
  - collapsed-row `.background(Color(.tertiarySystemBackground))` → `.background(Theme.Color.accentSoft)`
  - expanded-area `.background(Color(.secondarySystemBackground))` → `.background(Theme.Color.surface)`
  - the two `cornerRadius`/`topLeadingRadius: 8` literals stay `Theme.Radius.chip` (= 8) — replace the `8` literals in both `UnevenRoundedRectangle` blocks with `Theme.Radius.chip`.

Leave `statusIcon` (`.systemOrange`/`.systemGreen`/`Color.red`/`.secondaryLabel`) as-is — those are semantic lifecycle signals, not theming.

- [ ] **Step 2: In `CodeBlockView.swift`**, tokenize the container + labels:
  - language label `.foregroundStyle(.secondary)` → `.foregroundStyle(Theme.Color.inkSecondary)`
  - Copy button `.foregroundStyle(Color.accentColor)` → `.foregroundStyle(Theme.Color.accent)`
  - `.background(Color(.tertiarySystemBackground))` → `.background(Theme.Color.surface)`
  - `.clipShape(RoundedRectangle(cornerRadius: 8))` → `.clipShape(RoundedRectangle(cornerRadius: Theme.Radius.chip))`
  - add a hairline border to match the editorial surfaces: after `.clipShape(…)` append
    `.overlay { RoundedRectangle(cornerRadius: Theme.Radius.chip).stroke(Theme.Color.surfaceBorder, lineWidth: 1) }`

- [ ] **Step 3: Build to confirm both compile.**

Run: `xcodebuild … build`
Expected: BUILD SUCCEEDED.

- [ ] **Step 4: Revert pbxproj/scheme pollution; commit.**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/ToolVisibility/ToolCallIndicatorView.swift ios/AINMMessenger/AINMMessenger/UI/ContentBlocks/CodeBlockView.swift
rtk proxy git commit -m "feat(ui): warm tool-call chip + tokenized code block (SP2)"
```

---

### Task CSP-10: `ComposerView` tokens + `Haptics.send()`; `MessageThreadView` paper bg + `Haptics.receive()`

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/Composer/ComposerView.swift`
- Modify: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift`

Tokenize the composer, fire `Haptics.send()` on send, give the thread a paper background, and fire `Haptics.receive()` when a new stream lands.

- [ ] **Step 1: In `ComposerView.swift`**, tokenize + wire send haptic.
  - TextField `.font(.body)` → `.font(Theme.Typography.body())`
  - send icon `.foregroundStyle(canSend ? Color.accentColor : .secondary)` → `.foregroundStyle(canSend ? Theme.Color.accent : Theme.Color.inkSecondary)`
  - container `.background(Color(.systemBackground))` → `.background(Theme.Color.paper)`
  - In the send `Button` action, immediately after the `guard !trimmed.isEmpty else { return }` line, add `Haptics.send()` so the tap that actually sends gives feedback:

```swift
            Button {
                let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !trimmed.isEmpty else { return }
                Haptics.send()
                if let onSendWithMentions, !mentions.isEmpty {
                    onSendWithMentions(trimmed, mentions)
                } else {
                    onSend(trimmed)
                }
                text = ""
                mentions = []
                showMentionPopup = false
            } label: {
```

  - Leave the `TextField` placeholder/offline string, the `.overlay { Divider() }`, and all accessibility identifiers unchanged. (The `Divider()` may optionally be `.overlay(alignment: .top) { Theme.Color.separator.frame(height: 1) }` — keep `Divider()` to avoid touching layout; tokenizing it is not required.)

- [ ] **Step 2: In `MessageThreadView.swift`**, set the paper background and wire the receive haptic.
  - Give the scrolling surface a paper background. On the `ScrollView { … }` (the one containing the `LazyVStack`), append `.background(Theme.Color.paper)` and also set the outer `VStack(spacing: 0)`'s background to paper so banners/composer sit on the same paper:
    add `.background(Theme.Color.paper)` to the root `VStack(spacing: 0) { … }` (place it alongside the existing `.overlay`/`.toolbar` modifiers, e.g. right after `.navigationBarTitleDisplayMode(.inline)`).
  - Wire `Haptics.receive()` when a new stream appears for this thread. Add this modifier to the root `VStack` (near the other `.onChange`/`.onAppear` modifiers):

```swift
        .onChange(of: appState.streamingViewModel.activeStreamsForThread(threadId).count) { oldCount, newCount in
            if newCount > oldCount { Haptics.receive() }
        }
```

(Firing on a newly-appearing stream is the agent-native "message landing" moment and is a fire-and-forget side effect — no view-model change. `Haptics` is `@MainActor`; this closure runs on the main actor.)

- [ ] **Step 3: Build to confirm it compiles** (UI behavior unchanged; verify the existing Mention UITest identifiers still resolve).

Run: `xcodebuild … build`
Expected: BUILD SUCCEEDED.

- [ ] **Step 4: Revert pbxproj/scheme pollution; commit.**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/Composer/ComposerView.swift ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift
rtk proxy git commit -m "feat(ui): tokenized composer + paper thread bg + haptics on send/receive (SP2)"
```

---

### Task CSP-11: `ConversationListView` / `ConversationRowView` — tokens + identity avatar

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/ConversationList/ConversationRowView.swift`
- Modify: `ios/AINMMessenger/AINMMessenger/UI/ConversationList/ConversationListView.swift`
- Test: append to `ConversationSurfaceRenderTests.swift`

Give rows an identity-colored avatar (falls back to the legacy avatar when no agent address is resolvable), serif title, and warm preview/timestamp/unread tokens.

- [ ] **Step 1: Check whether `ConversationRow` exposes an agent address.** Inspect the `ConversationRow` model:

Run: `grep -rn "struct ConversationRow" ios/AINMMessenger/AINMMessenger`
Then read that struct's fields.
  - **If it has an agent-address-like field** (e.g. `agentAddress`, `counterpartAddress`): pass it as `address` to `AvatarView`, with `displayName: conversation.title`.
  - **If it does not:** pass `address: nil` (legacy avatar path — nothing regresses, per spec §8). Do **not** add new model/query plumbing in SP2.

- [ ] **Step 2: Replace `ConversationRowView.swift` body** (using whichever address expression Step 1 determined — shown here with the nil-safe fallback; substitute the real field if it exists):

```swift
import SwiftUI

/// Single conversation row (UI-SPEC Screen 1), Editorial Warm.
/// Identity-colored avatar (legacy fallback when no address), serif title,
/// warm preview/timestamp, accent unread dot.
struct ConversationRowView: View {
    let conversation: ConversationRow
    let previewText: String
    let relativeTime: String

    var body: some View {
        HStack(spacing: Theme.Spacing.md) {
            AvatarView(
                participantType: "agent",
                size: 36,
                address: rowAddress,
                displayName: conversation.title
            )

            VStack(alignment: .leading, spacing: Theme.Spacing.xs) {
                HStack {
                    Text(conversation.title ?? "Untitled")
                        .font(Theme.Typography.senderName())
                        .foregroundStyle(Theme.Color.inkPrimary)
                        .lineLimit(1)
                    Spacer()
                    Text(relativeTime)
                        .font(Theme.Typography.caption())
                        .foregroundStyle(Theme.Color.inkSecondary)
                }
                HStack {
                    Text(previewText)
                        .font(Theme.Typography.caption())
                        .foregroundStyle(Theme.Color.inkSecondary)
                        .lineLimit(2)
                    Spacer(minLength: 0)
                    if conversation.hasUnread {
                        Circle()
                            .fill(Theme.Color.accent)
                            .frame(width: 8, height: 8)
                            .accessibilityLabel("Unread")
                    }
                }
            }
        }
        .padding(.vertical, Theme.Spacing.sm)
        .padding(.horizontal, Theme.Spacing.lg)
        .frame(minHeight: 64)
        .contentShape(Rectangle())
    }

    /// Address driving the identity avatar; nil → legacy avatar (no regression).
    private var rowAddress: String? {
        // Substitute the real ConversationRow agent-address field here if Step 1 found one.
        nil
    }
}
```

- [ ] **Step 2b: If Step 1 found a real address field**, set `rowAddress` to return it (e.g. `conversation.agentAddress`). Otherwise leave `nil`.

- [ ] **Step 3: In `ConversationListView.swift`**, tokenize the list + empty state:
  - empty-state icon `.foregroundStyle(Color.accentColor)` → `.foregroundStyle(Theme.Color.accent)`
  - empty-state title `.font(.title2.bold())` → `.font(Theme.Typography.displayTitle())` + `.foregroundStyle(Theme.Color.inkPrimary)`
  - empty-state subtitle `.font(.subheadline)` `.foregroundStyle(.secondary)` → `.font(Theme.Typography.body())` `.foregroundStyle(Theme.Color.inkSecondary)`
  - add a paper backdrop: on the `List { … }`, append `.scrollContentBackground(.hidden)` and `.background(Theme.Color.paper)`; on the empty-state `VStack`, append `.frame(maxWidth: .infinity, maxHeight: .infinity).background(Theme.Color.paper)`.
  - Leave the "Start a Conversation" `.buttonStyle(.borderedProminent)` as-is (uses the app accent), and keep all `accessibilityIdentifier`s and the `.listRowSeparator(.visible)` behavior.

- [ ] **Step 4: Append a conversation-row render test:**

```swift
    func test_conversationRow_rendersWarmTokens() {
        // Build a minimal ConversationRow via its real initializer; adjust field names to match.
        // If the model is not trivially constructible in tests, skip this test and rely on the
        // DEBUG catalog/#Preview for visual review (note it in the task summary).
    }
```

If `ConversationRow` is trivially constructible, fill in a real instance and call `renderBothSchemes(ConversationRowView(conversation: row, previewText: "Sounds good", relativeTime: "9:41 AM"))`. If not, leave the test body empty with the explanatory comment above and rely on previews.

- [ ] **Step 5: Build + run render tests.**

Run: `xcodebuild … test -only-testing:AINMMessengerTests/ConversationSurfaceRenderTests`
Expected: PASS.

- [ ] **Step 6: Revert pbxproj/scheme pollution; commit.**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/ConversationList/ConversationRowView.swift ios/AINMMessenger/AINMMessenger/UI/ConversationList/ConversationListView.swift AINMMessengerTests/DesignSystemTests/ConversationSurfaceRenderTests.swift
rtk proxy git commit -m "feat(ui): conversation list/row warm tokens + identity avatar (SP2)"
```

---

### Task CSP-12: Reduce-motion test, optional snapshots, full-suite verification + DoD

**Files:**
- Modify: `AINMMessengerTests/DesignSystemTests/ConversationSurfaceRenderTests.swift`
- (SNAPSHOT_LIB path only) Create: `AINMMessengerTests/DesignSystemTests/ConversationSurfaceSnapshotTests.swift` + committed reference images
- (If any avatar accessibility label changed) Modify: the specific UITest assertion(s)

Close out: add a reduce-motion render test, layer image snapshots if CSP-2 chose `SNAPSHOT_LIB`, fix any UITest label drift, and run the full suites.

- [ ] **Step 1: Append a reduce-motion render test** (caret + typing dots must be static under reduce-motion — they render without animation, so this asserts no crash with the trait set):

```swift
    func test_streamingAndTyping_renderUnderReduceMotion() {
        let state = StreamingViewModel.StreamingMessageState(
            id: "s2", messageId: "m2", from: "researcher@acme.test", threadId: "t1",
            blocks: [.init(type: "text", content: "Calculating…")],
            status: .streaming, finalizingStartTime: nil
        )
        let stack = VStack {
            StreamingMessageView(state: state)
            TypingIndicatorView(name: "Researcher", isAgent: true, address: "researcher@acme.test")
        }
        .environment(\.accessibilityReduceMotion, true)
        renderBothSchemes(stack, file: #filePath, line: #line)
    }
```

- [ ] **Step 2 (SNAPSHOT_LIB path only): add image snapshots** for the key surfaces in light + dark, fixed size, in a new `ConversationSurfaceSnapshotTests.swift` using `import SnapshotTesting`. Record references on first run (`isRecording = true`), then commit images and flip recording off. Surfaces: human bubble, airy agent message, streaming message (caret+halo+status), typing indicator, tool-call chip, composer (enabled/disabled/offline), conversation row.

  - **RENDER_SMOKE path:** skip image snapshots entirely — the per-surface render/smoke tests + DEBUG catalog/`#Preview`s are the visual-regression coverage (spec §7 fallback). Note this in the summary.

- [ ] **Step 3: Run the full unit suite.**

Run: `xcodebuild … test`
Expected: PASS (all existing + new tests).

- [ ] **Step 4: Run the UITest suite** (only avatars' accessibility labels may have shifted from "Agent avatar" to "<name> avatar" where identity wiring was added — typing indicator, conversation row):

Run: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessengerUITests -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test`
Expected: PASS. If a test asserts an avatar label that changed, update **only** that assertion to the new "<name> avatar" string (`AvatarView.accessibilityText` returns `"\(displayName) avatar"` when a displayName is present). Do not weaken unrelated assertions.

- [ ] **Step 5: DoD audit** — confirm each spec §9 item, grepping for stray raw colors/fonts in the restyled files:

```bash
grep -rn "Color.accentColor\|secondarySystemGroupedBackground\|UIScreen.main.bounds\|\.systemBackground" \
  ios/AINMMessenger/AINMMessenger/UI/MessageThread \
  ios/AINMMessenger/AINMMessenger/UI/Composer \
  ios/AINMMessenger/AINMMessenger/UI/ConversationList
```
Expected: no matches in the SP2-restyled files (status-icon semantic colors in `ToolCallIndicatorView` are allowed and live in `UI/ToolVisibility`). Manually confirm: airy agent message (no card); streaming/typing/settled all identity-colored (no orange flip); `Identity0` recolored + golden tests green; clipping-safe halo covered by a test; haptics wired; adaptive bubble width.

- [ ] **Step 6: Final clean build + revert any pbxproj/scheme pollution; commit.**

```bash
rtk proxy git status   # revert .pbxproj/.xcscheme if only xcodebuild-normalized
rtk proxy git add AINMMessengerTests/DesignSystemTests/
# (SNAPSHOT_LIB) also add ConversationSurfaceSnapshotTests.swift + __Snapshots__ images
rtk proxy git commit -m "test(ui): SP2 reduce-motion + snapshot/render coverage; full suites green (SP2 DoD)"
```

- [ ] **Step 7: Hand off to finishing-a-development-branch** — present the merge/PR/keep/discard options. (Pushing to origin remains a separate user-gated decision.)

---

## Self-Review

**Spec coverage (§9 DoD ↔ tasks):**
1. All surfaces consume `Theme` tokens, light+dark → CSP-5..CSP-11; audited CSP-12 §5. ✓
2. Airy agent message; settled==streaming except live affordances → CSP-7 (settled) + CSP-5 (streaming), shared identity inputs. ✓
3. Avatar color-flip gone → CSP-5 (streaming) + CSP-6 (typing) thread `address`; CSP-7 settled already identity. ✓
4. `Identity0` recolored; golden tests pass → CSP-1. ✓
5. Clipping-safe halo + test → CSP-4. ✓
6. `Haptics.send()` on send; `Haptics.receive()` on stream arrival → CSP-10. ✓
7. Adaptive bubble width → CSP-8. ✓
8. Snapshot/render coverage light+dark; suites green; clean build; committed via `rtk proxy git` → CSP-2 harness, per-task render tests, CSP-12 finalize. ✓

**Placeholder scan:** No "TBD"/"TODO" left undefined. The only conditional is CSP-11's `rowAddress` (explicit Step-1 inspection → real field or `nil` fallback) and CSP-2's `SNAPSHOT_LIB`/`RENDER_SMOKE` verdict — both have fully-specified branches.

**Type consistency:** `CursorView(height:color:)` defined CSP-3, used CSP-5. `AvatarView(participantType:size:address:displayName:showAgentHalo:)` matches the SP1 signature, halo body replaced CSP-4. `TypingIndicatorView(name:isAgent:address:)` defined CSP-6, call site updated same task. `StreamingMessageState(id:messageId:from:threadId:blocks:status:finalizingStartTime:)` and `StreamingBlockState(type:content:language:)` match the source. `Theme.Typography.*()` called with parens throughout. `ContentBlock.TextContent(version:source:text:mentions:fallback:)` matches the real initializer.

**Out of scope confirmed:** no A2A `→ @name` tags, no multi-agent treatment, no digest capsule, no memory/onboarding surfaces, no data-model/network/view-model changes.
