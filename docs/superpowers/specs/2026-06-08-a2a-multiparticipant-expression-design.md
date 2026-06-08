# A2A & Multi-Participant Expression (SP3) — Design (v1)

**Date:** 2026-06-08
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)

## Context

SP1 (Design System Foundation) and SP2 (Conversation Surface Polish) shipped the Editorial Warm
design system and applied it to every conversation surface, including the living agent message.
Per-identity colors already exist (`ParticipantColor`) and are applied to avatars (settled, streaming,
typing). SP3 is the third and final sub-project of the UI/UX refresh: it makes **multi-participant /
agent-to-agent (A2A) conversations** legible — who is distinct from whom, who is addressing whom, and
who is from another server.

This is the **inline-expression** slice of the original A2A vision. The **digest capsule** (collapsing
a burst of A2A chatter into an expandable, summarizable capsule) is explicitly deferred to a future
**SP4** — it is heavier (grouping logic, collapsed/expanded views, summarization) and benefits from
being designed against real A2A traffic. This honors the SP1 decision: "inline A2A first, digest capsule
as fast-follow."

Two visual treatments were chosen in an SP3 mockup review:
- **Directional tag → treatment A** (inline in the byline): `Assistant → Researcher`.
- **Federation provenance → treatment A** (dim `· host` suffix), shown for foreign hosts only.

## 1. Goal & scope

Make multi-participant threads scannable and A2A intent explicit, using the existing Editorial Warm
tokens and identity colors — full light + dark.

**In scope (three inline features):**
1. **Multi-agent per-identity distinction** — in threads with more than one non-current-user
   participant, the agent **sender name renders in its identity color** (reinforcing the avatar).
   1:1 human↔agent threads keep the calm `InkPrimary` name (no gratuitous color).
2. **Directional `→ @name` tag** (treatment A, in the byline) — shown **only** when a message is
   genuinely addressed to a specific participant. Target name rendered in the target's identity color.
3. **Federation `@host` provenance** (treatment A) — a dim `· {host}` suffix after a participant's
   name, shown **only** when that participant's host differs from the user's home host. Applies to both
   the sender name and the directional target name.

**Directional target resolution (hybrid):** `toAddress` if present, else the first mention in the
message text. This requires persisting the envelope `to` field (a small data-model change — see §3).

**Folded-in SP2 follow-ups (touched while in these files):**
- `ConversationRowView.rowAddress` resolves the thread's **primary agent address** (the thread's
  persisted `agentAddress`, from LOOP-1) so rows show identity-colored avatars.
- Tokenize the memory-context toolbar pill in `MessageThreadView` (`Color.accentColor` →
  `Theme.Color.accent`/`accentSoft`).

**Out of scope:** the digest capsule (→ SP4); any multi-agent threading/branching changes; multi-agent
*list* representation beyond the single primary-agent avatar; any server, protocol, runtime, or network
change (the envelope `to` is already sent and received — SP3 only persists and displays it).

## 2. Architecture & components

SP3 introduces one new focused view (`ParticipantByline`) reused by both the settled and streaming
agent messages so they can't drift, plus small **pure** helper functions (provenance, mention
extraction, directional-target derivation) that are unit-testable without a host. Views read tokens and
call the helpers; all logic lives in the pure core.

```
Persistence/
  <migrations>                    (modify) add nullable `toAddress` column to `message` table (next version)
  MessageRecord.swift             (modify) +var toAddress: String?; init(from:) copies envelope.to;
                                            memberwise init gains `toAddress: String? = nil`
UI/MessageThread/
  MessageThreadViewModel.swift    (modify) MessageGroup +directedTo: String?; build directedTo
                                            (toAddress ?? first-mention, with self/sender suppression);
                                            expose isMultiParticipant; carry toAddress through fetch
  AgentMessageView.swift          (modify) header uses ParticipantByline; gains directedTo,
                                            isMultiParticipant, homeHost params
  StreamingMessageView.swift      (modify) name row uses ParticipantByline (directedTo from stream)
  MessageGroupView.swift          (modify) pass directedTo / isMultiParticipant / homeHost down
  MessageThreadView.swift         (modify) compute homeHost from connectedIdentity, pass into groups;
                                            tokenize the memory-context toolbar pill
UI/Shared/
  ParticipantByline.swift         (NEW) name [· host]? [→ targetName [· targetHost]?] row
  ParticipantProvenance.swift     (NEW) pure: homeHost(from:), isForeignHost(_,homeHost:), hostSuffix(of:)
  MentionTarget.swift             (NEW) pure: firstMentionAddress(in: [ContentBlock]) -> String?
UI/ConversationList/
  ConversationRowView.swift       (modify) rowAddress = thread's primary agent address
AINMMessengerTests/
  DesignSystemTests/...           (NEW) render/smoke for byline variants (light+dark) + reduce-motion
  <unit tests>                    (NEW) ParticipantProvenance, MentionTarget, directedTo derivation,
                                            MessageRecord toAddress round-trip + migration
```

## 3. Data model & derivation

**Persistence:**
- Add a nullable `toAddress TEXT` column to the `message` table via a new GRDB migration registered at
  the next sequential version after the current latest (the plan will read the current latest and use
  `currentLatest + 1`). Existing rows get `NULL` (no directional tag — correct).
- `MessageRecord` gains `var toAddress: String?`. `init(from envelope: MessageEnvelope)` sets
  `self.toAddress = envelope.to`. The memberwise initializer gains `toAddress: String? = nil` (defaulted
  so existing call sites and tests compile unchanged).

**Directional-target derivation (pure):** for a `MessageGroup`, `directedTo: String?` =
- the first message's `toAddress` if non-nil and non-empty; else
- `MentionTarget.firstMentionAddress(in:)` over the first message's decoded content blocks; else `nil`.
- **Suppression:** if the resolved target equals the sender's own address or the current user's address,
  `directedTo = nil` (don't tag self-addressing or you-addressing — those are noise).
`MessageGroup` gains `directedTo: String?`, computed during group construction in the view model.

**Provenance (pure):**
- `homeHost(from address: String) -> String?` — host part (after `@`) of the current user's address.
- `isForeignHost(_ address: String, homeHost: String?) -> Bool` — true when the address's host differs
  from `homeHost` (false if `homeHost` is nil → treat all as local).
- `hostSuffix(of address: String) -> String?` — the host substring for display, or nil if no `@`.

**Multi-participant flag:** `MessageThreadViewModel` exposes `isMultiParticipant` = more than one
non-current-user participant (derived from the participants map it already maintains).

**Display name + identity color of any address** derive exactly as the existing settled path does:
local-part for the name, `ParticipantColor.color(for:)` for the color. No participant-registry lookup.

## 4. The ParticipantByline component

A single view that renders the participant byline identically for settled and streaming agents:

```
[serif name]  [· host]?   →  [serif targetName] [· targetHost]?
   ^ identity-tinted when      ^ "→" in InkSecondary; targetName in
     isMultiParticipant,         the target's identity color
     else InkPrimary
```

Inputs: `senderAddress`, `senderName`, `directedTo: String?`, `isMultiParticipant: Bool`,
`homeHost: String?`. Behavior:
- **Name:** `Theme.Typography.senderName()`; color = `ParticipantColor.color(for: senderAddress)` when
  `isMultiParticipant`, else `Theme.Color.inkPrimary`.
- **Provenance suffix:** `· {hostSuffix}` in `Theme.Typography.caption()` / `Theme.Color.inkSecondary`,
  shown only when `isForeignHost(senderAddress, homeHost:)`.
- **Directional tag:** when `directedTo != nil`, append `→ {targetName}` — the `→` in `inkSecondary`,
  the target name in `ParticipantColor.color(for: directedTo!)`; append the target's `· host` suffix
  when the target is foreign.
- **Accessibility:** the byline composes into one label, e.g. *"Researcher, on acme.test, to Assistant"*
  (sender, optional provenance, optional direction), so VoiceOver conveys identity + provenance +
  direction in one phrase.

Consumers:
- `AgentMessageView` header (rendered on the group's first message) replaces the current inline name
  `Text` with `ParticipantByline`.
- `StreamingMessageView` name row uses `ParticipantByline` for **identity-tinted name + provenance
  only**. The streaming state carries no persisted `to`, and parsing a target from partial streamed
  text would flicker mid-stream, so the streaming byline passes `directedTo: nil` — the directional tag
  appears once the message settles into `AgentMessageView`. (Name tint + provenance still render live,
  preserving avatar/name continuity with the settled message.)
- `MessageBubbleView` (human, "you") is unchanged — no byline tag/provenance for human messages in v1.

## 5. Data flow

`envelope.to` → `MessageRecord.toAddress` (persisted) → view model builds `MessageGroup.directedTo`
(`toAddress` ?? first-mention, with suppression) and `isMultiParticipant` → `MessageGroupView` passes
`directedTo` + `isMultiParticipant` + `homeHost` → `AgentMessageView` / `StreamingMessageView` →
`ParticipantByline` reads tokens + pure helpers. `homeHost` is computed once in `MessageThreadView` from
`appState.connectedIdentity`. All additions are display-only; no view-model behavior, network, runtime,
or server change.

## 6. Error handling / edge cases

- **`directedTo == sender` or `== current user`** → tag suppressed.
- **Mixed per-message targets within a group** → byline (shown once, on the first message) reflects the
  first message's target. Acceptable for v1; finer per-message direction is a future refinement.
- **Malformed / local-part-less address** → `ParticipantColor.initials` and local-part extraction are
  already total (`"?"` fallback); `hostSuffix` is nil-safe.
- **Not connected / no home host** → all participants treated as local (no provenance suffix); no crash.
- **1:1 human↔agent thread** → no name tint; directional tag only if a message is genuinely directed
  (rare) — the thread stays calm.
- **Dark mode** → every byline element uses semantic/identity tokens that already define dark
  appearances (SP1); covered by construction and asserted by render/smoke.

## 7. Testing

- **Pure unit tests (no host needed):**
  - `ParticipantProvenance`: `homeHost(from:)`, `isForeignHost(_,homeHost:)` (foreign true, same-host
    false, nil-home → false), `hostSuffix(of:)` (with/without `@`).
  - `MentionTarget.firstMentionAddress(in:)`: first mention returned; none → nil; multiple → first.
  - `MessageGroup.directedTo` derivation: `toAddress` wins; mention fallback; self-target suppressed;
    current-user-target suppressed; empty `toAddress` falls through to mention.
  - `MessageRecord` `toAddress`: `init(from:)` copies `envelope.to`; encode/decode round-trip; migration
    inserts a row with and without `toAddress` and reads both back.
- **Render/smoke (light + dark, dependency-free `UIHostingController` harness — same as SP2; the
  snapshot library still cannot be added without normalizing the hand-authored pbxproj):**
  `ParticipantByline` in: 1:1 calm (ink name, no tag/host), multi-agent (identity-tinted name),
  directional (`→ target` in target color), foreign sender (`· host`), foreign target; streaming byline
  parity; reduce-motion parity.
- **Existing suites stay green:** full iOS unit suite; UITests. Where a byline/avatar accessibility
  label legitimately changes (identity wiring / direction), update only that specific assertion.

## 8. Definition of Done

1. `toAddress` persisted on `message` via a new migration; `MessageRecord` round-trips it; existing rows
   default `NULL`.
2. Directional `→ @name` byline tag (treatment A) renders **only** when a message is genuinely directed
   (`directedTo != nil` after suppression); target name in the target's identity color.
3. Foreign-only `· {host}` provenance suffix on the sender name and the directional target name; same-
   host participants show no suffix.
4. Multi-participant name-tinting: agent sender name is identity-colored when `isMultiParticipant`;
   1:1 threads keep `InkPrimary` (stay calm).
5. `ParticipantByline` is the single shared component for settled + streaming bylines (no drift).
6. Folded SP2 follow-ups done: `ConversationRowView` uses the thread's primary agent address; the
   memory-context toolbar pill uses `Theme.Color.accent`/`accentSoft`.
7. Pure helpers unit-tested; render/smoke covers the byline variants in light + dark + reduce-motion.
8. Full iOS unit suite + UITests green; clean build; committed to `main` with `rtk proxy git`.

## 9. Build order

1. Migration + `MessageRecord.toAddress` (+ round-trip/migration tests) — the data foundation.
2. Pure helpers: `ParticipantProvenance`, `MentionTarget` (+ unit tests).
3. `MessageThreadViewModel`: `MessageGroup.directedTo` + `isMultiParticipant` (+ derivation tests).
4. `ParticipantByline` (NEW) + render/smoke variants.
5. Wire `ParticipantByline` into `AgentMessageView` (byline params threaded through `MessageGroupView`
   + `MessageThreadView` homeHost).
6. Wire `ParticipantByline` into `StreamingMessageView` (name tint + provenance only; `directedTo: nil`).
7. Folded follow-ups: `ConversationRowView.rowAddress` + memory-pill tokenization.
8. Reduce-motion parity test; full unit + UITest suites green; DoD audit; commit.

The data model lands first (everything reads it), then the pure helpers, then the shared byline, then
its consumers, then the folded follow-ups, then verification.
