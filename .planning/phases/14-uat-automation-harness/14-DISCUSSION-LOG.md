# Phase 14: UAT Automation Harness - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-18
**Phase:** 14-uat-automation-harness
**Areas discussed:** Seed bootstrap architecture, Server lifecycle for test runs, Accessibility identifier strategy, Dynamic event provisioning

---

## Gray Area Selection

**Question:** Which gray areas do you want to discuss for Phase 14?

| Option | Description | Selected |
|--------|-------------|----------|
| Seed bootstrap architecture | How `-UITestSeed <fixture>` provisions backend state — real AINM protocol calls, GRDB-direct fixtures, or hybrid. | ✓ |
| Server lifecycle for test runs | How the local Rust AINM server gets started/stopped around `xcodebuild test`. | ✓ |
| Accessibility identifier strategy | Retrofit approach given no `.accessibilityIdentifier` exists in Phase 11–13 views. | ✓ |
| Dynamic event provisioning | How streaming deltas + mid-lifecycle tool calls are triggered at runtime. | ✓ |

**User's choice:** All four areas
**Notes:** Phase 14 success criteria pre-answered several otherwise-gray items (target name, test count, invocation command, CI deferral).

---

## Seed bootstrap architecture

### Seeding approach

| Option | Description | Selected |
|--------|-------------|----------|
| GRDB-direct seed loader | In-app SeedLoader resets DB and inserts fixture records directly. Fast, hermetic, deterministic. | ✓ |
| Real-protocol provisioning | Seed drives real AINM protocol calls to the server. Highest realism, slowest, needs server-side fixture agent. | |
| Hybrid: GRDB + server | Static state via GRDB, live events via server fixture agent. Middle path with dual-seeding complexity. | |

### Fixture format

| Option | Description | Selected |
|--------|-------------|----------|
| JSON files bundled in test target | `Fixtures/scenario-NN-*.json` decoded by SeedLoader. Diffable, version-controlled. | ✓ |
| Swift fixture builders | Code-based fixture functions. Type-safe, harder to eyeball. | |
| Hybrid: Swift builders + JSON parameters | Builder functions with per-scenario JSON parameters. | |

### Seed loader location

| Option | Description | Selected |
|--------|-------------|----------|
| Behind `#if DEBUG` in app target | Only compiled in Debug builds, invoked before `AppState.setup()`. | ✓ |
| Always compiled, gated by launch arg | Always present, inert without the flag. | |
| Separate UITestHost target | Dedicated UI test host app. | |

### Identity reset policy

| Option | Description | Selected |
|--------|-------------|----------|
| Reset identity + regenerate keypair | Nuke Keychain, provision deterministic `uat-user@localhost` with fixed-seed Ed25519. | ✓ |
| Reuse existing Keychain identity | Skip identity setup; assume developer has onboarded. | |
| Ephemeral in-memory identity | Keypair held only for app lifetime, never written to Keychain. | |

**User's choice:** All four recommended options
**Notes:** GRDB-direct + JSON + `#if DEBUG` + deterministic Keychain is the hermetic/fast path. Reset Keychain enables signature verification assertions and eliminates cross-run contamination.

---

## Server lifecycle for test runs

### Orchestration

| Option | Description | Selected |
|--------|-------------|----------|
| Makefile target orchestrates | `make uat` boots server, waits, runs tests, tears down. | ✓ |
| Xcode scheme pre-action | Test scheme's pre-action launches server. | |
| Test-bundle SPM wrapper | Swift script spawns server in `setUp`, kills in `tearDown`. | |

### Readiness check

| Option | Description | Selected |
|--------|-------------|----------|
| Poll `GET /health` until 200 OK, 30s timeout | Relies on existing health endpoint. Deterministic. | ✓ |
| Fixed `sleep 3` | Simple but flaky. | |
| Poll WebSocket `ainm.initialize` round-trip | Full protocol handshake. Needs dedicated helper binary. | |

### Server config

| Option | Description | Selected |
|--------|-------------|----------|
| Ephemeral in-memory/temp DB, fixed port 8080 | Clean slate each run, port collisions surface. | ✓ |
| Reuse developer's dev config | Non-hermetic but simple. | |
| Unique random port + ephemeral DB | No collision risk but env-var plumbing. | |

### CI invocation

| Option | Description | Selected |
|--------|-------------|----------|
| Makefile + README + placeholder workflow | `make uat`, docs, disabled `ios-uat.yml` for future. | ✓ |
| Makefile + README only | Pure documentation, no CI file. | |
| Enable GitHub Actions matrix job now | Full CI wiring in this phase. | |

**User's choice:** All four recommended options
**Notes:** The `/health` endpoint already exists in `demo/src/server.rs`. Placeholder workflow file preserves concrete starting point for future matrix activation without triggering CI cost now.

---

## Accessibility identifier strategy

### Retrofit scope

| Option | Description | Selected |
|--------|-------------|----------|
| Targeted: only elements the 6 tests query | Minimum set (~20–40 touches across ~10 files). | ✓ |
| Namespace audit: all Phase 13 views | All interactive controls across ToolVisibility/Memory/SubThreading/AgentDiscovery/Mentions. | |
| Full-app audit | Every tappable element across Phases 11–13. | |

### Naming convention

| Option | Description | Selected |
|--------|-------------|----------|
| `screen.component.action` dot-path | Hierarchical, greppable. | ✓ |
| Screaming snake case constants | Verbose, un-Swifty. | |
| Free-form strings per test | Fast now, drifts to inconsistency. | |

### Identifier source of truth

| Option | Description | Selected |
|--------|-------------|----------|
| Shared `UITestIdentifiers.swift` linked to both targets | Single source, compile-time typo protection. | ✓ |
| String literals at each call site | Zero ceremony, silent typo failures. | |
| Test-side constants only | Centralized on test side, literal on app side. | |

### State indicator assertion

| Option | Description | Selected |
|--------|-------------|----------|
| Distinct identifier per state | `.running`, `.completed`, `.error`, `.timedOut`; assert existence. | ✓ |
| Single identifier + `accessibilityValue` | One element, value carries state. | |
| Snapshot assertions (image diff) | Visual regression; flaky. | |

**User's choice:** All four recommended options
**Notes:** Targeted scope keeps the phase focused on UAT coverage rather than turning into a full a11y audit. Per-state identifiers simplify existence-based assertions.

---

## Dynamic event provisioning

### Event source

| Option | Description | Selected |
|--------|-------------|----------|
| In-app test-only event injector | `TestEventInjector` behind `#if DEBUG`, activated by launch flag; injects into AINMClient callback pipeline. | ✓ |
| Scripted fixture agent on Rust server | Server-side test agent replays events over WebSocket. | |
| Driver helper hooks on AppState | HTTP control endpoint for direct state pokes. | |

### Event timing format

| Option | Description | Selected |
|--------|-------------|----------|
| Explicit millisecond offsets in JSON | `atMs` per event; tests use generous `waitForExistence`. | ✓ |
| Step-advanced (test pokes "next") | Fully test-driven, no clock. | |
| Single fire-and-forget burst | All events fire immediately. | |

### @-mention autocomplete

| Option | Description | Selected |
|--------|-------------|----------|
| Pure XCUITest: type `@` then letters | Real keystroke path, seeded participant list. | ✓ |
| Inject autocomplete dataset via launch arg | Override data source with fixed list. | |
| Bypass composer, set text programmatically | Skips the trigger that's being tested. | |

### Deep link delivery

| Option | Description | Selected |
|--------|-------------|----------|
| `xcrun simctl openurl booted <url>` from test `setUp` | Exercises real `onOpenURL` handler. | ✓ |
| Launch app with URL in launch arguments | Different code path from production URL routing. | |
| Safari/Messages bounce-in | Most realistic, wildly flaky. | |

**User's choice:** All four recommended options
**Notes:** In-app injector exercises the full UI rendering path (notification → ViewModel → SwiftUI) without server-side scripting. `@-mention` stays pure-UI because that keystroke trigger IS the behavior being tested. `simctl openurl` preserves production URL routing code path.

---

## Closing check

**Question:** Which gray areas remain unclear?

| Option | Description | Selected |
|--------|-------------|----------|
| I'm ready for context | Write CONTEXT.md and advance. | ✓ |
| Explore more gray areas | Test isolation granularity, fixture data shape, streaming cursor wait strategy, HUMAN-UAT update mechanism. | |

**User's choice:** Ready for context.
**Notes:** Remaining items (test isolation granularity, xcodebuild destination strings, fixture JSON schema, exact timeout values) move into Claude's Discretion under the locked architecture.

---

## Claude's Discretion

- Exact XCUITest timeout values (base and per-scenario streaming extensions)
- Fixture JSON schema shape — field names, flat vs nested layout
- `TestEventInjector` implementation details — scheduling primitive, backing queue
- `make uat` bash style — trap handlers, log handling, simulator hygiene
- Simulator destination strings (iPhone 16 / Pro, iOS version) — align with existing `ios-ci.yml`
- Optional `make uat-debug` target that keeps server running after tests
- `13-HUMAN-UAT.md` auto-update mechanism when suite goes green

## Deferred Ideas

- Broader accessibility audit beyond tests' needs
- Snapshot/image-diff testing
- CI matrix activation on push/PR (the "wired into matrix later" step)
- UAT automation for Phases 10–12 scenarios
- Real-protocol end-to-end fidelity tests with server-side fixture agent
- `AppState` HTTP control endpoint for direct state pokes
- Step-advanced event injection pulling mode
