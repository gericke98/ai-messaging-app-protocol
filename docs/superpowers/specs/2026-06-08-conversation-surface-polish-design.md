# Conversation Surface Polish (SP2) — Design (v1)

**Date:** 2026-06-08
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)

## Context

SP1 (Design System Foundation, shipped 2026-06-08) introduced the Editorial Warm design system:
`Theme` tokens (semantic colors light+dark, New York serif + SF typography, spacing/radius/motion),
`ParticipantColor` (deterministic per-identity color + initials), a refactored `AvatarView` (optional
`address`/`displayName`/`showAgentHalo`), and `Haptics`. Only the message-thread *header* avatar
visibly changed; the conversation surfaces themselves still use stock SwiftUI styling
(`Color.accentColor`, `Color(.secondarySystemGroupedBackground)`, system fonts).

SP2 applies the design system to the everyday conversation surfaces and adds the "living agent
message" treatment, making the thread feel premium and agent-native. The visual direction is locked
from the SP1 brainstorming and an SP2 mockup review:

- **Editorial Warm**, full light + dark.
- **Agent messages are "airy"** — no wrapping card. Identity avatar + serif sender name + serif
  prose laid open on the paper. Rich/structured blocks (code, table, form, file, image) keep their
  own (tokenized) containers.
- **Human messages** are ink bubbles (right-aligned).
- **Living agent message**: identity-colored avatar that gains a soft **halo while streaming**, an
  ink **caret in the identity color**, and a quiet **`✺ thinking…`** status line.
- **Grouping**: avatar + serif name on the **first** message of a same-sender run only; subsequent
  messages indent under it with tight spacing; larger gap between different senders (matches current
  `MessageGroupView` behavior).

This is the second of three sub-projects. **SP3 (A2A & Multi-Participant)** — directional `→ @name`
tags, multi-agent thread treatment, federation provenance, digest capsule — is explicitly OUT of
scope here. Per-identity colors already exist (SP1); SP2 only *applies* them.

## 1. Goal & scope

Restyle every everyday conversation surface with the Editorial Warm token system, implement the
living agent message (haloed streaming avatar + identity caret + status line), wire haptics on
send/receive, and resolve the three carried follow-ups from SP1's final review.

**In scope (surfaces restyled):**
- `MessageBubbleView` (human ink bubble; fix non-adaptive width)
- `AgentMessageView` (airy settled agent message)
- `MessageGroupView` (grouping spacing via tokens)
- `StreamingMessageView` (living streaming message)
- `TypingIndicatorView` (identity avatar + status line)
- `CursorView` (identity/accent caret)
- `ToolCallIndicatorView` (warm chip)
- `ComposerView` (tokenized + `Haptics.send()`)
- `ConversationListView` / `ConversationRowView` (tokenized + identity avatar)
- `MessageThreadView` (paper background, token spacing)
- Content-block containers that carry their own background (`CodeBlockView`, `TableBlockView`,
  `FormBlockView`, `FileBlockView`, `ImageBlockView`, `ButtonBlockView`, `ActionBlockView`,
  `ApprovalBlockView`) — tokenize their surface/border/radius only (no structural change).

**Carried follow-ups resolved (from SP1 final holistic review):**
1. **Identity0 == Accent collision** — recolor `Identity0` in the asset catalog to a distinct hue so
   no participant shares the brand accent color. Palette stays 10 entries; `ParticipantColor` and its
   golden-index tests are unaffected (indices map to slots, not specific hues).
2. **Avatar color-flip on stream settle** — `StreamingMessageView` and `TypingIndicatorView` currently
   call `AvatarView(participantType: "agent")` (legacy orange path). Thread `address`/`displayName`
   through so the avatar is identity-colored throughout, eliminating the orange→identity snap when a
   streamed message becomes a settled `AgentMessageView`.
3. **`showAgentHalo` `scaleEffect(1.18)` clipping** — replace the scaled ring with a padded/inner
   stroke that survives clipped parents; add a snapshot test for the haloed avatar.

**Haptics:** `Haptics.send()` on composer send; `Haptics.receive()` when an incoming/streamed message
lands. (SP1 defined these; SP2 wires them.)

**Out of scope:** A2A directional `→ @name` tags, multi-agent thread treatment, federation provenance,
digest capsule (**SP3**). Memory/briefing surfaces (`UI/Memory/*`) — not conversation surfaces; not
restyled here. Onboarding/recovery/account flows — not conversation surfaces. No data-model, network,
or view-model behavior changes (visual/token application + avatar wiring + haptic calls only).

## 2. Architecture & components

SP2 is overwhelmingly *modification* of existing views to consume `Theme`/`ParticipantColor`, plus a
small amount of new structure for the living message. No new architectural layers.

```
UI/DesignSystem/Assets.xcassets/Identity0.colorset  (modify) recolor distinct from Accent
UI/MessageThread/MessageBubbleView.swift   (modify) ink bubble tokens; adaptive max width
UI/MessageThread/AgentMessageView.swift    (modify) airy layout; serif name+prose; drop grey card+badge
UI/MessageThread/MessageGroupView.swift    (modify) Theme.Spacing group gaps
UI/MessageThread/StreamingMessageView.swift(modify) identity avatar+halo; identity caret; status line
UI/MessageThread/TypingIndicatorView.swift (modify) identity avatar; status-line treatment
UI/MessageThread/MessageThreadView.swift   (modify) Theme.Color.paper background; token spacing
UI/Shared/CursorView.swift                 (modify) identity/accent caret color + reduce-motion respect
UI/ToolVisibility/ToolCallIndicatorView.swift (modify) AccentSoft chip
UI/Composer/ComposerView.swift             (modify) tokens; Haptics.send() on send
UI/ConversationList/ConversationListView.swift (modify) paper bg; token spacing
UI/ConversationList/ConversationRowView.swift  (modify) identity avatar; serif title; token preview
UI/ContentBlocks/*.swift                   (modify) tokenize surface/border/radius only
AINMMessengerTests/DesignSystemTests/...   (new)    snapshot tests (see §7)
```

Where a streamed message and its settled counterpart must look identical, both read the same
`Theme`/`ParticipantColor` inputs from the same identity (`address`/`displayName`), guaranteeing
visual continuity.

## 3. The living agent message

`AgentMessageView` (settled) and `StreamingMessageView` (in-progress) must render the *same* agent
identically except for the live affordances. Shared visual contract:

- **Leading identity avatar** (`AvatarView` with `address`+`displayName`), 30–36pt.
- **Serif sender name** (`Theme.Typography.senderName`) on the first message of a run.
- **Serif prose** (`Theme.Typography.agentProse`) for text blocks; `InkPrimary` text on `Paper`
  (no bubble/card around the agent message itself).
- **Rich blocks** keep their own tokenized container (code/table/form/etc.).

Streaming-only affordances:
- **Halo** on the avatar while `status == .streaming` (`showAgentHalo: true`), using the
  clipping-safe inner/padded stroke (follow-up #3) in the identity color at low opacity.
- **Identity caret** (`CursorView`) at the end of the active block, colored with the message's
  identity color (or `Accent` fallback if no address), respecting `accessibilityReduceMotion`
  (no blink when reduce-motion is on — show a static caret).
- **Status line** `✺ thinking…` in `Theme.Typography.caption`/`InkSecondary` above or beside the
  prose while streaming; hidden once settled.
- **"Not verified"** badge (existing 5s-timeout behavior) tokenized (`InkSecondary` + warning icon).

## 4. Human message + composer

- **`MessageBubbleView`**: background `Theme.Color.humanBubble`, text `humanBubbleText`, SF body,
  `Theme.Radius.bubble` corners with the existing 4pt bottom-right tail. **Fix**: replace
  `UIScreen.main.bounds.width * 0.75` with an adaptive constraint (`frame(maxWidth:)` derived from the
  layout via `containerRelativeFrame(.horizontal) { w, _ in w * 0.75 }` or a `GeometryReader`-free
  proportional approach) so it adapts to split-view/iPad/rotation.
- **`ComposerView`**: field on `Surface`/`Paper` with a `SurfaceBorder` hairline and `Theme.Radius`,
  `InkPrimary` text, `InkSecondary` placeholder; send button uses `Accent` when enabled,
  `InkSecondary` when disabled; call `Haptics.send()` when a message is sent. Mention popup and
  offline state preserved.

## 5. Conversation list

- **`ConversationListView`**: `Theme.Color.paper` background, token spacing.
- **`ConversationRowView`**: identity-colored `AvatarView` for the conversation's agent
  (`address` = the thread's agent address when available; falls back to the legacy type avatar when
  no address), serif title (`Theme.Typography.senderName`-ish), `InkSecondary` preview + timestamp,
  `Accent` unread dot. No change to row data or tap behavior.

## 6. Data flow

`Theme`/`ParticipantColor` are pure/static (SP1). SP2 views read tokens directly and pass identity
`address`/`displayName` into `AvatarView`/`CursorView`. Streaming identity comes from
`StreamingMessageState.from` (an address) — derive `displayName` from it the same way the settled path
does (`MessageGroup.senderName`). Haptic calls are fire-and-forget side effects at send/receive points.
No view-model, repository, network, or persistence changes.

## 7. Testing

- **Snapshot tests (chosen): add `pointfreeco/swift-snapshot-testing`** as a test-only SPM dependency
  on the `AINMMessengerTests` target. The project already consumes SPM packages (GRDB, KeychainAccess),
  so the integration follows the existing package pattern. Snapshot — in **light and dark**, fixed
  size — the key surfaces: human bubble, airy settled agent message, streaming message (caret + halo +
  status), typing indicator, tool-call chip, composer (enabled/disabled/offline), conversation row.
  Record reference images on first run, commit them, assert thereafter.
  - **Risk + fallback:** adding the SPM dependency requires a `project.pbxproj` package-reference edit
    (the one file SP1 avoided). If editing the hand-authored pbxproj proves too fragile or triggers an
    unavoidable full Xcode normalization, fall back to **render/smoke tests** (instantiate each view's
    body via a hosting controller to catch crashes/compile breaks, no image diffing) and rely on the
    DEBUG catalog + `#Preview`s for visual review. The plan must isolate the dependency-add as its own
    early, revertible task and verify the pbxproj remains the hand-authored form (or commit the minimal
    necessary package-reference additions deliberately).
- **Existing tests stay green:** full iOS unit suite; `AgentDiscoveryUITests` and the other UITests
  (layout/accessibility identifiers preserved; where an avatar's accessibility label changes from
  "Agent avatar" to "<name> avatar" due to identity wiring, update only those specific assertions).
- **Halo snapshot** specifically covers follow-up #3 (no clipping in a clipped parent).
- **Reduce-motion**: a test/snapshot confirming the caret and typing dots are static under
  `accessibilityReduceMotion`.

## 8. Error handling / edge cases

- **No address available** (e.g. a conversation row with no resolved agent address) → `AvatarView`
  legacy path (unchanged), so nothing regresses.
- **Streaming `from` not an email-like address** → `ParticipantColor.initials` already handles
  arbitrary strings (`"?"` fallback); color mapping is total.
- **Dark mode**: every restyled surface uses semantic tokens that already define dark appearances
  (SP1), so dark mode is covered by construction; snapshots assert it.
- **Long agent prose / many blocks** → airy layout flows naturally; rich blocks self-contain.

## 9. Definition of Done

1. All listed conversation surfaces consume `Theme` tokens (no stray `Color.accentColor`,
   `Color(.systemBackground)`, `Color(.secondarySystemGroupedBackground)`, or system-font calls in
   those files); light + dark both correct.
2. Agent messages render airy (no wrapping card); settled and streaming render the same agent
   identically except for streaming affordances (halo, caret, status line).
3. The avatar color-flip is gone — streaming/typing/settled all show the identity-colored avatar for
   the same agent (follow-up #2).
4. `Identity0` is recolored distinct from `Accent`; `ParticipantColor` golden-index tests still pass
   (follow-up #1).
5. The halo uses a clipping-safe stroke (follow-up #3), covered by a snapshot/render test.
6. `Haptics.send()` wired on composer send; `Haptics.receive()` on incoming/streamed message arrival.
7. `MessageBubbleView` width is adaptive (no `UIScreen.main.bounds`).
8. Snapshot tests (or the documented render/smoke fallback) cover the key surfaces in light + dark;
   full iOS unit suite + UITests green; clean build; committed to `main` with `rtk proxy git`.

## 10. Build order

1. Recolor `Identity0` (catalog) + confirm `ParticipantColor` golden tests still pass (follow-up #1) —
   smallest, isolated.
2. Add `swift-snapshot-testing` dependency to the test target (isolated, revertible; verify pbxproj
   handling) — or invoke the render/smoke fallback.
3. `CursorView` identity caret + reduce-motion.
4. `AvatarView` halo clipping fix + halo snapshot (follow-up #3).
5. `StreamingMessageView` living treatment (identity avatar+halo, caret, status line) — resolves
   follow-up #2 for streaming.
6. `TypingIndicatorView` identity avatar + status line — resolves follow-up #2 for typing.
7. `AgentMessageView` airy settled layout (serif name+prose, drop grey card/badge).
8. `MessageBubbleView` ink tokens + adaptive width.
9. `ToolCallIndicatorView` warm chip; content-block container tokenization.
10. `ComposerView` tokens + `Haptics.send()`; `MessageThreadView` paper bg + `Haptics.receive()`.
11. `ConversationListView`/`ConversationRowView` tokens + identity avatar.
12. Snapshot suite for all key surfaces (light+dark); full suites green; commit.

The recolor and the dependency-add land first (they unblock everything and carry the most risk);
streaming/agent/bubble restyles follow; list and composer last; snapshots close it out.
