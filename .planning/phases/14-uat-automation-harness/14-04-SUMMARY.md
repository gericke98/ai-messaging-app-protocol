---
phase: 14-uat-automation-harness
plan: 04
subsystem: ios-ui-test-harness
tags:
  - ios
  - ui-testing
  - swift-6-concurrency
  - event-injection
  - bundle-resources
  - tdd
dependency-graph:
  requires:
    - 14-01 (complete) — AINMMessengerApp.swift TODO(14-04) anchor, UITestID namespace, launch-arg parsing
    - Phase 13 AppState.handleToolEvent(threadId:event:) and AppState.contextFrames[String] (both @MainActor)
    - Phase 13 ToolCallEvent flat Codable struct; ContextFrame + nested Frame/ActiveContext/MessageSummary/ArchivalFact/FrameParticipant
  provides:
    - TestEventInjector.schedule(script:for:) — @MainActor static entry point used by unit tests
    - TestEventInjector.scheduleIfRequested(script:for:) async — launch-time entry point used by AINMMessengerApp.task
    - Codable EventScript / ScriptEntry / ScriptProgress / ScriptFramePayload schema (covers 5 event types)
    - ios/AINMMessenger/AINMMessenger/Testing/EventScripts/scenario-01-events.json (tool-call lifecycle, 3 events @ 0/800/2000ms)
    - ios/AINMMessenger/AINMMessenger/Testing/EventScripts/scenario-04-events.json (context.frame push, 1 event @ 500ms)
    - docs/testing.md "Event script schema" section
  affects:
    - ios/AINMMessenger/AINMMessenger/App/AINMMessengerApp.swift (TODO(14-04) block replaced with live scheduleIfRequested call)
tech-stack:
  added: []
  patterns:
    - "@MainActor-isolated final class + Task { @MainActor in ... } for Swift 6 Sendable-clean dispatch"
    - "Static activeInjectors pin ([TestEventInjector]) keeping injectors alive across the launch-time async boundary"
    - "Strong self capture inside Task bodies (safe because Tasks complete and release) — avoids weak-self dropping pre-dispatch"
    - "Codable ScriptEntry as field-union of all event types (optional fields populated by type) — forward-compat unknown types decode + log-and-skip"
    - "Translator pattern: flat ScriptFramePayload -> nested ContextFrame.Frame assembled by injector at dispatch time"
    - "Bundle.main.url(forResource:withExtension:) with synchronized root group — JSON fixtures ship automatically as app-target resources"
key-files:
  created:
    - ios/AINMMessenger/AINMMessenger/Testing/TestEventInjector.swift (255 lines)
    - ios/AINMMessenger/AINMMessenger/Testing/EventScripts/scenario-01-events.json (30 lines)
    - ios/AINMMessenger/AINMMessenger/Testing/EventScripts/scenario-04-events.json (16 lines)
    - ios/AINMMessenger/AINMMessengerTests/TestingHelpersTests/TestEventInjectorTests.swift (123 lines)
  modified:
    - ios/AINMMessenger/AINMMessenger/App/AINMMessengerApp.swift (TODO(14-04) → live scheduleIfRequested call; -5 / +6 lines)
    - docs/testing.md (+54 lines; "Event script schema" section)
decisions:
  - "Strong self inside Task + static activeInjectors pin instead of weak self + caller-held reference: AINMMessengerApp.task returns before events fire, so relying on the caller to hold the injector would break. Static pin is @MainActor so no sync needed."
  - "Codable ScriptEntry as a flat union (all fields optional, selected by type) rather than a Codable enum with associated values: JSON ergonomics win for hand-authored scripts; unknown event types decode cleanly (XCTAssertNoThrow test enforces) and are logged + skipped at fire time."
  - "Injector assembles ContextFrame synthetically from ScriptFramePayload (building MessageSummary / ArchivalFact stubs from the id arrays) rather than requiring full nested JSON: reduces authoring friction for scenario scripts while still exercising the real ContextFrame wire-shape code path."
  - "ToolCallEvent status semantics: nil = new / in-progress (handleToolEvent creates running state), 'success' = completed, 'error' = error. The injector maps tool.call.started/progress → nil status, tool.call.completed → 'success' (or user override), tool.call.error → 'error'."
  - "Participant type heuristic in buildContextFrame: identities starting with 'helper' or containing 'agent' → 'agent', else 'human'. Simple + predictable; override by naming fixture identities consistently with this rule."
metrics:
  duration_minutes: 26
  tasks_completed: 3
  files_created: 4
  files_modified: 2
  commits: 3
  completed_date: "2026-04-18"
---

# Phase 14 Plan 04: TestEventInjector + Event Scripts Summary

Ships the runtime event harness for UAT scenarios 1 (tool call lifecycle) and 4 (memory context push). A `@MainActor`-isolated `TestEventInjector` loads scripted JSON at launch (when `-UITestEvents scenario-NN-events` is present), schedules each entry at its `atMs` offset via `Task.sleep(for:)`, and dispatches into the SAME `AppState.handleToolEvent(...)` / `AppState.contextFrames[...]` code paths that `AINMClient` wires as server-event callbacks — so XCUITest scenarios hit the exact production UI rendering graph even though no real LLM is running.

## Tasks Completed

| Task | Name                                                                       | Commit    | Files                                                                                                                                    |
| ---- | -------------------------------------------------------------------------- | --------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| 1    | TestEventInjector + Codable schema + 4 unit tests (TDD)                    | `e6081f0` | `Testing/TestEventInjector.swift`, `AINMMessengerTests/TestingHelpersTests/TestEventInjectorTests.swift`                                 |
| 2    | scenario-01/04 event scripts + bundle shipment + schema docs                | `be7ca4d` | `Testing/EventScripts/scenario-01-events.json`, `Testing/EventScripts/scenario-04-events.json`, `docs/testing.md`                        |
| 3    | Replace TODO(14-04) with live TestEventInjector call in app bootstrap      | `0aa214f` | `App/AINMMessengerApp.swift`                                                                                                             |

## Contract Surface for Downstream Plans

### ToolCallEvent enum cases — actually a flat struct (not an enum)

The 14-04 plan referenced `ToolCallEvent.started/.progress/.completed/.error` as if it were an enum with associated values. **The real `ToolCallEvent` (Models/ToolEvent.swift) is a flat `Codable` struct** with optional fields, and the `status` field (nil | "success" | "error") drives the lifecycle transition in `AppState.handleToolEvent`:

```swift
struct ToolCallEvent: Codable, Sendable {
    let callId: String
    let name: String?
    let description: String?
    let input: [String: AnyCodableValue]?
    let progress: ToolProgress?
    let resultSummary: String?
    let durationMs: Int?
    let status: String?          // nil = new/progress; "success"|"error" = terminal
    let errorMessage: String?
}
```

The injector maps script event types to struct instances:

| Script `type`         | Resulting `ToolCallEvent` status | `handleToolEvent` effect                                         |
| --------------------- | -------------------------------- | ---------------------------------------------------------------- |
| `tool.call.started`   | `status = nil`                   | Create new `ToolCallState(.running)` + schedule 5-min timeout    |
| `tool.call.progress`  | `status = nil`, `progress` set   | Update existing state's `progress` property                      |
| `tool.call.completed` | `status = "success"`             | Transition to `.completed`; set `resultSummary` + `durationMs`   |
| `tool.call.error`     | `status = "error"`               | Transition to `.error`; set `errorMessage`                       |

**`ToolCallState` (the Observable UI-state class) status enum:**

```swift
enum ToolCallStatus: String, Sendable {
    case running, completed, error, timedOut
}
```

**Existing `toolCallStates` indexing:** `var toolCallStates: [String: [ToolCallState]]` — keyed by `threadId`, each entry is an array of per-`callId` states. `ToolCallState.id` is the `call_id` string.

### ContextFrame initializer signature — nested, not flat

The 14-04 plan pseudocode referenced a flat `ContextFrame(threadId:summary:activeMessages:archivalFactIds:participantIdentities:receivedAt:)` initializer. **The real `ContextFrame` (Models/MemoryModels.swift) is deeply nested:**

```swift
struct ContextFrame: Codable, Sendable {
    let threadId: String
    let frame: Frame                                // <-- nested!

    struct Frame {
        let activeContext: ActiveContext            // nested ActiveContext
        let conversationSummary: String?
        let archivalFacts: [ArchivalFact]           // full Codable rows
        let participants: [FrameParticipant]
    }

    struct ActiveContext {
        let recentMessages: [MessageSummary]        // not just ids
        let windowStart: String                     // RFC 3339 timestamp
        let messageCount: Int
    }

    struct MessageSummary { let id, from, timestamp, summary: String }
    struct ArchivalFact   { let factId, key: String; let value: AnyCodableValue; ... }
    struct FrameParticipant { let identity, type, presence: String }
}
```

**Injector translation strategy:** the flat `ScriptFramePayload` (summary + activeMessages[id] + archivalFactIds[] + participantIdentities[]) is expanded by `TestEventInjector.buildContextFrame` into the full nested shape at dispatch time:

- `activeMessages[String]` → `[MessageSummary(id, from: "uat-user@localhost", timestamp: now, summary: "Seeded active message \(id)")]`
- `archivalFactIds[String]` → `[ArchivalFact(factId, key: factId, value: .string("seeded archival fact \(id)"), tags: nil, storedBy: "uat-user@localhost", storedAt: now)]`
- `participantIdentities[String]` → `[FrameParticipant(identity, type: classifier(identity), presence: "active")]` where classifier returns `"agent"` for identities starting with `helper` or containing `agent`, else `"human"`

This keeps authoring simple (scenarios only need id arrays) while still producing a wire-shape-faithful `ContextFrame`.

### AINMMessengerApp.swift call site (post-14-04)

```swift
.task {
    #if DEBUG
    // launch-arg parsing (seedFixture, eventScript)
    // TODO(14-03): SeedLoader.preparePristineDatabase + UITestIdentityBootstrap (Wave 2 sibling)
    #endif

    try? appState.setup()

    #if DEBUG
    // TODO(14-03): SeedLoader.loadFixtureIfRequested (Wave 2 sibling)

    if let eventScript {
        await TestEventInjector.scheduleIfRequested(script: eventScript, for: appState)
    }
    #endif

    bootstrapComplete = true
}
```

Both `TODO(14-03)` anchor comments are preserved — 14-03 fills them without conflict.

## Event Script Schema (Authored)

See `docs/testing.md` "Event script schema" for the full per-field spec. Key points:

- 5 event `type` strings supported: `tool.call.started`, `tool.call.progress`, `tool.call.completed`, `tool.call.error`, `context.frame`.
- No additional event types added beyond what 14-RESEARCH.md recommended — `tool.call.error` was included for defense-in-depth (scenarios 1/4 don't currently need it) and streaming.* was intentionally omitted (no UAT scenario exercises dynamic streaming pushes).
- Unknown `type` strings decode without throwing (forward-compat) and are logged + skipped at fire time (`test_eventScript_decodesUnknownEventTypesWithoutThrowing` enforces).

## Cross-Plan Coordination Contract

### For 14-03 (parallel sibling, Wave 2):

The scenario-04 fixture MUST seed archival facts with these exact ids (referenced by `scenario-04-events.json`):

- `fact-coffee`
- `fact-meetings`

And MUST seed participants:

- `uat-user@localhost` (human)
- `helper@localhost` (agent)

And MUST seed `thread-01` with at least one message `msg-01` (referenced by `scenario-04-events.json` as `activeMessages: ["msg-01"]`).

The scenario-01 fixture MUST seed `thread-01` + a pre-existing tool call row with `callId: "call-01"` (matches both the event script and the XCUITest query `UITestID.ToolCall.indicatorRow(callId: "call-01")`).

**If 14-03 uses different ids, 14-04 is responsible for updating the event scripts to match** (per plan §Action for Task 2). Since 14-03 was not committed to the worktree at the time of 14-04 execution, 14-04 used canonical ids; if 14-03 diverges, this SUMMARY + scenario-04-events.json line `"archivalFactIds": ["fact-coffee", "fact-meetings"]` is the coordination contract.

### For 14-05 (accessibility identifier retrofit):

No new identifiers required from 14-04 — the injector fires into existing AppState state, which drives existing views. 14-05 retrofits those views with identifiers regardless of how the state got populated.

### For 14-06 (XCUITest scenarios + green run):

- `-UITestEvents scenario-01-events` launch arg enables the tool-call lifecycle script. Combined with `-UITestSeed scenario-01-tool-call` (from 14-03), scenario 1 tests can assert the three indicator states (running at ~0ms, progress at ~800ms, completed at ~2000ms) using `waitForExistence(timeout: 5.0)` with generous buffers.
- `-UITestEvents scenario-04-events` launch arg enables the context-frame push at 500ms. Scenario 4 test should navigate to `thread-01`, wait ~1s, then assert `memory.toolbarPill` is visible and the sheet's archival rows include `memory.archival.row.fact-coffee` and `memory.archival.row.fact-meetings`.

## Deviations from Plan

### Rule 1 - Bug fix: Strong self + static pin to keep injector alive

- **Found during:** Task 1 (test_injector_dispatchesToolEvent_updatesAppState test failure on first run)
- **Issue:** The plan's pseudocode used `Task { @MainActor [weak self] in ... self?.fire(entry) }` and instantiated the injector locally inside `static func schedule(...)`. Since the static function returned synchronously without retaining the injector, `weak self` became `nil` before any scheduled Task fired. Result: test saw empty `toolCallStates` array.
- **Fix:** Added `private static var activeInjectors: [TestEventInjector] = []` as a @MainActor pin. Injectors are appended on creation in both `schedule(script:for:)` and `scheduleIfRequested(script:for:)`. Task closures now capture `self` strongly (safe — Tasks complete and release).
- **Files modified:** `ios/AINMMessenger/AINMMessenger/Testing/TestEventInjector.swift`
- **Commit:** `e6081f0`

### Rule 2 - Missing functionality: Extra integration test for context-frame path

- **Found during:** Task 1 (test authoring)
- **Issue:** The plan only prescribed one integration test (`test_injector_dispatchesToolEvent_updatesAppState`). But the injector has two distinct dispatch paths (tool event vs context frame) and both need regression coverage — a broken context-frame translator would silently land `nil` into `appState.contextFrames`.
- **Fix:** Added `test_injector_dispatchesContextFrame_updatesAppState` with a 0ms context.frame script, asserting the resulting `ContextFrame` has the expected `conversationSummary` and `activeContext.recentMessages[0].id`.
- **Files modified:** `AINMMessengerTests/TestingHelpersTests/TestEventInjectorTests.swift`
- **Commit:** `e6081f0`

## Authentication Gates

None occurred during execution.

## Verification Results

| Check                                                                      | Result                                                                         |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| Task 1 acceptance: 4 unit tests pass                                        | PASS (iPhone 16e / iOS 26.2; all 4 green incl. 2 integration tests)            |
| Task 1 acceptance: Swift 6 strict concurrency clean (no DispatchQueue, no @unchecked) | PASS                                                                           |
| Task 1 acceptance: appState.handleToolEvent + contextFrames[...] uncommented | PASS (grep with comment-stripping)                                             |
| Task 1 acceptance: Release build succeeds (proves #if DEBUG gate)           | PASS                                                                           |
| Task 2 acceptance: scenario-01 has 3 events (started/progress/completed) at 0/800/2000ms | PASS                                                                           |
| Task 2 acceptance: scenario-04 has 1 context.frame event                     | PASS                                                                           |
| Task 2 acceptance: both JSON files ship in AINMMessenger.app bundle         | PASS (verified via find on built .app/ path)                                   |
| Task 2 acceptance: docs/testing.md has "Event script schema" section       | PASS                                                                           |
| Task 3 acceptance: TODO(14-04) gone, TestEventInjector.scheduleIfRequested present, if let eventScript preserved, TODO(14-03) intact | PASS                                                                           |
| Task 3 acceptance: Debug build, Release build, UI test build-for-testing all succeed | PASS                                                                           |

## Self-Check: PASSED

- [x] `ios/AINMMessenger/AINMMessenger/Testing/TestEventInjector.swift` — EXISTS (verified)
- [x] `ios/AINMMessenger/AINMMessenger/Testing/EventScripts/scenario-01-events.json` — EXISTS (verified)
- [x] `ios/AINMMessenger/AINMMessenger/Testing/EventScripts/scenario-04-events.json` — EXISTS (verified)
- [x] `ios/AINMMessenger/AINMMessengerTests/TestingHelpersTests/TestEventInjectorTests.swift` — EXISTS (verified)
- [x] `docs/testing.md` updated with "Event script schema" — EXISTS (verified via grep)
- [x] `ios/AINMMessenger/AINMMessenger/App/AINMMessengerApp.swift` — TODO(14-04) replaced with live call (verified)
- [x] Commit `e6081f0` (Task 1) — FOUND in `git log --oneline`
- [x] Commit `be7ca4d` (Task 2) — FOUND in `git log --oneline`
- [x] Commit `0aa214f` (Task 3) — FOUND in `git log --oneline`
