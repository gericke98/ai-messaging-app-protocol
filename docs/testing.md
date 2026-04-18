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

### CI integration

`.github/workflows/ios-uat.yml` is intentionally `workflow_dispatch`-only (manual trigger from the Actions tab). Flipping to push/PR triggers is tracked as a deferred item (see `.planning/phases/14-uat-automation-harness/14-CONTEXT.md` "Deferred Ideas").

### Known dependencies

- Xcode 16+ with iOS 18 SDK (matches `.github/workflows/ios-ci.yml`)
- Rust 1.85+ (project minimum)
- `curl` and GNU-compatible Make (macOS built-ins work)
- Port 8080 free on the host

### Fixture JSON schema

UAT scenarios seed their starting state via JSON fixtures bundled with the iOS app under
`ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-NN-*.json`. When the app
launches with `-UITestSeed <fixture-name>`, `SeedLoader` looks up `<fixture-name>.json`
in `Bundle.main`, decodes it into a `ScenarioFixture`, and inserts every row in a
single GRDB transaction before `AppState` observations fire. One JSON field per SQLite
column — fixture authors do not need to understand any intermediate model.

**Top-level shape** (see `ScenarioFixture.swift` for the canonical Swift types):

```json
{
  "version": "1.0",
  "scenario": "scenario-01-tool-call",
  "identity": "uat-user@localhost",
  "threads":        [ /* FixtureThread        */ ],
  "participants":   [ /* FixtureParticipant   */ ],
  "messages":       [ /* FixtureMessage       */ ],
  "toolCalls":      [ /* FixtureToolCall      */ ],
  "archivalFacts":  [ /* FixtureArchivalFact  */ ],
  "agentCards":     [ /* FixtureAgentCard     */ ]
}
```

| Array             | Maps onto GRDB record | Column-by-column |
| ----------------- | --------------------- | ---------------- |
| `threads`         | `ThreadRecord`        | `id`, `title?`, `createdAt`, `createdBy`, `parentThreadId?`, `parentMessageId?`, `hasUnread` |
| `participants`    | `ParticipantRecord`   | `threadId`, `identity`, `type`, `joinedAt`, `role` (+ optional `displayName` fixture-only for @-mention seeding) |
| `messages`        | `MessageRecord`       | `id`, `threadId`, `fromAddress`, `timestamp`, `type`, `contentJson`, `signature`, `serverTimestamp?` |
| `toolCalls`       | `ToolCallRecord`      | `callId`, `threadId`, `name`, `status`, `input?`, `resultSummary?`, `durationMs?`, `errorMessage?`, `startedAt`, `completedAt?` |
| `archivalFacts`   | `MemoryFactRecord`    | `factId`, `key`, `value`, `tags?`, `storedBy`, `storedAt` |
| `agentCards`      | `AgentCardRecord`     | `url`, `identity`, `cardJson`, `etag?`, `fetchedAt` |

**Conventions across every fixture:**

- `version` must equal `"1.0"` — `SeedLoader` rejects fixtures with mismatched top-level version in future revisions.
- `identity` must equal `"uat-user@localhost"` so the deterministic Keychain keypair installed by `UITestIdentityBootstrap.resetIdentity()` signs messages that match seeded participant rows.
- All timestamps are RFC 3339 with millisecond precision in UTC (`Z` suffix), e.g. `"2026-04-18T10:00:00.000Z"`.
- `signature` on `messages` is a test placeholder (`AAAA`, `BBBB`, …) — the UI does NOT verify signatures on locally-seeded rows. Real server-delivered messages go through the full verify path.
- `tags` on `archivalFacts` and `cardJson` on `agentCards` are JSON-encoded strings (one layer of escaping) so they round-trip through the GRDB text columns exactly as the Phase 13 code writes them.

**Canonical scenario fixtures and what they seed:**

| Fixture                            | Scenario (13-HUMAN-UAT.md)   | Seeds                                                                               | Paired 14-04 event script       |
| ---------------------------------- | ---------------------------- | ----------------------------------------------------------------------------------- | ------------------------------- |
| `scenario-01-tool-call.json`       | #1 Tool-call visibility      | `thread-01` + `uat-user` + `helper@localhost` + one user message                    | `scenario-01-events.json` (call-01 lifecycle) |
| `scenario-02-mentions.json`        | #2 @-mention autocomplete    | `thread-01` + `uat-user` + `helper` + `remy@localhost` (for "@Re" filter)           | —                               |
| `scenario-03-agent-discovery.json` | #3 Agent discovery UI        | `helper@localhost` + `assistant@localhost` AgentCards (no thread)                   | —                               |
| `scenario-04-memory.json`          | #4 Memory context sheet      | `thread-01` + `uat-user` + `helper` + 4 archival facts (`coffee`, `meetings`, `project`, `timezone`) | `scenario-04-events.json` (context frame push) |
| `scenario-05-sub-thread.json`      | #5 Sub-thread branch action  | `thread-parent-01` + `uat-user` + `helper` + one parent message (`msg-parent-01`)   | —                               |
| `scenario-06-deep-link.json`       | #6 `ainm://` deep link       | `thread-deeplink-01` + `uat-user` + `helper` + one message (`msg-deeplink-01`)      | —                               |

See `.planning/phases/14-uat-automation-harness/14-03-SUMMARY.md` for the authoring log and `14-04-PLAN.md` for the paired runtime event scripts.
