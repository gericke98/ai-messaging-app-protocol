# Runbook — A2A Arc Visual Verification (2026-06-08)

Verifies the multi-agent / A2A UI arc (SP3 byline + tinting + provenance, the participant roster,
and the SP4 digest capsule) renders correctly.

> **Scope note:** the server runtime does not generate agent-to-agent traffic today (human ↔ single
> agent only; no inter-agent routing). This runbook verifies the **UI** against a *seeded* multi-agent
> thread through the real app's real code paths. Real runtime A2A generation is separate future work.

## A. Automated (CI-able)

Runs the seeded UITest and captures collapsed/expanded screenshots as test attachments.

```bash
xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessengerUITests \
  -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd \
  test -only-testing:AINMMessengerUITests/A2ACapsuleUITests
```

Open the resulting `.xcresult` (Xcode → Report navigator, or `xcrun xcresulttool`) to view the
`a2a-capsule-collapsed` / `a2a-capsule-expanded` screenshots.

## B. Manual visual check (eyeball the design)

1. In Xcode, edit the **AINMMessenger** scheme → Run → Arguments → "Arguments Passed On Launch",
   add two entries: `-UITestSeed` and `scenario-07-a2a-burst`.
2. Run the app on the iPhone 16 simulator. It opens already "connected" as `uat-user@localhost`.
3. Open the **Launch readiness** conversation.
4. Confirm, in order — first in light mode, then re-run in dark mode:
   - [ ] A **collapsed capsule** sits between your two messages: overlapped avatars +
         `Alpha ↔ Analyst` (each in its own identity color) + `3 messages`.
   - [ ] Both of your messages ("Are we ready to launch?" and "Great — launching now.") render
         as normal human bubbles **outside** the capsule.
   - [ ] Tap the capsule → it **expands in place** to the three agent messages, each with its
         identity-tinted serif name; **`Analyst`** shows a dim **`· acme.test`** provenance suffix
         (foreign host); the Analyst message shows a directional **`→ Alpha`** tag.
   - [ ] Tap **Collapse** → it folds back to the digest card.
   - [ ] The thread header shows **3 participants**.
   - [ ] Dark mode: all of the above renders with correct dark tokens (no white-on-white).
