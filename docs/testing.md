# Testing

## UAT Automation Harness (Phase 14)

Automated XCUITest coverage for the six Phase 13 acceptance scenarios. Runs hermetically — no human taps required.

### Local invocation

```bash
make uat
```

This target:

1. Checks that port 8080 is free (fails fast if in use — common collision on dev machines running other services).
2. Starts `cargo run --bin ainm-demo -- --uat` in the background; captures PID to `tmp/uat-server.pid`.
3. Polls `http://localhost:8080/health` up to 30s for readiness.
4. Runs `xcodebuild test -scheme AINMMessengerUITests -destination 'platform=iOS Simulator,name=iPhone 16,OS=18.0'` against the iPhone 16 / iOS 18 simulator.
5. Tears down the server on exit, Ctrl-C, or SIGTERM via `trap EXIT INT TERM`.
6. Emits results to `tmp/UAT-Results.xcresult` (open in Xcode with `open tmp/UAT-Results.xcresult`).

### Ancillary targets

- `make uat-debug` — boots the UAT server and holds it until Ctrl-C (for manual inspection via `curl http://localhost:8080/health` or connecting the iOS app in Xcode without the test runner).
- `make uat-clean` — removes `tmp/` (server logs, PID files, result bundles).

### Server behavior under `--uat`

| Knob          | Value                                                          |
| ------------- | -------------------------------------------------------------- |
| Port          | 8080 (fixed; fails fast if occupied)                           |
| Storage       | Ephemeral tempdir; wiped on process exit                       |
| Log level     | INFO                                                           |
| Log path      | `tmp/uat-server.log` (repo-root relative)                      |
| Identities    | `uat-user@localhost` (human), `helper@localhost` (agent) — deterministic; match the iOS fixture identities |

### Event script schema

UAT scenarios 1 (tool call lifecycle) and 4 (memory context frame) rely on the `TestEventInjector` to push synthetic events into the app at millisecond offsets. Activated via a second launch argument pair: `-UITestEvents <script-name>` (no `.json` suffix). Scripts live under `ios/AINMMessenger/AINMMessenger/Testing/EventScripts/` and ship in the app bundle (`Bundle.main.url(forResource:withExtension:)` loader path).

Scenarios 2 (mentions), 3 (agent discovery), 5 (sub-threads), and 6 (deep links) do NOT use the injector — they rely on seeded state + real XCUITest gestures only.

**Top-level shape:**

```json
{
  "version": "1.0",
  "scenario": "scenario-NN-name",
  "events": [ /* ScriptEntry array, ordered is informational — actual firing order is atMs */ ]
}
```

**ScriptEntry fields:**

| Field                              | Required                  | Notes                                                             |
| ---------------------------------- | ------------------------- | ----------------------------------------------------------------- |
| `atMs`                             | yes                       | Offset in milliseconds from injector schedule start (app launch). |
| `type`                             | yes                       | One of the event types below.                                     |
| `threadId`                         | yes                       | Target thread — must match seed fixture's thread id.              |
| `callId`                           | tool.call.* only          | Must match a fixture-seeded tool-call id when applicable.         |
| `toolName` / `toolDescription`     | tool.call.started optional | Populates `ToolCallState.name`.                                   |
| `input`                            | tool.call.* optional      | Arbitrary JSON object; typed as `[String: AnyCodableValue]`.      |
| `progress`                         | tool.call.progress        | `{current, total, detail}` triple.                                |
| `status`                           | tool.call.completed/error | `"success"` or `"error"`.                                         |
| `resultSummary` / `durationMs`     | tool.call.completed       | Populates `ToolCallState.resultSummary` / `.durationMs`.          |
| `errorMessage`                     | tool.call.error optional  | Human-readable error text.                                        |
| `frame`                            | context.frame             | See context.frame payload below.                                  |

**Supported event types:**

- `tool.call.started` — emits a `ToolCallEvent` with no status; `handleToolEvent` creates a new `ToolCallState(.running)`.
- `tool.call.progress` — emits a `ToolCallEvent` with a `progress` block; `handleToolEvent` updates the existing state's `progress` property.
- `tool.call.completed` — emits a `ToolCallEvent` with `status = "success"`; `handleToolEvent` transitions state to `.completed` with `resultSummary` and `durationMs`.
- `tool.call.error` — emits a `ToolCallEvent` with `status = "error"`; `handleToolEvent` transitions state to `.error`.
- `context.frame` — builds a full `ContextFrame` (threadId + nested Frame) and assigns into `appState.contextFrames[threadId]`.

**context.frame payload shape** (translated to the real `ContextFrame.Frame` struct at dispatch):

```json
{
  "summary": "Conversation summary to display in memory sheet",
  "activeMessages": ["msg-01"],
  "archivalFactIds": ["fact-coffee", "fact-meetings"],
  "participantIdentities": ["uat-user@localhost", "helper@localhost"]
}
```

The injector expands `activeMessages` into `ContextFrame.MessageSummary` stubs, `archivalFactIds` into `ContextFrame.ArchivalFact` stubs keyed by the given ids, and classifies `participantIdentities` as agent vs human based on identity prefix.

**Timing model:** `atMs` is relative to `TestEventInjector.schedule()`, which runs inside the `.task` closure after `appState.setup()` and the seed load. Jitter on iOS 18/26 simulator is typically ~1ms; XCUITest assertions should use `waitForExistence(timeout: 5.0)` or generous budgets to absorb cold-start simulator variance.

**Cross-fixture contract:** `threadId` and `callId` must match the rows a 14-03 scenario fixture seeds into GRDB. Fact ids referenced by `context.frame.archivalFactIds` must exist in `scenario-NN-memory.json`'s seeded archival facts. Mismatches surface as empty memory sheets or missing indicator rows.

### CI integration

`.github/workflows/ios-uat.yml` is intentionally `workflow_dispatch`-only (manual trigger from the Actions tab). Flipping to push/PR triggers is tracked as a deferred item (see `.planning/phases/14-uat-automation-harness/14-CONTEXT.md` "Deferred Ideas").

### Known dependencies

- Xcode 16+ with iOS 18 SDK (matches `.github/workflows/ios-ci.yml`)
- Rust 1.85+ (project minimum)
- `curl` and GNU-compatible Make (macOS built-ins work)
- Port 8080 free on the host
