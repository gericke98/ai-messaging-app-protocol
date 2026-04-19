---
phase: 15
slug: v3-2-tech-debt-off-gsd-ratification
status: approved
shadcn_initialized: false
preset: not applicable (native iOS SwiftUI + Rust server + TypeScript SDK; no web framework)
created: 2026-04-18
reviewed_at: 2026-04-18
contract_type: negative
flags:
  - dimension: spacing
    severity: non-blocking
    note: "AgentMessageView badge row uses HStack(spacing: 6) — pre-existing v3.2 value, not a multiple of 4. Phase 15 must not change it. Realign to 8pt when Phase 21 (MA-01 distinct-agent-identity) next touches AgentMessageView."
---

# Phase 15 — UI Design Contract (NEGATIVE)

> **NEGATIVE design contract.** Phase 15 is closure work — v3.2 tech debt + off-GSD package ratification. It ships ZERO new screens, ZERO new components, ZERO new visual surfaces. This document exists to (a) enforce the no-new-UI invariant on the planner and executor, (b) inventory the existing v3.2 components touched by HUMAN-UAT and XCUITest re-enables so they are NOT redesigned in passing, and (c) lock the design-system invariants those existing components must continue to honor.
>
> If the planner sees a task that introduces a new view, modifier, color, font, or component during Phase 15, that task is out of scope and must be rejected back to the orchestrator.

---

## Phase Scope vs. UI Surface

| Aspect | Phase 15 disposition |
|--------|----------------------|
| New screens | **None.** Forbidden. |
| New components | **None.** Forbidden. |
| New design tokens | **None.** Forbidden. |
| New copy strings | **None.** Forbidden (HUMAN-UAT records existing copy; ratification artifacts are markdown docs not in-app strings). |
| New iOS interactions | **None.** Forbidden. |
| Modified existing components | **Permitted only as side-effect of fixing DEBT-10/DEBT-11 SwiftUI dependency-tracking bugs.** Visual output must remain pixel-identical. |
| Modified existing visuals | **Forbidden.** |
| In-app affordance for HUMAN-UAT capture | **None.** UAT proofs are captured as packet logs / screenshots / `tmp/uat-*.{png,log}` artifacts external to the app. No in-app "verification mode" surface. |
| In-app affordance for ratification status | **None.** Ratification status lives in markdown (`DECISIONS.md`, `CHANGELOG.md`, `.planning/`). Not user-facing. |

**Why no new UI:** Per `15-RESEARCH.md` Per-Requirement UI Surface Inventory, all 19 Phase 15 requirements decompose into:
- Server-side / SDK-side work (DEBT-01, DEBT-02, DEBT-03, DEBT-05, DEBT-13, RAT-01..06) — invisible to the user
- A one-line iOS Keychain accessibility-class fix (DEBT-04) — invisible to the user
- HUMAN-UAT verification of *existing* iOS surface (DEBT-06, DEBT-07, DEBT-08, DEBT-09) — exercises existing components, no design change
- XCUITest stabilization (DEBT-10, DEBT-11) — fixes SwiftUI dependency-tracking under SwiftUI's `@Observable` / `.onChange(of:)` patterns; views render identically before and after
- Process artifacts (DEBT-10 dependency map, DEBT-11 audit doc, DEBT-12 retro doc) — markdown only

User-facing connector lifecycle UI (Add agent, paired-connectors list, revoke flow) lives in **Phase 21** (CN-01..CN-05). Multi-agent UX lives in **Phase 21** (MA-01..MA-04, MA-06, MA-07). Live agent discovery + memory preview UI lives in **Phase 20** (DISC-04, MEM-03, MEM-04). Phase 15 must not encroach on these phases' design space.

---

## Design System (existing, by reference — not redefined here)

| Property | Value | Authority |
|----------|-------|-----------|
| Tool | none (native iOS) | `CLAUDE.md` § iOS Client Stack |
| Preset | not applicable | — |
| Component library | SwiftUI (iOS 18+) | `CLAUDE.md` § iOS Client Stack |
| Icon library | SF Symbols (system) | `CLAUDE.md` § iOS Client Stack |
| Font | San Francisco via SwiftUI semantic styles (`.body`, `.subheadline`, `.caption`, `.headline`) | Apple HIG / observed in v3.2 components |
| Color system | SwiftUI semantic colors (`Color.accentColor`, `Color(.secondarySystemGroupedBackground)`, `.foregroundStyle(.secondary)`) — no hex literals | Observed in `MessageBubbleView.swift:35`, `AgentMessageView.swift:39` |
| State management | `@Observable` (iOS 17+) for view models; `@MainActor` for UI-driving state | `CLAUDE.md` § iOS App Architecture; STATE.md decisions log |

**Phase 15 must not introduce hex color literals, custom fonts, custom spacing tokens, or third-party UI libraries.** All visual decisions defer to the v3.2 component implementations as-shipped.

---

## Existing Components Touched (DO NOT REDESIGN)

The following existing v3.2 components are exercised by Phase 15 work. The planner must NOT add design-change tasks for any of them. Behavior changes are limited to the specific bugs called out per row.

| Component | File | Touched by | Permitted change | Forbidden change |
|-----------|------|------------|------------------|------------------|
| `ConversationListView` | `ios/AINMMessenger/AINMMessenger/UI/ConversationList/ConversationListView.swift` | DEBT-06, DEBT-07 (HUMAN-UAT) | None — observe and record | Any visual or layout edit |
| `ConversationListViewModel` | `ios/AINMMessenger/AINMMessenger/UI/ConversationList/ConversationListViewModel.swift` | DEBT-06, DEBT-07 (HUMAN-UAT) | None — `ValueObservation` already wired | Any change to subscription/sort/filter logic |
| `ConversationRowView` | `ios/AINMMessenger/AINMMessenger/UI/ConversationList/ConversationRowView.swift` | DEBT-07 (HUMAN-UAT) | None | Any visual edit |
| `MessageBubbleView` | `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageBubbleView.swift` | DEBT-08, DEBT-09 (HUMAN-UAT) | None | Any visual edit (bubble shape, color, padding, alignment) |
| `AgentMessageView` | `ios/AINMMessenger/AINMMessenger/UI/MessageThread/AgentMessageView.swift` | DEBT-08 (HUMAN-UAT — agent badge distinctness) | None — agent badge already renders at line 20-24 | Any change to badge copy ("Agent"), avatar size (24pt), color, padding (12), corner radius (12), background (`secondarySystemGroupedBackground`) |
| `ContentBlockRenderer` + 9 block views | `ios/AINMMessenger/AINMMessenger/UI/ContentBlocks/*.swift` | DEBT-09 (HUMAN-UAT — multi-block rendering) | None — switch dispatch over 9 block types is correct | Any change to block ordering, switch arms, padding between blocks, or per-block visuals |
| `MessageThreadView` | `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageThreadView.swift` | DEBT-10 (XCUITest re-enable) | **SwiftUI dependency-tracking fix only:** convert `scrollToMessageId` consumption from one-shot `onAppear` to `.onChange(of: scrollToMessageId)` (or set value before route push). Visual output must remain identical. | Any change to scroll position, animation, message layout, or anchor behavior other than the dependency-tracking fix |
| `MessageGroupView` | `ios/AINMMessenger/AINMMessenger/UI/MessageThread/MessageGroupView.swift` | DEBT-11 (XCUITest re-enable) | **SwiftUI dependency-tracking fix only:** promote `subThreadCache` reference type to `@Observable` so `ForEach` participates in dependency tracking. Visual output must remain identical. | Any change to grouping rules, consecutive-sender collapse, sub-thread badge placement, badge copy, or visual treatment |
| `SubThreadIndicatorView` | `ios/AINMMessenger/AINMMessenger/UI/SubThreading/SubThreadIndicatorView.swift` | DEBT-11 (XCUITest re-enable) | None — Phase 14-05 already added `parentMessageId` prop | Any change to badge appearance |
| `AvatarView` | `ios/AINMMessenger/AINMMessenger/UI/Shared/AvatarView.swift` | DEBT-08 (HUMAN-UAT — agent vs human visual distinction) | None | Any change to avatar shape, size, fallback color, or agent/human differentiation |
| `KeyManager` (non-UI but adjacent) | `ios/AINMMessenger/AINMMessenger/Crypto/KeyManager.swift` | DEBT-04 (one-line accessibility fix) | Replace `kSecAttrAccessibleAfterFirstUnlock` → `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`; add explicit `kSecAttrSynchronizable: false`. No UI implication. | Any change to public API, error surface, or wiring into `AppState` |

**XCUITest target screens (DEBT-10, DEBT-11) — verified, not redesigned:**
- `DeepLinkUITests` exercises `MessageThreadView` deep-link landing + `NotFoundToast` (existing)
- `SubThreadBranchUITests` exercises `MessageGroupView` reply-count badge + `SubThreadBannerView` back-navigation (existing)

---

## Spacing Scale (existing — by reference)

Phase 15 makes no spacing changes. The v3.2 baseline is preserved verbatim.

| Token | Value | Usage (v3.2 baseline) |
|-------|-------|------------------------|
| Inline gap | 4pt | `VStack(spacing: 4)` in `AgentMessageView` |
| Inline horizontal | 6pt | `HStack(spacing: 6)` in agent badge row |
| Bubble padding | 12pt | `.padding(12)` in `AgentMessageView`, mirrored in `MessageBubbleView` |
| Corner radius | 12pt | `RoundedRectangle(cornerRadius: 12)` |
| Avatar size | 24pt | `AvatarView(... size: 24)` for inline header |

Exceptions: **none.** Phase 15 introduces no spacing tokens.

---

## Typography (existing — by reference)

Phase 15 makes no typography changes. SwiftUI semantic type styles are used as-is.

| Role | SwiftUI style | v3.2 site of use |
|------|--------------|------------------|
| Body (message text) | `.body` (default) | `TextBlockView` |
| Sender name | `.subheadline.bold()` | `AgentMessageView:18` |
| Agent badge | `.caption.bold()` | `AgentMessageView:21` |
| Timestamp | `.caption` | `AgentMessageView:34` |

Dynamic Type, Dark Mode, and accessibility text size compatibility are existing v3.2 invariants; DEBT-08's HUMAN-UAT verifies they continue to hold for the agent badge — but the verification is observational, not corrective.

---

## Color (existing — by reference)

Phase 15 makes no color changes. SwiftUI semantic colors honor Light/Dark mode + High Contrast automatically.

| Role | SwiftUI binding | v3.2 site of use |
|------|----------------|------------------|
| Human bubble | `Color.accentColor` | `MessageBubbleView:35` |
| Agent / other-sender container | `Color(.secondarySystemGroupedBackground)` | `AgentMessageView:39` |
| Secondary text (badges, timestamps) | `.foregroundStyle(.secondary)` | `AgentMessageView:23, 35` |
| Accent reserved for | Human bubble fill, primary CTAs (existing v3.2 — unchanged) | — |

**Forbidden:** introducing any `Color(red:green:blue:)`, `Color(hex:)`, custom `Color` extension, or `.tint()` override that was not present in v3.2 main.

WCAG AA contrast (4.5:1) for the agent badge under Dark Mode + Dynamic Type XXL is verified observationally during DEBT-08 HUMAN-UAT. If the verification reveals a contrast failure, the remediation is a Phase 15 *blocker* that must be escalated to the orchestrator — **do not silently re-color** in Phase 15.

---

## Copywriting Contract

| Element | Disposition |
|---------|-------------|
| Primary CTA | **No new CTA.** Existing v3.2 CTAs ("Send", "New Conversation", "Connect") remain unchanged. |
| Empty state heading | **No new empty state.** Existing v3.2 empty states unchanged. |
| Empty state body | **No new empty state.** |
| Error state | **No new in-app error.** SDK-level errors (RAT-01..04 error-class additions) are emitted to logs/console — they do not surface to iOS UI in Phase 15. |
| Destructive confirmation | **No new destructive iOS action in Phase 15.** Connector revocation lives in Phase 21 (CN-05), not here. |
| Ratification artifact copy | Lives in markdown only (`packages/<pkg>/DECISIONS.md`, `CHANGELOG.md`). Not in-app copy. Naming convention: `0.1.1-ratified` per RESEARCH.md success criterion 7. |
| HUMAN-UAT proof copy | Lives in `.planning/phases/15-.../*-HUMAN-UAT.md` with screenshot/log artifact paths. Not in-app copy. |

**Forbidden:** adding any in-app `Text(...)`, `Alert(...)`, toast, or label string that was not present in v3.2 main.

---

## Registry Safety

Not applicable. Phase 15 introduces no third-party UI registry, no shadcn block, no external component dependency. The 8-point ratification checklist (RAT-06) covers connector npm packages — those are SDK packages with no UI surface and live outside the iOS app target. They are vetted via the dependency-pinning, log-redaction, and DECISIONS.md gates documented in `15-RESEARCH.md` § Ratification Approach, not via the design-system registry gate.

| Registry | Blocks Used | Safety Gate |
|----------|-------------|-------------|
| (none) | (none) | not applicable — no UI registry in scope |

---

## Invariants the Planner Must Enforce

Encode these as explicit checkpoints in `15-PLAN.md`:

1. **No new files under `ios/AINMMessenger/AINMMessenger/UI/`.** Diff `git status -- ios/AINMMessenger/AINMMessenger/UI/` at end of each wave; if any new file exists, the wave is rejected.
2. **No new SwiftUI `View` types.** Grep for `^struct .*: View` in modified iOS source; count must equal pre-phase count.
3. **No new public copy strings.** Diff localizable strings (or hardcoded `Text("...")`) in `ios/AINMMessenger/AINMMessenger/UI/`; net-new count must be zero.
4. **DEBT-10 / DEBT-11 fixes are dependency-tracking only.** PR diff for `MessageThreadView` and `MessageGroupView` must not touch any `.padding`, `.background`, `.foregroundStyle`, `.font`, `.frame`, or `RoundedRectangle` modifier.
5. **HUMAN-UAT proofs are external artifacts.** Each of DEBT-01, DEBT-06, DEBT-07, DEBT-08, DEBT-09 closure attaches a path under `.planning/phases/15-.../uat-proofs/` (e.g. `uat-debt-01-handshake.pcap`, `uat-debt-08-agent-badge-darkmode.png`). No in-app UI exists for capture.
6. **Visual regression check (manual).** After DEBT-10 + DEBT-11 SwiftUI fixes land, the executor runs `make uat` and visually confirms the 6 Phase 14 scenarios still render identically (no spacing, color, or layout drift). A side-by-side screenshot before/after the dependency-tracking fix is recorded in the verification artifact for each fix.

---

## Edge Cases the Planner Must Handle

| Edge case | Resolution in Phase 15 |
|-----------|------------------------|
| HUMAN-UAT for DEBT-08 reveals agent badge fails WCAG AA contrast under Dark Mode + Dynamic Type XXL | **STOP. Do not re-color in Phase 15.** Escalate to the orchestrator as a blocker. Phase 15 is closure work; if a real contrast bug surfaces, it becomes a separate scoped task (likely lifted into Phase 21 alongside MA-01 distinct-identity work). |
| HUMAN-UAT for DEBT-09 reveals a block-renderer bug (e.g. ImageBlock fails to render in a 4-block sequence) | Permitted to fix the renderer bug in Phase 15 (it falls under "behavior is broken, observed under live UAT") — but **the fix must not change the block's visual treatment**, only its dispatch/rendering correctness. |
| DEBT-10 fix requires changing `MessageThreadView`'s scroll behavior beyond the `.onChange(of:)` swap | **STOP. Out of scope.** Document the deeper fix as a Phase 20 or Phase 21 follow-up (whichever owns thread-view rework) and ship Phase 15 with the original `XCTSkip` and updated comment explaining why the simple fix was insufficient. |
| Ratification audit (RAT-01..04) reveals a connector-side bug whose fix would surface a new in-app error | **STOP. Do not surface a new in-app error in Phase 15.** Connector-side error mapping (RAT-06 row 5) creates new error classes consumed by the SDK only; iOS surfaces them as the existing generic connection-error toast or no-op (ratification phase). User-facing error surface for connectors is Phase 16 (AU-01 handshake rejection) or Phase 21 (CN-04 paired-connectors list). |

---

## Checker Sign-Off

The 6-dimension design checker should validate this contract as a NEGATIVE design contract. Each dimension is satisfied by absence-of-change rather than presence-of-tokens.

- [ ] Dimension 1 Copywriting: PASS — no new copy introduced; existing v3.2 copy unchanged
- [ ] Dimension 2 Visuals: PASS — no new visuals introduced; existing v3.2 components untouched except for SwiftUI dependency-tracking bugs in `MessageThreadView` and `MessageGroupView` whose visual output is pixel-identical
- [ ] Dimension 3 Color: PASS — no new color tokens; SwiftUI semantic colors unchanged
- [ ] Dimension 4 Typography: PASS — no new type tokens; SwiftUI semantic styles unchanged
- [ ] Dimension 5 Spacing: PASS — no new spacing tokens; v3.2 spacing baseline preserved
- [ ] Dimension 6 Registry Safety: PASS — no UI registry in scope; connector npm packages vetted via the ratification 8-point checklist (separate, non-UI)

**Approval:** pending
