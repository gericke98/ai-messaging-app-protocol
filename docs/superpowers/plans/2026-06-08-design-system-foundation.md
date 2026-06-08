# Design System Foundation (SP1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce a centralized, testable Editorial-Warm design system (semantic colors light+dark, New York serif type scale, spacing/radius/motion tokens, haptics, a deterministic per-identity color engine) and refactor `AvatarView` to consume it so multiple participants become visually distinct.

**Architecture:** A new self-contained module `UI/DesignSystem/` holding an `Assets.xcassets` color catalog (source of truth for color), a pure `Theme` token namespace, a `Haptics` effect wrapper, and a pure deterministic `ParticipantColor` engine. `AvatarView` gains optional `address`/`displayName`/`showAgentHalo` inputs and renders identity-colored initials when an address is supplied, preserving prior behavior otherwise. No bubble/composer/list restyle (SP2), no A2A directional tags (SP3).

**Tech Stack:** SwiftUI (iOS 18+), XCTest, `@testable import AINMMessenger`. The Xcode project uses synchronized folder groups (`PBXFileSystemSynchronizedRootGroup`), so files and the asset catalog placed under the app folder are auto-included in the target — no `.pbxproj` editing needed.

---

## Conventions for every task

- **Repo root:** `/Users/santiagogerickeparga/Proyectos/ai-native-messaging` (paths below are relative to it).
- **App source root:** `ios/AINMMessenger/AINMMessenger/`
- **Test source root:** `ios/AINMMessenger/AINMMessengerTests/`
- **Build/test command (unit tests):**
  ```bash
  xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj \
    -scheme AINMMessenger \
    -destination 'platform=iOS Simulator,name=iPhone 16' \
    -derivedDataPath /tmp/ainm-dd \
    test -only-testing:AINMMessengerTests/<ClassName>
  ```
  (Drop `-only-testing` to run the whole suite. UITests run under `-scheme AINMMessengerUITests`.)
- **Commit command:** use `rtk proxy git` (never bare git), commit directly to `main`:
  ```bash
  rtk proxy git add <paths> && rtk proxy git commit -m "<msg>"
  ```
  End every commit message body with:
  `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`
- **Do NOT push** — pushing is a separate, user-gated step at the end of the whole sub-project.

---

## Task 1: Asset catalog — semantic + identity colors (light + dark)

Creates the `Assets.xcassets` color catalog (the color source of truth) and a test asserting every color resolves at runtime. This is the only task whose "test first" is an integration check (the catalog is data, not code): write the resolution test, watch it fail (no catalog), generate the catalog, watch it pass.

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/UI/DesignSystem/Assets.xcassets/` (catalog + 20 color sets)
- Create (temporary generator, deleted at end of task): `ios/AINMMessenger/AINMMessenger/UI/DesignSystem/_gen_colors.sh`
- Create: `ios/AINMMessenger/AINMMessenger/UI/DesignSystem/Bundle+DesignSystem.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/AssetColorTests.swift`

- [ ] **Step 1: Add the bundle marker** (so colors resolve from the right bundle in both app and tests)

Create `ios/AINMMessenger/AINMMessenger/UI/DesignSystem/Bundle+DesignSystem.swift`:

```swift
import Foundation

/// Marker class whose bundle locates the DesignSystem asset catalog.
/// Using a class from the app module makes `Bundle.designSystem` resolve correctly
/// from both the app target and the (app-hosted) unit test target.
final class DesignSystemBundleMarker {}

extension Bundle {
    static let designSystem = Bundle(for: DesignSystemBundleMarker.self)
}
```

- [ ] **Step 2: Write the failing asset-resolution test**

Create `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/AssetColorTests.swift`:

```swift
import XCTest
import UIKit
@testable import AINMMessenger

final class AssetColorTests: XCTestCase {
    private let semantic = [
        "Paper", "Surface", "SurfaceBorder", "InkPrimary", "InkSecondary",
        "HumanBubble", "HumanBubbleText", "Accent", "AccentSoft", "Separator",
    ]

    func testAllSemanticColorsResolve() {
        for name in semantic {
            XCTAssertNotNil(
                UIColor(named: name, in: .designSystem, compatibleWith: nil),
                "Missing semantic color asset: \(name)"
            )
        }
    }

    func testAllIdentityColorsResolve() {
        for i in 0..<10 {
            XCTAssertNotNil(
                UIColor(named: "Identity\(i)", in: .designSystem, compatibleWith: nil),
                "Missing identity color asset: Identity\(i)"
            )
        }
    }
}
```

- [ ] **Step 3: Run the test, verify it fails**

Run:
```bash
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/AssetColorTests
```
Expected: FAIL — assertions fire ("Missing semantic color asset: Paper") because the catalog does not exist yet. (If the project fails to compile because `Bundle+DesignSystem.swift` isn't picked up, confirm the file is under the synchronized app folder; no `.pbxproj` edit should be needed.)

- [ ] **Step 4: Create the catalog generator script**

Create `ios/AINMMessenger/AINMMessenger/UI/DesignSystem/_gen_colors.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail
# Generates Assets.xcassets color sets (light + dark) from the table below.
# Run from repo root: bash ios/AINMMessenger/AINMMessenger/UI/DesignSystem/_gen_colors.sh
CAT="ios/AINMMessenger/AINMMessenger/UI/DesignSystem/Assets.xcassets"
rm -rf "$CAT"
mkdir -p "$CAT"
printf '{\n  "info" : {\n    "author" : "xcode",\n    "version" : 1\n  }\n}\n' > "$CAT/Contents.json"

emit() { # name  Lr Lg Lb  Dr Dg Db   (hex pairs, no 0x prefix)
  local name="$1" lr="$2" lg="$3" lb="$4" dr="$5" dg="$6" db="$7"
  local dir="$CAT/$name.colorset"
  mkdir -p "$dir"
  cat > "$dir/Contents.json" <<JSON
{
  "colors" : [
    {
      "color" : {
        "color-space" : "srgb",
        "components" : { "alpha" : "1.000", "blue" : "0x$lb", "green" : "0x$lg", "red" : "0x$lr" }
      },
      "idiom" : "universal"
    },
    {
      "appearances" : [ { "appearance" : "luminosity", "value" : "dark" } ],
      "color" : {
        "color-space" : "srgb",
        "components" : { "alpha" : "1.000", "blue" : "0x$db", "green" : "0x$dg", "red" : "0x$dr" }
      },
      "idiom" : "universal"
    }
  ],
  "info" : { "author" : "xcode", "version" : 1 }
}
JSON
}

# Semantic tokens          Lr Lg Lb   Dr Dg Db
emit Paper           FA F6 F0   1A 17 14
emit Surface         FF FF FF   24 20 1B
emit SurfaceBorder   EC E2 D4   3A 33 2B
emit InkPrimary      2A 24 1D   F2 EC E2
emit InkSecondary    6B 5D 49   A9 9A 86
emit HumanBubble     1F 1B 16   3A 33 2B
emit HumanBubbleText FA F6 F0   F2 EC E2
emit Accent          C2 41 0C   E8 74 3A
emit AccentSoft      FD F0 E4   3A 2A 1E
emit Separator       E6 DC CB   3A 33 2B

# Per-identity palette (index order is load-bearing — matches Theme.Color.identityPalette)
emit Identity0       C2 41 0C   E8 74 3A
emit Identity1       0F 76 6E   2D D4 BF
emit Identity2       B4 53 09   F5 9E 0B
emit Identity3       BE 12 3C   FB 71 85
emit Identity4       6D 28 D9   A7 8B FA
emit Identity5       4D 7C 0F   A3 E6 35
emit Identity6       9A 34 12   FD BA 74
emit Identity7       1D 4E D8   60 A5 FA
emit Identity8       A2 1C AF   E8 79 F9
emit Identity9       15 80 3D   4A DE 80

echo "Generated $CAT with $(find "$CAT" -name Contents.json | wc -l) Contents.json files (expect 21: 1 catalog + 20 colorsets)."
```

- [ ] **Step 5: Run the generator and verify output**

Run:
```bash
bash ios/AINMMessenger/AINMMessenger/UI/DesignSystem/_gen_colors.sh
```
Expected: prints "Generated ... 21 Contents.json files (expect 21 ...)". Confirm with:
```bash
find ios/AINMMessenger/AINMMessenger/UI/DesignSystem/Assets.xcassets -name Contents.json | wc -l
```
Expected: `21`.

- [ ] **Step 6: Delete the generator (the catalog is the artifact, not the script)**

Run:
```bash
rm ios/AINMMessenger/AINMMessenger/UI/DesignSystem/_gen_colors.sh
```

- [ ] **Step 7: Run the resolution test, verify it passes**

Run:
```bash
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/AssetColorTests
```
Expected: PASS (both tests). If a color fails to resolve, confirm the `.xcassets` folder was auto-added to the target (synchronized groups) — open the project once in Xcode if needed, but it should not be required.

- [ ] **Step 8: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/DesignSystem/Assets.xcassets \
  ios/AINMMessenger/AINMMessenger/UI/DesignSystem/Bundle+DesignSystem.swift \
  ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/AssetColorTests.swift
rtk proxy git commit -m "$(printf 'feat(sp1): Editorial Warm asset catalog (semantic + identity colors, light+dark)\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>')"
```

---

## Task 2: Theme token namespace

Adds `Theme` — the typed Swift surface over the catalog plus typography/spacing/radius/motion tokens. New York serif via `design: .serif`; SF for body. No raw point sizes (text styles only, so Dynamic Type works).

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/UI/DesignSystem/Theme.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/ThemeTokenTests.swift`

- [ ] **Step 1: Write the failing test**

Create `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/ThemeTokenTests.swift`:

```swift
import XCTest
import SwiftUI
@testable import AINMMessenger

final class ThemeTokenTests: XCTestCase {
    func testSpacingScale() {
        XCTAssertEqual(Theme.Spacing.xs, 4)
        XCTAssertEqual(Theme.Spacing.sm, 8)
        XCTAssertEqual(Theme.Spacing.md, 12)
        XCTAssertEqual(Theme.Spacing.lg, 16)
        XCTAssertEqual(Theme.Spacing.xl, 20)
        XCTAssertEqual(Theme.Spacing.xxl, 24)
    }

    func testRadiusScale() {
        XCTAssertEqual(Theme.Radius.chip, 8)
        XCTAssertEqual(Theme.Radius.card, 12)
        XCTAssertEqual(Theme.Radius.bubble, 16)
    }

    func testIdentityPaletteHasTenColors() {
        XCTAssertEqual(Theme.Color.identityPalette.count, 10)
    }
}
```

- [ ] **Step 2: Run the test, verify it fails**

Run:
```bash
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/ThemeTokenTests
```
Expected: FAIL to compile ("cannot find 'Theme' in scope").

- [ ] **Step 3: Implement `Theme`**

Create `ios/AINMMessenger/AINMMessenger/UI/DesignSystem/Theme.swift`:

```swift
import SwiftUI

/// Central design-token namespace for the Editorial Warm system.
/// Colors are backed by Assets.xcassets (light + dark); typography uses the
/// system serif (New York) for the editorial voice and SF for body text.
enum Theme {

    // MARK: - Color
    enum Color {
        private static func ds(_ name: String) -> SwiftUI.Color {
            SwiftUI.Color(name, bundle: .designSystem)
        }
        static var paper: SwiftUI.Color           { ds("Paper") }
        static var surface: SwiftUI.Color         { ds("Surface") }
        static var surfaceBorder: SwiftUI.Color   { ds("SurfaceBorder") }
        static var inkPrimary: SwiftUI.Color      { ds("InkPrimary") }
        static var inkSecondary: SwiftUI.Color    { ds("InkSecondary") }
        static var humanBubble: SwiftUI.Color     { ds("HumanBubble") }
        static var humanBubbleText: SwiftUI.Color { ds("HumanBubbleText") }
        static var accent: SwiftUI.Color          { ds("Accent") }
        static var accentSoft: SwiftUI.Color      { ds("AccentSoft") }
        static var separator: SwiftUI.Color       { ds("Separator") }

        /// Per-identity palette. Index order MUST match Identity0…9 in the asset catalog.
        static var identityPalette: [SwiftUI.Color] {
            (0..<10).map { ds("Identity\($0)") }
        }
    }

    // MARK: - Typography (text styles only — Dynamic Type preserved)
    enum Typography {
        static func displayLarge() -> Font { .system(.largeTitle, design: .serif).weight(.semibold) }
        static func displayTitle() -> Font { .system(.title2, design: .serif).weight(.semibold) }
        static func senderName()  -> Font  { .system(.subheadline, design: .serif).weight(.semibold) }
        static func agentProse()  -> Font  { .system(.body, design: .serif) }
        static func body()        -> Font  { .system(.body) }
        static func callout()     -> Font  { .system(.callout) }
        static func label()       -> Font  { .system(.caption, design: .default).weight(.semibold) }
        static func caption()     -> Font  { .system(.caption2) }
    }

    // MARK: - Spacing
    enum Spacing {
        static let xs: CGFloat = 4
        static let sm: CGFloat = 8
        static let md: CGFloat = 12
        static let lg: CGFloat = 16
        static let xl: CGFloat = 20
        static let xxl: CGFloat = 24
    }

    // MARK: - Radius
    enum Radius {
        static let chip: CGFloat = 8
        static let card: CGFloat = 12
        static let bubble: CGFloat = 16
    }

    // MARK: - Motion
    enum Motion {
        static let messageAppear = Animation.spring(response: 0.34, dampingFraction: 0.82)
        static let subtle = Animation.easeInOut(duration: 0.2)
    }
}
```

- [ ] **Step 4: Run the test, verify it passes**

Run:
```bash
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/ThemeTokenTests
```
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/DesignSystem/Theme.swift \
  ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/ThemeTokenTests.swift
rtk proxy git commit -m "$(printf 'feat(sp1): Theme token namespace (color/typography/spacing/radius/motion)\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>')"
```

---

## Task 3: ParticipantColor engine (pure, deterministic)

The A2A foundation: a stable address→palette-slot mapping (FNV-1a, not Swift's per-process `Hasher`) plus initials derivation. Fully unit-tested; golden indices below were computed offline against the FNV-1a 32-bit algorithm.

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/UI/DesignSystem/ParticipantColor.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/ParticipantColorTests.swift`

- [ ] **Step 1: Write the failing tests**

Create `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/ParticipantColorTests.swift`:

```swift
import XCTest
@testable import AINMMessenger

final class ParticipantColorTests: XCTestCase {

    // Golden FNV-1a 32-bit indices (count = 10), computed offline. These lock the algorithm:
    // a change to the hash would change these and fail the test.
    func testPaletteIndexGoldenValues() {
        XCTAssertEqual(ParticipantColor.paletteIndex(for: "assistant@localhost", count: 10), 8)
        XCTAssertEqual(ParticipantColor.paletteIndex(for: "researcher@acme.test", count: 10), 5)
    }

    func testPaletteIndexIsDeterministic() {
        for _ in 0..<100 {
            XCTAssertEqual(
                ParticipantColor.paletteIndex(for: "sam@home.test", count: 10),
                ParticipantColor.paletteIndex(for: "sam@home.test", count: 10)
            )
        }
    }

    func testPaletteIndexInRange() {
        for i in 0..<200 {
            let idx = ParticipantColor.paletteIndex(for: "user\(i)@host.test", count: 10)
            XCTAssertTrue((0..<10).contains(idx))
        }
    }

    func testDistributionCoversManySlots() {
        var slots = Set<Int>()
        for i in 0..<50 {
            slots.insert(ParticipantColor.paletteIndex(for: "user\(i)@host\(i % 7).test", count: 10))
        }
        XCTAssertGreaterThanOrEqual(slots.count, 6, "Hash distribution is degenerate: \(slots)")
    }

    func testInitials() {
        XCTAssertEqual(ParticipantColor.initials(for: "Sam Rivera"), "SR")
        XCTAssertEqual(ParticipantColor.initials(for: "assistant"), "A")
        XCTAssertEqual(ParticipantColor.initials(for: "researcher@acme.test"), "R")
        XCTAssertEqual(ParticipantColor.initials(for: "Sam"), "S")
        XCTAssertEqual(ParticipantColor.initials(for: ""), "?")
        XCTAssertEqual(ParticipantColor.initials(for: "   "), "?")
        XCTAssertEqual(ParticipantColor.initials(for: "🙂"), "?")
    }
}
```

- [ ] **Step 2: Run the tests, verify they fail**

Run:
```bash
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/ParticipantColorTests
```
Expected: FAIL to compile ("cannot find 'ParticipantColor' in scope").

- [ ] **Step 3: Implement `ParticipantColor`**

Create `ios/AINMMessenger/AINMMessenger/UI/DesignSystem/ParticipantColor.swift`:

```swift
import SwiftUI

/// Deterministic per-identity color + initials. Pure and stable across processes/devices
/// (uses an explicit FNV-1a hash, NOT Swift's seed-randomized Hasher).
enum ParticipantColor {

    /// Stable palette index for an address in 0..<count.
    static func paletteIndex(for address: String, count: Int) -> Int {
        precondition(count > 0, "palette count must be positive")
        var hash: UInt32 = 2166136261          // FNV-1a 32-bit offset basis
        for byte in address.utf8 {
            hash ^= UInt32(byte)
            hash = hash &* 16777619            // FNV prime, wrapping multiply
        }
        return Int(hash % UInt32(count))
    }

    /// The SwiftUI Color for an address, drawn from Theme.Color.identityPalette.
    static func color(for address: String) -> Color {
        let palette = Theme.Color.identityPalette
        return palette[paletteIndex(for: address, count: palette.count)]
    }

    /// Up to two uppercase initials. For an email-like address with no spaces,
    /// derives from the local-part (before "@"). Never returns empty ("?" fallback).
    static func initials(for displayName: String) -> String {
        let isBareAddress = displayName.contains("@") && !displayName.contains(" ")
        let source = isBareAddress
            ? String(displayName.split(separator: "@").first ?? "")
            : displayName

        let words = source
            .split(whereSeparator: { $0 == " " || $0 == "." || $0 == "_" || $0 == "-" })
            .filter { $0.contains(where: \.isLetter) }

        let letters = words.prefix(2).compactMap { $0.first(where: \.isLetter) }
        if !letters.isEmpty {
            return letters.map { String($0).uppercased() }.joined()
        }
        if let c = source.first(where: \.isLetter) {
            return String(c).uppercased()
        }
        return "?"
    }
}
```

- [ ] **Step 4: Run the tests, verify they pass**

Run:
```bash
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/ParticipantColorTests
```
Expected: PASS (all tests). If `testPaletteIndexGoldenValues` fails, do NOT change the goldens — the implementation diverges from FNV-1a; fix the implementation.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/DesignSystem/ParticipantColor.swift \
  ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/ParticipantColorTests.swift
rtk proxy git commit -m "$(printf 'feat(sp1): deterministic ParticipantColor engine (FNV-1a + initials)\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>')"
```

---

## Task 4: Haptics wrapper

Thin, fire-and-forget feedback helpers. SP1 only defines them (wiring into send/receive is SP2). Test asserts the calls don't crash.

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/UI/DesignSystem/Haptics.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/HapticsTests.swift`

- [ ] **Step 1: Write the failing test**

Create `ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/HapticsTests.swift`:

```swift
import XCTest
@testable import AINMMessenger

final class HapticsTests: XCTestCase {
    @MainActor
    func testHapticCallsDoNotCrash() {
        Haptics.send()
        Haptics.receive()
        Haptics.select()
        Haptics.success()
    }
}
```

- [ ] **Step 2: Run the test, verify it fails**

Run:
```bash
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/HapticsTests
```
Expected: FAIL to compile ("cannot find 'Haptics' in scope").

- [ ] **Step 3: Implement `Haptics`**

Create `ios/AINMMessenger/AINMMessenger/UI/DesignSystem/Haptics.swift`:

```swift
import UIKit

/// Fire-and-forget haptic feedback. No-ops gracefully where unavailable (e.g. Simulator).
@MainActor
enum Haptics {
    static func send() {
        let g = UIImpactFeedbackGenerator(style: .light)
        g.prepare(); g.impactOccurred()
    }
    static func receive() {
        let g = UIImpactFeedbackGenerator(style: .soft)
        g.prepare(); g.impactOccurred()
    }
    static func select() {
        let g = UISelectionFeedbackGenerator()
        g.prepare(); g.selectionChanged()
    }
    static func success() {
        let g = UINotificationFeedbackGenerator()
        g.prepare(); g.notificationOccurred(.success)
    }
}
```

- [ ] **Step 4: Run the test, verify it passes**

Run:
```bash
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerTests/HapticsTests
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/DesignSystem/Haptics.swift \
  ios/AINMMessenger/AINMMessengerTests/DesignSystemTests/HapticsTests.swift
rtk proxy git commit -m "$(printf 'feat(sp1): Haptics feedback wrapper\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>')"
```

---

## Task 5: AvatarView refactor (identity color + initials)

Refactor the one shared avatar view to consume `ParticipantColor`, adding optional `address`/`displayName`/`showAgentHalo` while preserving the existing `participantType` behavior so all 8 call sites keep compiling. Then thread the identity through the one most-visible, non-restyling call site (the message-thread header in `AgentMessageView`).

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/Shared/AvatarView.swift` (full replacement below)
- Modify: `ios/AINMMessenger/AINMMessenger/UI/MessageThread/AgentMessageView.swift:14-18` (pass address/displayName)

Reference — the current `AvatarView.swift` to replace:
```swift
struct AvatarView: View {
    let participantType: String  // "human" or "agent"
    var size: CGFloat = 36
    var body: some View {
        Circle()
            .fill(participantType == "agent" ? Color.orange.opacity(0.15) : Color.blue.opacity(0.15))
            .frame(width: size, height: size)
            .overlay {
                Image(systemName: participantType == "agent" ? "cpu.fill" : "person.fill")
                    .font(.system(size: size * 0.45))
                    .foregroundStyle(participantType == "agent" ? .orange : .blue)
            }
            .accessibilityLabel(participantType == "agent" ? "Agent avatar" : "Human avatar")
    }
}
```

- [ ] **Step 1: Replace `AvatarView.swift` with the identity-aware version**

Overwrite `ios/AINMMessenger/AINMMessenger/UI/Shared/AvatarView.swift`:

```swift
import SwiftUI

/// Reusable avatar circle. When an `address` is supplied, renders a per-identity
/// colored circle with initials (so distinct participants are visually separable).
/// Without an address, preserves the prior type-tinted symbol behavior so existing
/// call sites are unchanged.
struct AvatarView: View {
    let participantType: String       // "human" or "agent"
    var size: CGFloat = 36
    var address: String? = nil        // drives per-identity color + initials
    var displayName: String? = nil    // initials source (falls back to address)
    var showAgentHalo: Bool = false   // soft ring for agents (opt-in; used in SP2)

    private var isAgent: Bool { participantType == "agent" }

    var body: some View {
        Group {
            if let address {
                identityAvatar(address: address)
            } else {
                legacyAvatar
            }
        }
        .accessibilityLabel(accessibilityText)
    }

    // MARK: Identity-colored avatar

    @ViewBuilder
    private func identityAvatar(address: String) -> some View {
        let fill = ParticipantColor.color(for: address)
        let initials = ParticipantColor.initials(for: displayName ?? address)
        Circle()
            .fill(fill)
            .frame(width: size, height: size)
            .overlay {
                Text(initials)
                    .font(.system(size: size * 0.42, weight: .semibold, design: .rounded))
                    .foregroundStyle(.white)
            }
            .overlay {
                if showAgentHalo && isAgent {
                    Circle().stroke(fill.opacity(0.35), lineWidth: max(2, size * 0.06))
                        .scaleEffect(1.18)
                }
            }
    }

    // MARK: Legacy type-tinted avatar (unchanged behavior)

    private var legacyAvatar: some View {
        Circle()
            .fill(isAgent ? Color.orange.opacity(0.15) : Color.blue.opacity(0.15))
            .frame(width: size, height: size)
            .overlay {
                Image(systemName: isAgent ? "cpu.fill" : "person.fill")
                    .font(.system(size: size * 0.45))
                    .foregroundStyle(isAgent ? .orange : .blue)
            }
    }

    private var accessibilityText: String {
        if let displayName { return "\(displayName) avatar" }
        return isAgent ? "Agent avatar" : "Human avatar"
    }
}
```

- [ ] **Step 2: Build to confirm all 8 call sites still compile**

Run (build only, fast feedback):
```bash
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  build-for-testing
```
Expected: BUILD SUCCEEDED. (The 7 call sites that don't pass `address` are source-compatible because the new params are defaulted; the 8th uses `participantType:` only as well.)

- [ ] **Step 3: Thread identity into the message-thread header**

In `ios/AINMMessenger/AINMMessenger/UI/MessageThread/AgentMessageView.swift`, the header currently calls `AvatarView(participantType: senderType, size: 24)`. `AgentMessageView` already receives `senderName` and `senderType`; add a `senderAddress` input and pass it through.

Change the struct's stored properties (top of the file) from:
```swift
    let blocks: [ContentBlock]
    let senderName: String
    let senderType: String
```
to:
```swift
    let blocks: [ContentBlock]
    let senderName: String
    let senderType: String
    var senderAddress: String? = nil
```

Change the avatar call in the header from:
```swift
                    AvatarView(participantType: senderType, size: 24)
```
to:
```swift
                    AvatarView(
                        participantType: senderType,
                        size: 24,
                        address: senderAddress,
                        displayName: senderName
                    )
```

- [ ] **Step 4: Pass `senderAddress` from `MessageGroupView`**

In `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageGroupView.swift`, the `else` branch constructs `AgentMessageView(...)`. `group` is a `MessageGroup`, which has `senderAddress: String`. Add the argument. Change:
```swift
                        AgentMessageView(
                            blocks: blocks,
                            senderName: group.senderName,
                            senderType: group.senderType,
                            showHeader: isFirst,
```
to:
```swift
                        AgentMessageView(
                            blocks: blocks,
                            senderName: group.senderName,
                            senderType: group.senderType,
                            senderAddress: group.senderAddress,
                            showHeader: isFirst,
```

- [ ] **Step 5: Build + run the existing thread/discovery UITests to confirm no regression**

Run the full unit suite plus the agent-discovery UITests:
```bash
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessengerUITests \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerUITests/AgentDiscoveryUITests
```
Expected: all green. (Agent avatars in the thread header now render identity-colored initials instead of the generic cpu symbol; the layout is unchanged, so UITests that assert identifiers/labels still pass. If a UITest asserted the literal "Agent avatar" accessibility label for a header avatar, update that assertion to the participant's `"<name> avatar"`.)

- [ ] **Step 6: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/Shared/AvatarView.swift \
  ios/AINMMessenger/AINMMessenger/UI/MessageThread/AgentMessageView.swift \
  ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageGroupView.swift
rtk proxy git commit -m "$(printf 'feat(sp1): identity-aware AvatarView + thread-header wiring\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>')"
```

---

## Task 6: DesignSystem catalog view (DEBUG review surface)

A DEBUG-only SwiftUI view + previews that render the palette, type scale, and sample avatars in light and dark — the human-review artifact. Not shipped in release.

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/UI/DesignSystem/DesignSystemCatalogView.swift`

- [ ] **Step 1: Implement the catalog view (DEBUG-gated)**

Create `ios/AINMMessenger/AINMMessenger/UI/DesignSystem/DesignSystemCatalogView.swift`:

```swift
#if DEBUG
import SwiftUI

/// DEBUG-only visual catalog of the Editorial Warm design system.
/// Reachable via #Preview; not compiled into release builds.
struct DesignSystemCatalogView: View {
    private let swatches: [(String, Color)] = [
        ("Paper", Theme.Color.paper),
        ("Surface", Theme.Color.surface),
        ("SurfaceBorder", Theme.Color.surfaceBorder),
        ("InkPrimary", Theme.Color.inkPrimary),
        ("InkSecondary", Theme.Color.inkSecondary),
        ("HumanBubble", Theme.Color.humanBubble),
        ("HumanBubbleText", Theme.Color.humanBubbleText),
        ("Accent", Theme.Color.accent),
        ("AccentSoft", Theme.Color.accentSoft),
        ("Separator", Theme.Color.separator),
    ]
    private let sampleAddresses = [
        "assistant@localhost", "researcher@acme.test", "sam@home.test",
        "alice@example.com", "bob@example.com", "olivia@studio.test",
    ]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Theme.Spacing.xl) {
                section("Colors") {
                    ForEach(swatches, id: \.0) { name, color in
                        HStack(spacing: Theme.Spacing.md) {
                            RoundedRectangle(cornerRadius: Theme.Radius.chip)
                                .fill(color)
                                .frame(width: 44, height: 44)
                                .overlay(RoundedRectangle(cornerRadius: Theme.Radius.chip)
                                    .stroke(Theme.Color.separator))
                            Text(name).font(Theme.Typography.body())
                        }
                    }
                }
                section("Identity palette") {
                    HStack(spacing: Theme.Spacing.sm) {
                        ForEach(sampleAddresses, id: \.self) { addr in
                            AvatarView(participantType: "agent", size: 40,
                                       address: addr, displayName: addr, showAgentHalo: true)
                        }
                    }
                }
                section("Typography") {
                    Text("Display Large").font(Theme.Typography.displayLarge())
                    Text("Display Title").font(Theme.Typography.displayTitle())
                    Text("Sender Name").font(Theme.Typography.senderName())
                    Text("Agent prose — considered, serif.").font(Theme.Typography.agentProse())
                    Text("Body text in SF.").font(Theme.Typography.body())
                    Text("LABEL").font(Theme.Typography.label())
                    Text("Caption").font(Theme.Typography.caption())
                }
            }
            .padding(Theme.Spacing.lg)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(Theme.Color.paper)
        .foregroundStyle(Theme.Color.inkPrimary)
    }

    @ViewBuilder
    private func section(_ title: String, @ViewBuilder _ content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: Theme.Spacing.md) {
            Text(title).font(Theme.Typography.label()).foregroundStyle(Theme.Color.inkSecondary)
            content()
        }
    }
}

#Preview("Catalog — Light") {
    DesignSystemCatalogView().preferredColorScheme(.light)
}
#Preview("Catalog — Dark") {
    DesignSystemCatalogView().preferredColorScheme(.dark)
}
#endif
```

- [ ] **Step 2: Build to confirm it compiles (DEBUG config)**

Run:
```bash
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  build-for-testing
```
Expected: BUILD SUCCEEDED.

- [ ] **Step 3: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/DesignSystem/DesignSystemCatalogView.swift
rtk proxy git commit -m "$(printf 'feat(sp1): DEBUG DesignSystem catalog view + previews\n\nCo-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>')"
```

---

## Task 7: Full-suite verification + Definition of Done

Confirm the whole thing is green and clean — no regressions, all DoD items met.

**Files:** none (verification only).

- [ ] **Step 1: Run the full iOS unit suite**

Run:
```bash
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test
```
Expected: TEST SUCCEEDED — including the new `DesignSystemTests` (AssetColorTests, ThemeTokenTests, ParticipantColorTests, HapticsTests).

- [ ] **Step 2: Run the UITest suite**

Run:
```bash
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessengerUITests \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test
```
Expected: all green. (If any avatar-related label assertion fails, reconcile per Task 5 Step 5.)

- [ ] **Step 3: Confirm DoD checklist**

Verify each item from the spec §13:
1. `Assets.xcassets` has all 10 semantic + `Identity0…9` color sets, light + dark — AssetColorTests green.
2. `Theme` exposes Color/Typography (serif + SF)/Spacing/Radius/Motion; no raw point sizes.
3. `Haptics.send/receive/select/success` present; HapticsTests green.
4. `ParticipantColor` pure + deterministic; ParticipantColorTests green (goldens unchanged).
5. `AvatarView` has optional `address`/`displayName`/`showAgentHalo`; renders identity color + initials with an address, legacy behavior without; all call sites compile; thread header wired.
6. `DesignSystemCatalogView` renders palette/type/avatars (DEBUG) in light + dark.
7. Full unit suite + UITests green; clean build.

- [ ] **Step 4: Confirm no stray files / clean working tree for the module**

Run:
```bash
rtk proxy git status --short
```
Expected: no uncommitted DesignSystem files; the generator `_gen_colors.sh` is gone (deleted in Task 1).

(No commit needed — this task is verification. The whole-sub-project push to origin is a separate, user-gated step after this plan completes.)

---

## Self-review notes (for the executor)

- **New files auto-include** via synchronized folder groups; if a "cannot find X in scope" error appears for a freshly-created file, it's almost always a stale derived-data cache — clean `/tmp/ainm-dd` and rebuild, or open the project in Xcode once.
- **Golden hash values are authoritative.** If `testPaletteIndexGoldenValues` fails, fix the implementation to match FNV-1a — never edit the expected indices.
- **Scope discipline:** do NOT restyle bubbles, composer, lists, typing indicator, or content blocks here. Identity-coloring the thread-header avatar (Task 5) is the only visible change; everything else is foundation. Bubble/surface restyle is SP2.
