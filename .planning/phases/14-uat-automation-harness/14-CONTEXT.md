# Phase 14: UAT Automation Harness - Context

**Gathered:** 2026-04-18
**Status:** Ready for planning

<domain>
## Phase Boundary

Automated XCUITest coverage for the six Phase 13 acceptance scenarios so UAT runs hermetically from `xcodebuild test` — no human taps required. Phase delivers:

- `AINMMessengerUITests` target in `AINMMessenger.xcodeproj` with a shared scheme
- Launch-argument-driven seed bootstrap (`-UITestSeed <fixture>`) that provisions backend state per scenario
- Six XCUITest cases mapped 1:1 to the scenarios in `13-HUMAN-UAT.md` (tool call indicator states, @-mention autocomplete, agent discovery flow, memory context sheet, sub-thread branch/navigate, `ainm://` deep links)
- `make uat` orchestrator that boots the Rust AINM server, runs the suite, and tears down
- Placeholder CI workflow file documenting the invocation for later matrix activation
- `13-HUMAN-UAT.md` flipped from `partial` → `complete` with tests 1–6 passing

Out of phase scope: broader a11y audit beyond test-touched views, full CI matrix activation, snapshot/image-diff testing, test coverage for Phases 10–12 scenarios.

</domain>

<decisions>
## Implementation Decisions

### Seed bootstrap architecture
- **D-01:** GRDB-direct seed loader. Launch arg `-UITestSeed <fixture>` triggers an in-app `SeedLoader` that resets `AppDatabase` and inserts fixture records (threads, participants, messages, tool calls, memory frames, sub-thread parent messages) directly into GRDB. `AINMClient` still connects for live events, but historical state never round-trips through the server.
- **D-02:** Fixtures are JSON files bundled in the test target under `Fixtures/scenario-NN-<name>.json`, decoded by `SeedLoader` into GRDB records. Version-controlled, diffable. Launch arg value names the fixture (e.g. `-UITestSeed scenario-01-tool-call`).
- **D-03:** `SeedLoader` lives in the app target gated by `#if DEBUG`. `AINMMessengerApp` checks `ProcessInfo.processInfo.arguments` for `-UITestSeed` at launch and invokes the loader before the normal `AppState.setup()` path. Release builds do not include this code.
- **D-04:** Seed loader resets Keychain-stored identity and provisions a deterministic `uat-user@localhost` with a fixed-seed Ed25519 keypair on every run. Guarantees cross-test isolation and makes signature assertions reproducible.

### Server lifecycle
- **D-05:** Top-level `Makefile` with a `uat` target orchestrates the full run: starts `cargo run --bin ainm-server --uat` in the background, waits for readiness, runs `xcodebuild test -scheme AINMMessengerUITests`, tears the server down on exit (including trap handlers for Ctrl-C). This is the documented local and CI invocation.
- **D-06:** Readiness check: Makefile polls `GET http://localhost:8080/health` until 200 OK with a 30-second timeout. The demo server already exposes `/health` (`demo/src/server.rs`); if any gaps surface during research they're resolved as part of this phase.
- **D-07:** Server runs with a `--uat` flag that forces an ephemeral SQLite database in a temp directory, logs at INFO to a known file (`tmp/uat-server.log`), and binds fixed port `8080`. Every run starts from a clean slate; port collisions surface immediately rather than taint results.
- **D-08:** CI surface: `Makefile` + docs entry (`docs/testing.md` or similar) + a placeholder `.github/workflows/ios-uat.yml` that references the Makefile but is disabled (workflow_dispatch only, not triggered on push/PR). Phase goal explicitly defers full matrix wiring — the placeholder gives the future phase a concrete starting point.

### Accessibility identifier strategy
- **D-09:** Targeted retrofit only. Walk each HUMAN-UAT scenario, identify the minimum set of views/controls tests must query, add `accessibilityIdentifier` just to those — expected footprint ~20–40 touches across ~10 files. Broader a11y audit is not in scope for this phase.
- **D-10:** Naming convention: `screen.component.action` dot-path (e.g. `thread.toolbar.memoryPill`, `toolCall.indicator.expandButton`, `agentDiscovery.list.cell.{agentId}`). Hierarchical, greppable, consistent.
- **D-11:** Identifiers are declared in a single `UITestIdentifiers.swift` file exposing an `enum UITestID { static let memoryPill = "thread.toolbar.memoryPill" ... }`. File membership includes both the app target (gated `#if DEBUG` if needed) and the UI test target so Swift code on both sides imports the same constants — compile-time typo protection.
- **D-12:** Stateful indicators (tool call spinner/checkmark/X/clock, streaming cursor, etc.) get a distinct identifier per state rather than a single identifier with changing `accessibilityValue`. Tests assert existence (`app.images["toolCall.indicator.status.completed"].exists`) rather than reading values — simpler and less brittle to state-string drift.

### Dynamic event provisioning
- **D-13:** Runtime event provisioning uses an in-app `TestEventInjector` gated by `#if DEBUG`. A second launch arg (e.g. `-UITestEvents <script>.json`) activates it; the injector schedules synthetic `StreamEvent`, tool-call lifecycle notifications, presence updates, and memory push notifications into the existing `AINMClient` callback pipeline. No real LLM, no server-side event scripting. Exercises the real UI rendering path from notification → ViewModel → SwiftUI.
- **D-14:** Event scripts express timing as explicit millisecond offsets in JSON (`[{"atMs": 0, "event": "toolcall.started", ...}, {"atMs": 800, "event": "toolcall.progress", ...}, {"atMs": 2000, "event": "toolcall.completed", ...}]`). Tests use generous `waitForExistence` timeouts to absorb slow-CI jitter.
- **D-15:** `@-mention` autocomplete is driven by real XCUITest typing against a pre-seeded participant list. Seed fixture populates the thread with known humans and agents; test calls `.typeText("@Re")` on the composer and asserts the autocomplete popup contents, then selects a row by identifier. No injection path — the scenario tests the keystroke-triggered resolver.
- **D-16:** Deep-link scenario (#6) is delivered via `xcrun simctl openurl booted ainm://thread/{id}` invoked from the test's `setUp` (shell-out via `Process`). This exercises the real `onOpenURL` handler end-to-end, matching how HUMAN-UAT describes the manual test. Tests resolve the booted simulator UDID via a small helper.

### Claude's Discretion
- Exact XCUITest timeout values (base `waitForExistence`, per-scenario extensions for streaming)
- Fixture JSON schema shape — concrete field names, nested vs flat record layout
- `TestEventInjector` implementation details — backing queue, scheduling primitive (Timer vs DispatchWorkItem vs Task.sleep)
- `make uat` bash style — which `trap` handlers, log file tee, simulator boot/shutdown hygiene
- Simulator destination strings (iPhone 16 vs 16 Pro, which iOS version) — align with existing `ios-ci.yml`
- Whether to include a `make uat-debug` target that keeps the server running after tests for inspection
- How to update `13-HUMAN-UAT.md` when tests go green (likely scripted via a small post-test step or the existing `/gsd:verify-work` path)

### Folded Todos
None — no pending todos matched Phase 14.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 13 UAT & Implementation (the tests being automated)
- `.planning/phases/13-ai-native-features-platform/13-HUMAN-UAT.md` — The six scenarios being automated. Each test must assert the user-observable behavior described here.
- `.planning/phases/13-ai-native-features-platform/13-VERIFICATION.md` — Phase 13 goal-backward verification; clarifies what "user-observable" means for each scenario.
- `.planning/phases/13-ai-native-features-platform/13-CONTEXT.md` — Design decisions behind the views under test (tool call UI, mentions, agent discovery, memory sheet, sub-threading, deep links).
- `.planning/phases/13-ai-native-features-platform/13-01-PLAN.md` through `13-05-PLAN.md` — Implementation plans for each scenario's underlying feature; identifies the concrete files/views tests must query.
- `.planning/phases/13-ai-native-features-platform/13-UI-SPEC.md` — UI design contract with the visual states and controls tests must assert on.

### Earlier phase context (foundations the harness touches)
- `.planning/phases/11-core-messaging-ui/11-CONTEXT.md` — Navigation stack, thread/composer layout, GRDB write-through cache patterns.
- `.planning/phases/12-streaming-interactive-blocks/12-CONTEXT.md` — Streaming callback wiring (`setStreamCallbacks` on `AINMClient` actor), `StreamingViewModel` pattern the injector must match.
- `.planning/phases/10-foundation/10-CONTEXT.md` — `AppState.setup()` lifecycle, Ed25519 Keychain identity, WebSocket client wiring that `SeedLoader` must slot into.

### iOS Codebase integration points
- `ios/AINMMessenger/AINMMessenger/App/AINMMessengerApp.swift` — Launch entry point; where `-UITestSeed`/`-UITestEvents` arg detection and SeedLoader invocation hooks in ahead of `appState.setup()`.
- `ios/AINMMessenger/AINMMessenger/App/AppState.swift` — @MainActor @Observable root state; fixture data lands in its published properties through GRDB observation.
- `ios/AINMMessenger/AINMMessenger/Persistence/AppDatabase.swift` — GRDB DatabasePool and migrations; SeedLoader uses its write APIs to insert fixture records.
- `ios/AINMMessenger/AINMMessenger/Networking/AINMClient.swift` — WebSocket + callback pipeline; `TestEventInjector` emits synthetic notifications here.
- `ios/AINMMessenger/AINMMessenger/App/DeepLinkHandler.swift` — Parses `ainm://` URLs; exercised by scenario #6 through `simctl openurl`.
- `ios/AINMMessenger/AINMMessenger.xcodeproj/project.pbxproj` — Target definition site for the new `AINMMessengerUITests` target and its shared scheme.
- `ios/AINMMessenger/AINMMessenger.xcodeproj/xcshareddata/xcschemes/` — Location for the new `AINMMessengerUITests.xcscheme` shared scheme.

### Rust server integration
- `demo/src/server.rs` — AINM server implementation, including the existing `/health` endpoint the Makefile polls for readiness.
- `demo/src/main.rs` — CLI entry; where a `--uat` flag argument and ephemeral-DB wiring are added.
- `demo/src/db.rs` — Database initialization; ephemeral SQLite temp-dir path lives here.
- `demo/Cargo.toml` — Binary definitions (`ainm-server`) the Makefile targets.

### Existing CI reference
- `.github/workflows/ios-ci.yml` — Existing macos-14 + Xcode 16 + iOS 18 simulator matrix; new `ios-uat.yml` follows its destination string conventions.

### Protocol specification (assertion semantics)
- `spec/streaming.md` — Stream event model (`started`/`delta`/`completed`/`error`) the injector must emit faithfully.
- `spec/tool-visibility.md` — Tool call lifecycle events and the UI states asserted in scenario #1.
- `spec/memory.md` — Active context frame + archival fact shape seeded for scenarios #4.
- `spec/threading.md` — Sub-thread structural rules (one level, parent linkage) seeded for scenario #5.
- `spec/transport.md` — `ainm.initialize` handshake the readiness check implicitly relies on.

### Tech stack
- `CLAUDE.md` §iOS Client Stack — SwiftUI + Swift Concurrency + GRDB.swift + @Observable conventions applied to all new Swift code in this phase.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AppDatabase` (GRDB) — DatabasePool and write APIs are the insertion surface for fixture records; migrations do not need to change for this phase.
- `AINMClient` callback pipeline (Phase 12 D-01 `setStreamCallbacks`) — `TestEventInjector` reuses the same shape to deliver synthetic events to `StreamingViewModel`, `AppState.toolCallStates`, `AppState.contextFrames`, etc.
- `AppState` @MainActor @Observable state (Phase 10+) — fixture data flows into its properties through existing GRDB `ValueObservation` paths; no new binding work required.
- `DeepLinkHandler.parse(url:)` — already correct for scenario #6; harness only needs to deliver the URL.
- Existing `/health` endpoint in `demo/src/server.rs` — readiness gate is ready to reuse.

### Established Patterns
- Write-through cache: all server data flows through GRDB before UI. SeedLoader extends this pattern — fixtures land in GRDB, UI observes them via existing paths.
- `#if DEBUG` gating for dev-only hooks (precedent in the codebase to follow for SeedLoader + TestEventInjector).
- Swift 6 strict concurrency, @MainActor on state, Sendable records — harness Swift code must comply.
- GitHub Actions on macos-14 / Xcode 16 / iOS 18 (from `ios-ci.yml`) — UAT workflow mirrors this.

### Integration Points
- `AINMMessengerApp.body` — pre-setup hook site for launch-arg detection (SeedLoader + TestEventInjector boot).
- `AINMClient` message/stream callback wiring — event injector plugs in alongside real server events; tests assert the UI reacts identically regardless of source.
- `AppDatabase` write operations — fixture insertion path. Test-only DB reset logic wraps existing APIs rather than introducing a new migration.
- Xcode scheme definitions — new `AINMMessengerUITests.xcscheme` must be marked shared so `xcodebuild -scheme AINMMessengerUITests` works from the CLI.
- Top-level `Makefile` — does not exist yet; this phase creates it alongside the `uat` target.

</code_context>

<specifics>
## Specific Ideas

- Launch arg shape: `-UITestSeed <fixture-name>` per roadmap success criterion; introduce companion `-UITestEvents <script-name>` for dynamic event provisioning (D-13).
- Test naming: one XCUITest class per scenario (`ToolCallIndicatorUITests`, `MentionAutocompleteUITests`, `AgentDiscoveryUITests`, `MemoryContextUITests`, `SubThreadBranchUITests`, `DeepLinkUITests`) — each method inside asserts a slice of the HUMAN-UAT expected text.
- Identifier examples to seed downstream planning: `thread.toolbar.memoryPill`, `toolCall.indicator.row.{callId}`, `toolCall.indicator.status.running|completed|error|timedOut`, `composer.mention.autocomplete.list`, `composer.mention.autocomplete.row.{identity}`, `agentDiscovery.list.cell.{agentId}`, `agentDiscovery.detail.startConversation`, `memory.sheet.archivalPreview`, `memory.archival.row.{factId}`, `thread.message.row.{messageId}.branchAction`, `subThread.banner.backButton`.
- Keep `13-HUMAN-UAT.md` update step explicit in the plan: when all 6 tests green, update status to `complete`, fill `passed: 6 / pending: 0`, cite automated run rather than human observation. Manual step initially; scripting is Claude's discretion.

</specifics>

<deferred>
## Deferred Ideas

- **Broader accessibility audit** — full-app `accessibilityIdentifier`/`accessibilityLabel` coverage beyond tests' needs. Belongs in a dedicated a11y phase.
- **Snapshot/image-diff testing** — visual regression detection for rendered blocks and streaming UI. Different testing discipline; add later if needed.
- **CI matrix activation on push/PR** — flipping `ios-uat.yml` from `workflow_dispatch`-only to triggered. Success criterion #6 says "wired into the GitHub Actions matrix later"; this is the "later."
- **UAT automation for Phases 10–12 scenarios** — onboarding, conversation list, streaming bubble rendering, interactive block actions. This phase scopes strictly to Phase 13.
- **Real-protocol end-to-end fidelity tests** — running a scripted fixture agent on the server so seed state travels the full wire path (option B of seed bootstrap). Kept on the shelf if hermetic GRDB seeding proves insufficient for catching regressions.
- **Driver helper hooks on `AppState`** (HTTP control endpoint for direct state pokes) — rejected in favor of the injector approach for this phase; revisit if scenarios emerge that can't be expressed as pushed events.
- **Step-advanced event injection** — pulling mode for the event injector. Deferred in favor of ms-offset timing; add if streaming cursor assertions become flaky.

### Reviewed Todos (not folded)
None — no pending todos matched Phase 14 during cross-reference.

</deferred>

---

*Phase: 14-uat-automation-harness*
*Context gathered: 2026-04-18*
