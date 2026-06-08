# Design System Foundation (SP1) — Design (v1)

**Date:** 2026-06-08
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)

## Context

The AINM iOS reference client (`ios/AINMMessenger`) is well-structured but styled with
stock SwiftUI defaults — `.accentColor`, `Color(.systemBackground)`, system fonts, and a
two-state `AvatarView` (blue "human" / orange "agent"). There is **no design system**: no
`Assets.xcassets`, no color palette, no typography scale, no spacing/motion tokens, and no
way to visually distinguish one agent from another. Colors are defined inline, ad hoc.

This is the first of three sub-projects in a UI/UX refresh whose agreed direction is:

- **Ambition:** signature agent-first redesign on a real design system.
- **Aesthetic:** **Editorial Warm** — warm paper tones, a serif display voice, ink-dark human
  bubbles — delivered as a full **light + dark** system.
- **Agent identity:** living bubble + haloed avatar, ink streaming cursor, quiet status line.
- **A2A (agent-to-agent):** every participant gets a unique avatar + color; agents address each
  other via `→ @name` directional tags; inline A2A first, digest capsule as fast-follow.

**Sub-project roadmap (each its own spec → plan → execution):**
- **SP1 — Design System Foundation (this spec):** tokens, palette, typography, spacing, motion,
  haptics, the per-identity color engine, and the `AvatarView` refactor. The prerequisite layer.
- **SP2 — Conversation Surface Polish:** apply the system to bubbles, the living agent message,
  streaming cursor, composer, lists, typing indicator, tool-call chip.
- **SP3 — A2A & Multi-Participant Expression:** directional tags, multi-agent color across threads,
  federation provenance, then the digest capsule.

**This spec is SP1 only.** It deliberately does **not** restyle bubbles, the composer, lists, or
any conversation surface — that is SP2. SP1 builds the foundation and proves it through the
per-identity color engine and the `AvatarView` refactor (the one shared component every surface
already uses).

## 1. Goal & scope

Introduce a centralized, testable Editorial-Warm design system — semantic colors (light + dark),
a New-York-serif typography scale, spacing/radius/motion tokens, haptics, and a deterministic
per-identity color engine — and refactor `AvatarView` to consume it so multiple participants
become visually distinct immediately.

**In scope:**
1. A `UI/DesignSystem/` module (new folder) with: `Assets.xcassets` (semantic color sets,
   light + dark), `Theme.swift` (color/typography/spacing/radius/motion accessors),
   `Haptics.swift`, `ParticipantColor.swift` (per-identity engine), and
   `DesignSystemCatalogView.swift` (`#if DEBUG` review surface + previews).
2. `AvatarView` refactored to render an identity-colored avatar with initials (symbol fallback),
   accepting an **optional** `address`/`displayName` while preserving its current `participantType`
   API so all 8 existing call sites keep compiling unchanged.
3. Unit tests for the pure logic (`ParticipantColor`, initials, token resolution) and a test that
   every semantic asset color resolves at runtime.

**Out of scope (later sub-projects / explicitly not now):**
- Restyling message bubbles, composer, conversation list/rows, typing indicator, tool-call chip,
  content blocks, memory/briefing surfaces (**SP2**).
- Directional `→ @name` tags, multi-agent thread treatment, federation provenance, digest capsule
  (**SP3**).
- Bundling a custom font (decision: use the system serif **New York** via `design: .serif`).
- Migrating every inline color in the app to tokens. SP1 establishes the tokens and migrates only
  `AvatarView`; broad migration happens naturally as SP2 restyles each surface.

## 2. Architecture & components

All new code lives in a new folder `ios/AINMMessenger/AINMMessenger/UI/DesignSystem/`. Because the
Xcode project uses **synchronized folder groups** (`PBXFileSystemSynchronizedRootGroup`), files and
the asset catalog added under the app folder are auto-included in the target — **no `.pbxproj`
editing is required**.

```
UI/DesignSystem/
  Assets.xcassets/              (new) semantic Color Sets, each with Any + Dark appearance
    Paper.colorset/             screen background
    Surface.colorset/           cards / agent message background
    SurfaceBorder.colorset/     hairline borders
    InkPrimary.colorset/        primary text
    InkSecondary.colorset/      secondary text
    HumanBubble.colorset/       human message background
    HumanBubbleText.colorset/   human message text
    Accent.colorset/            warm burnt-orange accent
    AccentSoft.colorset/        chip / soft-fill background
    Separator.colorset/         dividers
    Palette/                    per-identity palette entries: Identity0…Identity9 colorsets
  Theme.swift                   Theme.Color / .Typography / .Spacing / .Radius / .Motion
  Haptics.swift                 Haptics.send()/.receive()/.select()/.success()
  ParticipantColor.swift        deterministic address → palette slot; initials(for:)
  DesignSystemCatalogView.swift #if DEBUG catalog + SwiftUI previews
AvatarView.swift                (modify) consume ParticipantColor + Theme
```

Each unit has one responsibility and a clear interface: `Theme` is a pure token namespace,
`ParticipantColor` is pure deterministic logic, `Haptics` is a thin effect wrapper, `AvatarView`
is the one view that changes.

## 3. Color system (`Assets.xcassets` + `Theme.Color`)

The asset catalog is the single source of truth for color; `Theme.Color` exposes typed accessors
so call sites never pass stringly-typed names. Each color set defines a **Universal (light)** and a
**Dark** appearance. Values below are the starting palette (hex; may be nudged during the catalog
review step, but the token *names* are fixed):

| Token | Light | Dark |
|-------|-------|------|
| `Paper` | `#FAF6F0` | `#1A1714` |
| `Surface` | `#FFFFFF` | `#24201B` |
| `SurfaceBorder` | `#ECE2D4` | `#3A332B` |
| `InkPrimary` | `#2A241D` | `#F2ECE2` |
| `InkSecondary` | `#6B5D49` | `#A99A86` |
| `HumanBubble` | `#1F1B16` | `#3A332B` |
| `HumanBubbleText` | `#FAF6F0` | `#F2ECE2` |
| `Accent` | `#C2410C` | `#E8743A` |
| `AccentSoft` | `#FDF0E4` | `#3A2A1E` |
| `Separator` | `#E6DCCB` | `#3A332B` |

`Theme.Color` provides: `static var paper/surface/surfaceBorder/inkPrimary/inkSecondary/`
`humanBubble/humanBubbleText/accent/accentSoft/separator: Color` — each returning
`Color("<Name>", bundle: .main)`.

**Per-identity palette:** ten color sets `Identity0`…`Identity9` (each light + dark), a curated set
that harmonizes with Editorial Warm — e.g. burnt orange, teal, mustard, clay, plum, olive,
terracotta, slate-blue, rose, moss. Exposed as `Theme.Color.identityPalette: [Color]` (index order
fixed) so `ParticipantColor` can map into it.

## 4. Typography (`Theme.Typography`)

Two voices: **serif (New York)** for the editorial display voice (headers, agent prose) via
`Font.system(_, design: .serif)`, and **SF (default)** for body/secondary text. All built on system
text styles so **Dynamic Type works for free**. New York requires no bundling or licensing.

```swift
enum Typography {
    // Serif display voice
    static func displayLarge() -> Font  { .system(.largeTitle, design: .serif).weight(.semibold) }
    static func displayTitle() -> Font  { .system(.title2,     design: .serif).weight(.semibold) }
    static func senderName()  -> Font   { .system(.subheadline, design: .serif).weight(.semibold) }
    static func agentProse()  -> Font   { .system(.body,        design: .serif) }
    // SF body voice
    static func body()        -> Font   { .system(.body) }
    static func callout()     -> Font   { .system(.callout) }
    static func label()       -> Font   { .system(.caption,  design: .default).weight(.semibold) }
    static func caption()     -> Font   { .system(.caption2) }
}
```

(Exact role set may grow slightly during implementation; these are the committed roles. No raw
point sizes — always a text style so accessibility scaling is preserved.)

## 5. Spacing, radius, motion (`Theme`)

```swift
enum Spacing { static let xs:CGFloat=4, sm=8, md=12, lg=16, xl=20, xxl=24 }
enum Radius  { static let chip:CGFloat=8, card=12, bubble=16, avatar=.infinity }
enum Motion  {
    static let messageAppear = Animation.spring(response: 0.34, dampingFraction: 0.82)
    static let subtle        = Animation.easeInOut(duration: 0.2)
}
```

Pure constants; no behavior. They exist so SP2 surfaces share one rhythm rather than re-deriving
magic numbers.

## 6. Haptics (`Haptics.swift`)

A thin, side-effecting wrapper around `UIImpactFeedbackGenerator` /
`UINotificationFeedbackGenerator`, safe to call anywhere:

```swift
enum Haptics {
    static func send()    // .light impact   — message sent
    static func receive() // .soft impact    — message/stream arrived
    static func select()  // .selection      — picker / tab / option
    static func success() // .success notify — completed action
}
```

Each generator is created, `prepare()`d, and fired per call (simple and correct for this volume).
No-ops gracefully where haptics are unavailable (e.g. Simulator) — the system APIs already do this,
so no explicit guard logic is required. SP1 only *defines* these; wiring them into send/receive is
SP2. A unit test asserts the calls don't crash.

## 7. Per-identity color engine (`ParticipantColor.swift`)

The foundation for A2A: a **pure, deterministic** mapping from an AINM address to a stable palette
slot, so the same participant is always the same color across launches and devices, and distinct
participants are visually separable.

```swift
enum ParticipantColor {
    /// Stable palette index for an address (0..<count). Deterministic across runs/devices.
    static func paletteIndex(for address: String, count: Int) -> Int
    /// Convenience: the SwiftUI Color from Theme.Color.identityPalette for this address.
    static func color(for address: String) -> Color
    /// Up to two uppercase initials derived from a display name (falls back to address local-part).
    static func initials(for displayName: String) -> String
}
```

- `paletteIndex` uses a small **stable, explicit hash** (FNV-1a over the address's UTF-8 bytes)
  modulo `count`. It must NOT use Swift's `Hasher`/`hashValue` (per-process seed → not stable).
- `initials` is pure string logic: take the first letter of up to the first two whitespace-separated
  words, uppercased; if the name is empty or has no letters, derive from the address local-part
  (before `@`); never returns empty (final fallback `"?"`).

**Tests (pure, no simulator UI):**
- Determinism: `paletteIndex(for: a, count:)` equals itself across many calls and equals a
  hard-coded expected index for several fixed sample addresses (locks the hash).
- Stability vs `Hasher`: two different fixed addresses that are known to differ still differ here;
  the function never depends on process state.
- Distribution: hashing a set of ~50 varied addresses touches at least, say, 6 of 10 slots (guards
  against a degenerate hash) — asserted as a sanity bound, not an exact distribution.
- `initials`: `"Sam Rivera" → "SR"`, `"assistant" → "A"`, `"researcher@acme.test" → "R"`,
  `"" → "?"`, multi-space and emoji-only names handled without crashing.

## 8. `AvatarView` refactor

`AvatarView` is the only existing view SP1 modifies. It currently takes
`participantType: String` (+ `size`) and renders a tinted circle with `person.fill`/`cpu.fill`.
The refactor **adds optional inputs** and identity coloring while keeping the existing initializer
call shape valid:

```swift
struct AvatarView: View {
    let participantType: String          // "human" | "agent" (unchanged)
    var size: CGFloat = 36               // unchanged default
    var address: String? = nil           // NEW — drives per-identity color + initials
    var displayName: String? = nil       // NEW — initials source
    var showAgentHalo: Bool = false      // NEW — ring for agents (used by SP2; default off)
    // body: identity-colored fill when address != nil, else type-default fill;
    //       initials when displayName/address available, else person.fill/cpu.fill symbol.
}
```

- When `address != nil`: fill = `ParticipantColor.color(for: address)`; show
  `ParticipantColor.initials(for: displayName ?? address)`.
- When `address == nil`: preserve today's behavior — type-tinted fill + SF Symbol — so the 8 call
  sites that don't yet pass an address (e.g. `ConversationRowView`, `StreamingMessageView`) are
  visually unchanged and still compile.
- `showAgentHalo` draws a soft ring (using `Theme.Color.accentSoft`/identity color) for agents;
  default `false` so nothing changes until SP2 opts in.
- Accessibility label preserved/extended (`"<displayName> avatar"` when available).

**Call sites:** all 8 keep working as-is. Where an address is readily available
(`AgentMessageView` has `senderType`; its parent `MessageGroup` has `senderAddress`), SP1 may pass
`address`/`displayName` through to light up per-identity color in the thread header — but only as a
non-behavioral wiring change, with no layout restyle (that's SP2). If threading the address adds
risk, it is acceptable for SP1 to leave call sites untouched and let SP2 pass identities; the
engine + optional params are the deliverable.

## 9. Catalog / review surface (`DesignSystemCatalogView.swift`)

A `#if DEBUG` SwiftUI view (and `#Preview`s) that renders, in light and dark: every semantic color
swatch with its token name, the typography scale with sample text, spacing/radius samples, and a
row of avatars for sample addresses (`assistant@…`, `researcher@…`, `sam@…`, etc.) to visually
confirm per-identity distinctness. This is the human-review artifact for the palette and is not
shipped in release builds. It is reachable from a DEBUG-only entry (e.g. a hidden settings row or a
preview only) — no production navigation is added.

## 10. Data flow

`Theme` and `ParticipantColor` are pure/static — no state, no async. SwiftUI views call
`Theme.Color.*` / `Theme.Typography.*` directly. `AvatarView` calls `ParticipantColor` when given
an address. `Haptics` is fire-and-forget. There is no persistence, no network, no new app state,
and no change to any view model or repository.

## 11. Error handling

- Missing/typo'd asset color name → `Color("Name")` renders clear/black silently. Mitigation: the
  asset-resolution test (below) asserts every token in a fixed list resolves via
  `UIColor(named:)` non-nil, so a missing/renamed set fails CI rather than shipping invisibly.
- `ParticipantColor` never throws and never returns empty initials (final `"?"` fallback).
- `Haptics` calls are best-effort; unavailability is a silent no-op.

## 12. Testing

New test file `AINMMessengerTests/DesignSystem/DesignSystemTests.swift` (pure unit tests, no UI
host required):

1. **`ParticipantColor.paletteIndex` determinism** — fixed addresses map to fixed, repeatable
   indices; repeated calls are stable.
2. **`ParticipantColor.initials`** — the cases in §7.
3. **Distribution sanity** — ~50 sample addresses cover ≥6 of 10 slots.
4. **Asset resolution** — for every semantic token name and `Identity0…9`, `UIColor(named:)` is
   non-nil (catches a missing/renamed color set).
5. **`Haptics`** — calling each function does not crash (executes the generator path).

**Regression:** full iOS unit suite green; `AgentDiscoveryUITests` green; build clean (no new
warnings) under
`xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger
-destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test`.
Existing UITests run under `-scheme AINMMessengerUITests`.

## 13. Definition of Done

1. `UI/DesignSystem/Assets.xcassets` exists with all semantic color sets and `Identity0…9`, each
   with light + dark appearances; the asset-resolution test passes.
2. `Theme.swift` exposes Color, Typography (New York serif + SF), Spacing, Radius, and Motion
   tokens; no raw point font sizes (text styles only).
3. `Haptics.swift` exposes `send/receive/select/success`; its test does not crash.
4. `ParticipantColor.swift` is pure and deterministic; all §7 tests pass.
5. `AvatarView` accepts optional `address`/`displayName`/`showAgentHalo`, renders identity color +
   initials when an address is given, preserves prior behavior otherwise, and all 8 call sites
   compile and run.
6. `DesignSystemCatalogView` renders the palette/type/avatars in light + dark (DEBUG only).
7. Full iOS unit suite + `AgentDiscoveryUITests` green; clean build; committed directly to `main`
   with `rtk proxy git`.

## 14. Build order

1. Create `Assets.xcassets` with all semantic + identity color sets (light + dark) → add the
   asset-resolution test → verify it passes.
2. `Theme.swift` (Color → Typography → Spacing/Radius/Motion).
3. `ParticipantColor.swift` test-first (determinism, initials, distribution) → implementation.
4. `Haptics.swift` + crash-free test.
5. `AvatarView` refactor (optional params + identity rendering), keep all call sites compiling;
   optionally thread `address` from `MessageGroup` into the thread-header avatar (non-restyling).
6. `DesignSystemCatalogView` (DEBUG) + previews.
7. Full suites green + clean build + commit.
