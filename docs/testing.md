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
