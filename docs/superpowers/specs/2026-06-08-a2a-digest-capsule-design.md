# A2A Digest Capsule (SP4) — Design (v1)

**Date:** 2026-06-08
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)

## Context

SP1–SP3 (the Editorial Warm UI/UX refresh) and the participant-roster feature shipped the inline
expression of agent-to-agent (A2A) conversation: per-identity colors, the `ParticipantByline`
(name-tint + directional `→` tag + foreign-host provenance), `MessageGroup.directedTo`,
`isMultiParticipant`, and a live participant roster. SP4 is the **digest capsule** — the deferred
fast-follow from the original SP1 decision ("inline A2A first, digest capsule as fast-follow"),
explicitly punted from SP3.

**Problem:** in a multi-agent thread, a burst of agent↔agent deliberation (Assistant and Researcher
working something out across several messages, none addressed to the human) drowns the human who is
scanning the thread. SP4 collapses such a burst into a single expandable **capsule** so the human sees
"the agents conferred" at a glance, and can expand to read every message.

**Constraint:** display-only — no server, protocol, runtime, or network change (consistent with
SP1–SP3 and the roster feature). True semantic summarization needs an LLM/agent-provided summary
(a protocol change), so it is **out of scope**; the capsule's "digest" is a structural line derived
from message metadata the client already has.

## Decisions (locked during brainstorming)

1. **Trigger:** collapse a run of **≥3** consecutive A2A messages, in multi-participant threads only,
   involving **≥2 distinct agents**.
2. **Digest:** participants + count (e.g. *Assistant ↔ Researcher · 5 messages*), identity-tinted.
   No semantic summary, no message excerpt.
3. **Tail behavior:** collapse only **settled, bounded** bursts; the trailing/active run renders
   inline until the conversation moves past it.
4. **Visual treatment:** **bordered card** (treatment B) — a surface card with overlapped agent
   avatars, tapped to expand in place.

## 1. Goal & scope

Make multi-agent deliberation scannable: fold a finished A2A burst into one identity-tinted card that
expands in place to the full messages. Editorial Warm, light + dark. Display-only.

**In scope:**
- A pure `A2ACapsuleFolder` that turns `[MessageGroup]` into a `[ThreadItem]` render list.
- `ThreadItem` (group | capsule) and `A2ACapsule` models.
- `A2ACapsuleView` (collapsed card ↔ expanded-in-place) reusing `MessageGroupView` for the expansion.
- View-model `threadItems` + per-capsule expansion state.
- `MessageThreadView` rendering `threadItems`.
- Pure unit tests + render/smoke; suites green.

**Out of scope:** any server/protocol/network change; real/semantic summarization or agent-provided
summaries; message excerpts in the digest; capsuling the live/streaming tail; persisting expansion
state; capsules in 1:1 threads; collapsing single-agent monologues; changes to sub-thread, tool-call,
typing, or streaming rendering (all render outside `messageGroups` and are untouched).

## 2. Trigger logic (the folding rules)

A **capsule** wraps a maximal run of consecutive `MessageGroup`s that are all **A2A**, subject to a
size/threshold gate, appearing only in multi-participant threads.

**A2A group:** a `MessageGroup` where `senderType == "agent"`, `!isFromCurrentUser`, and the group is
**not directed at the current user** (`directedTo != currentUserIdentity`). (A group directed at the
user is the agent reporting back to the human — that must stay inline.)

**Boundary:** a run ends at any group that is not an A2A group — i.e. a human message, or an agent
message directed at the current user.

**Capsule gate:** a closed run becomes a capsule only when **all** hold:
- `isMultiParticipant` is true (the thread has >1 non-current-user participant);
- the run spans **≥3** messages total (sum of `messages.count` across the folded groups);
- the run involves **≥2 distinct agent senders** (a single agent's consecutive A2A groups do not
  capsule — that is not "agent-to-agent").

**Settled/bounded only:** a run capsules only when it is **closed** — followed by at least one
non-A2A `ThreadItem` later in the thread. The trailing run at the very end of `messageGroups` stays
inline (rendered as normal `.group` items). Actively-streaming messages are not part of
`messageGroups` (they render separately) and are therefore never folded; once they settle and the
conversation moves past them, the now-bounded run folds on the next `rebuildGroups()`.

Net effect: 1:1 assistant threads, short handoffs (≤2), single-agent runs, and the active tail are
**never** capsuled; only finished multi-agent deliberation collapses.

## 3. Architecture (render-list model)

`rebuildGroups()` continues to produce `[MessageGroup]` unchanged. A new **pure folding pass** turns
that into a `[ThreadItem]` render list. No DB/schema change.

```
UI/MessageThread/
  ThreadItem.swift            (NEW) enum ThreadItem: Identifiable {
                                    case group(MessageGroup)
                                    case capsule(A2ACapsule)
                                    var id: String { ... }     // group.id / "capsule-"+capsule.id
                                  }
  A2ACapsule.swift            (NEW) struct A2ACapsule: Identifiable {
                                    let id: String              // == first folded group's id (stable)
                                    let groups: [MessageGroup]
                                    let agents: [String]        // distinct agent addresses, first-seen order
                                    let messageCount: Int
                                  }
  A2ACapsuleFolder.swift      (NEW, PURE) static func fold(
                                    groups: [MessageGroup],
                                    currentUserIdentity: String,
                                    isMultiParticipant: Bool
                                  ) -> [ThreadItem]
                                  // detects closed A2A runs, applies the gate, leaves the trailing
                                  // run + everything else as .group. Host-free, unit-testable.
  MessageThreadViewModel.swift (modify) var threadItems: [ThreadItem] = []; rebuild it in
                                    rebuildGroups() via the folder; keep messageGroups.
                                    var expandedCapsuleIDs: Set<String> = []  (+ toggle method)
  A2ACapsuleView.swift        (NEW) treatment B; collapsed card ↔ expanded-in-place; reuses
                                    MessageGroupView for the expanded run.
  MessageThreadView.swift     (modify) ForEach over viewModel.threadItems, switching:
                                    .group → MessageGroupView (unchanged call)
                                    .capsule → A2ACapsuleView(capsule:, isExpanded: binding, ...)
```

The folder is the brain (pure); the view is dumb. `messageGroups` remains for any existing consumer
(e.g. `handleScrollToMessage` searches it); `threadItems` is the new render source. "Trailing run is
inline" is implemented by the folder: it only emits a `.capsule` for a run that is followed by a
non-A2A group; a run that reaches the end of the array is emitted as individual `.group`s.

## 4. The capsule view (treatment B) & expand/collapse

`A2ACapsuleView(capsule: A2ACapsule, isExpanded: Bool, homeHost: String?, isMultiParticipant: Bool,
onToggle: () -> Void, contentBlocksProvider:, timestampFormatter:, ...)` — the message-rendering
closures mirror what `MessageGroupView` already receives, so the expanded run reuses it verbatim.

- **Collapsed (default):** a `Theme.Color.surface` card, `Theme.Radius.card12`, `Theme.Color.surfaceBorder`
  hairline, `Theme.Spacing` padding. Left: **overlapped avatars** of `capsule.agents` (cap 3 shown,
  `+N` when more), each `AvatarView` in identity color, size ~24, overlapped by a negative offset.
  Center: identity-tinted serif names via `ParticipantColor.color(for:)` —
  `A ↔ B` for two, `A, B +N` for 3+ — then `· {messageCount} messages` in
  `Theme.Typography.caption()`/`Theme.Color.inkSecondary`. Right: a chevron (`chevron.down`).
  Whole card is one tap target; `Haptics.select()` on tap → `onToggle()`.
- **Expanded:** the card becomes a slim header row (same digest, chevron up); **below it the folded
  run renders as normal `MessageGroupView`s** (full bylines, avatars, content blocks — reused), inset
  slightly (leading padding) so it reads as "inside" the capsule, ending with a quiet `⌃ Collapse`
  control that also calls `onToggle()`.
- **Accessibility:** collapsed = one combined element, label e.g. *"Agent deliberation, Assistant and
  Researcher, 5 messages, collapsed"* + hint *"Double-tap to expand"*. Expanded: the header is a
  control; the children are the normal message elements.
- **Motion:** expand/collapse via `Theme.Motion`; under reduce-motion no height animation (same
  `UIView.setAnimationsEnabled(false)` proxy used in SP2/SP3 tests; runtime honors the env value).

## 5. Expansion state

Expansion is **view state, not persisted.** `MessageThreadViewModel` holds
`var expandedCapsuleIDs: Set<String> = []` and `func toggleCapsule(_ id: String)`. Default collapsed
(id absent). The capsule id is its **first folded group's id** = the first message id of the run,
which is **stable across `rebuildGroups()`** (messages aren't deleted; the run's first message is
fixed). So expanding a capsule survives inbound-message rebuilds, and a capsule that grows (more A2A
before the boundary) keeps its id and expansion. `MessageThreadView` passes
`isExpanded: viewModel.expandedCapsuleIDs.contains(capsule.id)` and `onToggle: { viewModel.toggleCapsule(capsule.id) }`.

## 6. Data flow & edge cases

**Flow:** `allMessages` → `rebuildGroups()` → `[MessageGroup]` → `A2ACapsuleFolder.fold(...)` →
`threadItems: [ThreadItem]` → `MessageThreadView` ForEach → `MessageGroupView` / `A2ACapsuleView` →
(expanded) `MessageGroupView`. Display-only; no view-model behavior, network, runtime, or server change.

**Edge cases:**
- Run of exactly 2, or a single distinct agent → not folded (stays `.group`).
- `isMultiParticipant == false` → folder returns all `.group` (no capsule ever).
- Trailing/unbounded run (thread tail) → emitted as `.group`s, folds once a later non-A2A item exists.
- Streaming/typing/tool-call/sub-thread views are untouched (rendered outside `messageGroups`).
- `directedTo` reflects the group's first message (SP3 limitation, unchanged); a group directed at the
  user correctly bounds the run.
- Capsule id collision is impossible (message ids are unique; first-message-id is unique per run).
- Dark mode: every element uses semantic/identity tokens with dark appearances (SP1).

## 7. Testing

- **Pure unit (`A2ACapsuleFolder`):**
  - ≥3 A2A run with 2 distinct agents, bounded by a to-human/human group → one `.capsule` with correct
    `agents`, `messageCount`, and id == first group id.
  - 2-message run → no capsule. Single-agent run (≥3 msgs, one agent) → no capsule.
  - `isMultiParticipant == false` → all `.group`, no capsule.
  - Human message and to-human agent message each end a run.
  - Trailing unbounded A2A run at array end → `.group`s (inline), not a capsule.
  - Two separate bursts in one thread → two capsules.
- **Render/smoke (dependency-free `UIHostingController` harness, light + dark):** collapsed card
  (2 agents), collapsed card (3+ agents, `+N`), expanded state (reuses MessageGroupView), reduce-motion
  parity.
- **Regression:** full iOS unit suite + UITests green; clean build. `handleScrollToMessage` and any
  `messageGroups` consumer still work (messageGroups retained).

## 8. Definition of Done

1. `A2ACapsuleFolder.fold` implements the trigger rules (≥3, ≥2 agents, multi-participant, bounded);
   pure and unit-tested across the matrix in §7.
2. `ThreadItem` + `A2ACapsule` models; `MessageThreadViewModel.threadItems` rebuilt via the folder;
   `messageGroups` retained.
3. `A2ACapsuleView` treatment B: collapsed card (overlapped avatars + tinted names + count) ↔
   expanded-in-place run (reusing `MessageGroupView`) + collapse affordance; `Haptics.select()`.
4. Per-capsule expansion via `expandedCapsuleIDs` (stable id = first group id), default collapsed,
   preserved across rebuilds.
5. 1:1 threads, ≤2 runs, single-agent runs, and the live tail are never capsuled.
6. `MessageThreadView` renders `threadItems`; render/smoke covers collapsed/expanded in light + dark +
   reduce-motion.
7. Full iOS unit suite + UITests green; clean build; committed to `main` with `rtk proxy git`.

## 9. Build order

1. `ThreadItem` + `A2ACapsule` models.
2. `A2ACapsuleFolder.fold` (pure) + unit tests — the trigger brain.
3. `MessageThreadViewModel`: `threadItems` (rebuilt via folder) + `expandedCapsuleIDs`/`toggleCapsule`.
4. `A2ACapsuleView` (collapsed + expanded) + render/smoke.
5. Wire `MessageThreadView` to render `threadItems`.
6. Full unit + UITest suites green; DoD audit; commit.

The models land first, then the pure folder (everything depends on it), then the view-model wiring,
then the view, then the render switch, then verification.
