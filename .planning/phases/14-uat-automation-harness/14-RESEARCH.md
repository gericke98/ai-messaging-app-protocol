# Phase 14: UAT Automation Harness — Research

**Researched:** 2026-04-18
**Domain:** XCUITest / Xcode 16 synchronized groups / GRDB-direct seed bootstrap / Swift 6 strict concurrency / Rust server `--uat` orchestration
**Confidence:** HIGH (most findings verified against actual project code + official docs). Two LOW-confidence items flagged for Step 1 ruby-gem probe.

## Summary

Phase 14 wires six XCUITest cases (one per Phase 13 scenario) against a locally-booted Rust AINM server, using a hermetic `-UITestSeed <fixture>` GRDB-direct bootstrap and `-UITestEvents <script>` runtime event injector. All architectural decisions are locked in `14-CONTEXT.md`; this research answers the HOW inside those constraints.

Three facts discovered while reading the codebase shape the plan decisively:

1. **The Xcode project uses `PBXFileSystemSynchronizedRootGroup` (Xcode 16 folder-sync)** — file target membership is determined by directory, not per-file references. Adding a UI test target means adding a new synchronized root group + target entry, not a classic pbxproj file reference edit. This makes the third-party `xcodeproj` ruby gem (v1.27+) the only safe programmatic option; otherwise use Xcode UI.
2. **The binary is `ainm-demo`, not `ainm-server`** (`demo/Cargo.toml` line 11). The `--uat` flag and Makefile must target this name.
3. **`/health/live` exists on `start_federation` only, bound on port `cli.port` (default 8080)** — but only when run in federation mode (`--mode human|agent`). The legacy mode (no `--mode` flag, default) goes through `run_legacy_demo` → `server::start` which binds `127.0.0.1:0` (random port!) and has **no health endpoint**. The `--uat` flag must explicitly drive federation mode OR add a simpler UAT-specific entry path.

**Primary recommendation:** Structure the phase as **six sequential plans in three waves**: (Wave 1, parallel) Xcode target scaffold + Rust `--uat` flag, (Wave 2, parallel) SeedLoader + TestEventInjector + a11y identifier retrofit, (Wave 3, serial) six XCUITest cases against the assembled harness, then the green-run checkpoint that flips `13-HUMAN-UAT.md` to `complete`.

<user_constraints>
## User Constraints (from 14-CONTEXT.md)

### Locked Decisions

**Seed bootstrap architecture**
- **D-01:** GRDB-direct seed loader. Launch arg `-UITestSeed <fixture>` triggers an in-app `SeedLoader` that resets `AppDatabase` and inserts fixture records (threads, participants, messages, tool calls, memory frames, sub-thread parent messages) directly into GRDB. `AINMClient` still connects for live events, but historical state never round-trips through the server.
- **D-02:** Fixtures are JSON files bundled in the test target under `Fixtures/scenario-NN-<name>.json`, decoded by `SeedLoader` into GRDB records. Version-controlled, diffable. Launch arg value names the fixture (e.g. `-UITestSeed scenario-01-tool-call`).
- **D-03:** `SeedLoader` lives in the app target gated by `#if DEBUG`. `AINMMessengerApp` checks `ProcessInfo.processInfo.arguments` for `-UITestSeed` at launch and invokes the loader before the normal `AppState.setup()` path. Release builds do not include this code.
- **D-04:** Seed loader resets Keychain-stored identity and provisions a deterministic `uat-user@localhost` with a fixed-seed Ed25519 keypair on every run. Guarantees cross-test isolation and makes signature assertions reproducible.

**Server lifecycle**
- **D-05:** Top-level `Makefile` with a `uat` target orchestrates the full run: starts `cargo run --bin ainm-server --uat` in the background, waits for readiness, runs `xcodebuild test -scheme AINMMessengerUITests`, tears the server down on exit (including trap handlers for Ctrl-C). This is the documented local and CI invocation.
- **D-06:** Readiness check: Makefile polls `GET http://localhost:8080/health` until 200 OK with a 30-second timeout. The demo server already exposes `/health` (`demo/src/server.rs`); if any gaps surface during research they're resolved as part of this phase.
- **D-07:** Server runs with a `--uat` flag that forces an ephemeral SQLite database in a temp directory, logs at INFO to a known file (`tmp/uat-server.log`), and binds fixed port `8080`. Every run starts from a clean slate; port collisions surface immediately rather than taint results.
- **D-08:** CI surface: `Makefile` + docs entry + a placeholder `.github/workflows/ios-uat.yml` that references the Makefile but is disabled (workflow_dispatch only, not triggered on push/PR).

**Accessibility identifier strategy**
- **D-09:** Targeted retrofit only. Walk each HUMAN-UAT scenario, identify the minimum set of views/controls tests must query, add `accessibilityIdentifier` just to those — expected footprint ~20–40 touches across ~10 files.
- **D-10:** Naming convention: `screen.component.action` dot-path (e.g. `thread.toolbar.memoryPill`).
- **D-11:** Identifiers declared in a single `UITestIdentifiers.swift` file, membership in both app target (`#if DEBUG`-gated if needed) and UI test target so Swift code on both sides imports the same constants.
- **D-12:** Stateful indicators get a distinct identifier per state (e.g. `toolCall.indicator.status.completed`); tests assert existence rather than reading values.

**Dynamic event provisioning**
- **D-13:** In-app `TestEventInjector` gated by `#if DEBUG`, activated by `-UITestEvents <script>.json`. Schedules synthetic `StreamEvent`, tool-call lifecycle notifications, presence updates, and memory push notifications into the existing `AINMClient` callback pipeline.
- **D-14:** Event scripts express timing as explicit millisecond offsets in JSON (`atMs`). Tests use generous `waitForExistence` timeouts.
- **D-15:** `@-mention` autocomplete driven by real XCUITest typing against a pre-seeded participant list.
- **D-16:** Deep-link scenario (#6) delivered via `xcrun simctl openurl booted ainm://thread/{id}` invoked from the test's `setUp` (**see Pitfall #3** — Process() doesn't work from iOS test runner; must be invoked from Makefile/external script before the test starts).

### Claude's Discretion

- Exact XCUITest timeout values (base `waitForExistence`, per-scenario extensions for streaming)
- Fixture JSON schema shape — concrete field names, nested vs flat record layout
- `TestEventInjector` implementation details — backing queue, scheduling primitive (Timer vs DispatchWorkItem vs Task.sleep)
- `make uat` bash style — which `trap` handlers, log file tee, simulator boot/shutdown hygiene
- Simulator destination strings (iPhone 16 vs 16 Pro, which iOS version) — align with existing `ios-ci.yml`
- Whether to include a `make uat-debug` target that keeps the server running after tests for inspection
- How to update `13-HUMAN-UAT.md` when tests go green

### Deferred Ideas (OUT OF SCOPE)

- Broader accessibility audit — full-app `accessibilityIdentifier`/`accessibilityLabel` coverage
- Snapshot/image-diff testing — visual regression detection
- CI matrix activation on push/PR — flipping `ios-uat.yml` from `workflow_dispatch`-only to triggered
- UAT automation for Phases 10–12 scenarios
- Real-protocol end-to-end fidelity tests with scripted fixture agent
- Driver helper hooks on `AppState` (HTTP control endpoint)
- Step-advanced event injection (pulling mode for the event injector)
</user_constraints>

<phase_requirements>
## Phase Requirements

No REQ-IDs map to Phase 14 — it is a process/testing phase that automates verification of requirements AI-01..AI-05, PLT-04, PLT-05 already completed in Phase 13. The deliverables are tied to ROADMAP success criteria (SC-1..SC-6) rather than numbered REQs.

| SC | Success Criterion | Research Support |
|----|-------------------|------------------|
| SC-1 | `AINMMessengerUITests` target exists, runnable via `xcodebuild test -scheme AINMMessengerUITests` | §Xcode project modification |
| SC-2 | `-UITestSeed <fixture>` launch arg provisions backend state per scenario | §Seed bootstrap |
| SC-3 | Six XCUITest cases (1:1 with `13-HUMAN-UAT.md`) assert user-observable behavior | §XCUITest suite architecture + §A11y identifiers |
| SC-4 | All 6 tests green on booted iOS 18+ simulator with AINM server running | §Rust server orchestration + §Makefile |
| SC-5 | `13-HUMAN-UAT.md` updates to `complete` with 6/6 passed | §HUMAN-UAT status update |
| SC-6 | CI-runnable command documented (Makefile + placeholder workflow) | §Makefile + §GitHub Actions |

The only REQs the harness transitively touches are those whose truths are visually verified only — `13-VERIFICATION.md` lists six items marked `? VERIFIED*` awaiting human observation; this harness discharges the human-observation requirement for those items automatically.
</phase_requirements>

## Project Constraints (from CLAUDE.md)

Directives the planner must honor regardless of preferences:

- **Swift 6 strict concurrency** (`SWIFT_STRICT_CONCURRENCY = complete` verified in `project.pbxproj` B1000001/B1000002). All new Swift code — `SeedLoader`, `TestEventInjector`, `UITestIdentifiers`, test cases — must be `Sendable`-clean. @MainActor on UI-driving state, actors for isolated mutable state.
- **@Observable (iOS 17+) pattern**, not `ObservableObject` / Combine.
- **MVVM + Repository architecture**, protocol-based DI for testability.
- **GRDB.swift 7.10+**, DatabasePool for concurrent reads / serialized writes. All DB mutations go through migrations and `dbPool.write`.
- **CryptoKit Curve25519.Signing for Ed25519** (not third-party crypto).
- **Keychain via KeychainAccess wrapper or direct Security framework** — precedent established in `KeyManager.swift`.
- **No new third-party Swift packages in scope for this phase** — UI-SPEC Phase 13 already states "Third-party Swift packages in scope for this phase: **none new.**" and the locked decisions reuse existing infrastructure.
- **Rust 1.85+ / Axum 0.8.x / clap 4.6** — the `--uat` flag slots into existing `clap::Parser` struct in `demo/src/main.rs`.
- **GSD workflow:** plans must map concrete tasks onto files, use atomic commits, and include verify steps.

## Standard Stack

### Core (already in project, zero new deps)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| XCTest / XCUITest | Xcode 16.2 (iOS 18 SDK) | Native Apple test framework | Only option for hermetic iOS UI testing; required by SC-1 `xcodebuild test` |
| SwiftUI | iOS 18 | Views under test | Already the app's UI framework |
| GRDB.swift | 7.10.0+ (`upToNextMajorVersion`) | DB seed inserts | Already the project's persistence layer; `DatabasePool` + migrations ready to reuse |
| CryptoKit | iOS 18 built-in | Fixed-seed Ed25519 keypair | `Curve25519.Signing.PrivateKey(rawRepresentation: Data)` accepts a 32-byte deterministic seed |
| Keychain Security framework | iOS 18 built-in | Identity reset | `KeyManager.deleteKeypair()` already exists — reuse it |
| Foundation `ProcessInfo` | iOS 18 built-in | Read launch arguments | Standard pattern: `ProcessInfo.processInfo.arguments.contains("-UITestSeed")` |
| clap | 4.6.x (Cargo.toml already pins) | Rust CLI parsing | Already used; add `--uat` as a new `#[arg(long)]` field |
| axum | 0.8 (pinned) | `/health` endpoint | Already has `/health/live` on federation path — reuse + possibly add wrapper `/health` |

### Supporting (zero new deps — infrastructure only)

| Tool | Version | Purpose | When to Use |
|------|---------|---------|-------------|
| GNU Make | 3.81+ (macOS built-in) | Orchestrator per D-05 | `make uat` target chains server boot, health poll, test run, teardown |
| `xcrun simctl` | Xcode 16 built-in | Boot simulator, `openurl` for deep links | Invoked from Makefile (NOT from inside the test — see Pitfall #3) |
| `curl` | macOS built-in | Health poll per D-06 | `curl -sf http://localhost:8080/health` in retry loop |
| `xcodebuild` | Xcode 16 built-in | Test runner per SC-1 | `xcodebuild test -scheme AINMMessengerUITests -destination '...'` |

### Alternatives Considered

| Instead of | Could Use | Tradeoff | Why rejected |
|------------|-----------|----------|--------------|
| XCUITest | Appium / Maestro / WebDriverAgent | Cross-platform | Extra dep; locked by SC-1 "xcodebuild test" wording |
| Hand-edited pbxproj for new target | `xcodeproj` ruby gem / XcodeGen / tuist | Programmatic & repeatable | See §Xcode project modification — **Xcode UI is the recommended path** for a Xcode-16 synchronized-group project; xcodeproj v1.27+ supports PBXFileSystemSynchronizedRootGroup but UI test target creation through the gem is poorly documented. Tuist / XcodeGen would require retrofitting the whole project to a manifest — far out of scope |
| `XCUIApplication.open(url:)` | `xcrun simctl openurl` from Makefile | Pure-Swift test | Apple API has documented issues on iOS Simulator through iOS 18 — custom URL schemes frequently launch the app without delivering the URL. Shelling out from Makefile pre-test is more reliable (see §XCUITest suite, scenario #6) |
| `Process()` from test setUp | Makefile orchestration | Single-file test | **Does not work** — iOS XCUITest runner is sandboxed; `Foundation.Process` is unavailable there (Apple Forums thread 657144) |
| GRDB-direct seed | Fixture agent over real WebSocket | True end-to-end | Locked D-01; GRDB direct is hermetic + fast + deterministic |

## Architecture Patterns

### Recommended Project Structure (additive)

```
Makefile                                                  # NEW — top-level orchestrator (D-05)
.github/workflows/ios-uat.yml                             # NEW — placeholder workflow_dispatch (D-08)

demo/src/main.rs                                          # MODIFY — add --uat flag to clap Cli struct
demo/src/server.rs                                        # MODIFY — ensure /health (plain) exists under --uat path
demo/src/db.rs                                            # MODIFY — tempdir path branch when --uat set

ios/AINMMessenger/
├── AINMMessenger.xcodeproj/
│   ├── project.pbxproj                                   # MODIFY — add AINMMessengerUITests target
│   └── xcshareddata/xcschemes/
│       ├── AINMMessenger.xcscheme                        # (unchanged)
│       └── AINMMessengerUITests.xcscheme                 # NEW — shared scheme for xcodebuild CLI
├── AINMMessenger/                                        # (existing app target dir — PBXFileSystemSynchronizedRootGroup)
│   ├── App/
│   │   ├── AINMMessengerApp.swift                        # MODIFY — launch-arg detection before appState.setup()
│   │   └── UITestIdentifiers.swift                       # NEW — shared constants, linked to both targets (D-11)
│   ├── Testing/                                          # NEW dir — #if DEBUG gated
│   │   ├── SeedLoader.swift                              # NEW — GRDB-direct fixture loader (D-01..D-03)
│   │   ├── TestEventInjector.swift                       # NEW — runtime event injector (D-13..D-14)
│   │   └── UITestIdentityBootstrap.swift                 # NEW — Keychain reset + fixed-seed keypair (D-04)
│   └── UI/ToolVisibility/ToolCallIndicatorView.swift     # MODIFY — add .accessibilityIdentifier
│   └── UI/Composer/ComposerView.swift                    # MODIFY — add composer + autocomplete identifiers
│   └── UI/Mentions/MentionAutocompleteView.swift         # MODIFY — add list + row identifiers
│   └── UI/MessageThread/MessageThreadView.swift          # MODIFY — add toolbar pill, message row identifiers
│   └── UI/MessageThread/MessageBubbleView.swift          # MODIFY — add branch action identifier
│   └── UI/Memory/MemoryContextView.swift                 # MODIFY — add section + preview identifiers
│   └── UI/Memory/ArchivalMemoryView.swift                # MODIFY — add search + row + delete identifiers
│   └── UI/AgentDiscovery/AgentDiscoveryView.swift        # MODIFY — add list cell + search identifiers
│   └── UI/AgentDiscovery/AgentCardDetailView.swift       # MODIFY — add startConversation button identifier
│   └── UI/SubThreading/SubThreadBannerView.swift         # MODIFY — add back button identifier
│   └── UI/SubThreading/SubThreadIndicatorView.swift      # MODIFY — add badge identifier
└── AINMMessengerUITests/                                 # NEW dir — PBXFileSystemSynchronizedRootGroup
    ├── Fixtures/                                         # NEW — scenario-NN-*.json (bundled resources)
    │   ├── scenario-01-tool-call.json
    │   ├── scenario-02-mentions.json
    │   ├── scenario-03-agent-discovery.json
    │   ├── scenario-04-memory.json
    │   ├── scenario-05-sub-thread.json
    │   └── scenario-06-deep-link.json
    ├── EventScripts/                                     # NEW — scenario-NN-events.json
    │   ├── scenario-01-events.json
    │   └── scenario-04-events.json
    ├── AINMUITestCase.swift                              # NEW — base class (launch, readiness wait, teardown)
    ├── ToolCallIndicatorUITests.swift                    # NEW — scenario 1
    ├── MentionAutocompleteUITests.swift                  # NEW — scenario 2
    ├── AgentDiscoveryUITests.swift                       # NEW — scenario 3
    ├── MemoryContextUITests.swift                        # NEW — scenario 4
    ├── SubThreadBranchUITests.swift                      # NEW — scenario 5
    └── DeepLinkUITests.swift                             # NEW — scenario 6

docs/testing.md                                           # NEW — `make uat` docs per D-08
```

### Pattern 1: `#if DEBUG`-gated testability layer

**What:** All UAT hooks (`SeedLoader`, `TestEventInjector`, `UITestIdentityBootstrap`, identifiers used only for tests) compile only in Debug builds. Release builds are identical to pre-Phase-14.

**When to use:** Any code that exists purely to enable UI tests against private internals.

**Example:**
```swift
// Source: project convention, see .planning/phases/11-04-PLAN.md precedent
#if DEBUG
import Foundation
import GRDB

enum SeedLoader {
    static func loadIfRequested(into db: AppDatabase) async throws -> Bool {
        let args = ProcessInfo.processInfo.arguments
        guard let flagIdx = args.firstIndex(of: "-UITestSeed"),
              flagIdx + 1 < args.count else { return false }
        let fixtureName = args[flagIdx + 1]
        // ... decode + reset + insert
        return true
    }
}
#endif
```

### Pattern 2: Launch-arg-driven bootstrap, before AppState.setup()

**What:** `AINMMessengerApp` inspects `ProcessInfo.processInfo.arguments` at the earliest possible moment and runs SeedLoader + identity-reset before the normal setup path.

**When to use:** Any test that needs hermetic state. Release builds skip the guard and run as before.

**Example (integration point in `AINMMessengerApp.swift`):**
```swift
// Source: existing AINMMessengerApp.swift lines 20-23 + CONTEXT D-03
@main
struct AINMMessengerApp: App {
    @State private var appState = AppState()
    @State private var showThreadNotFound = false

    var body: some Scene {
        WindowGroup {
            NavigationStack { ... }
            .task {
                #if DEBUG
                // BEFORE appState.setup() — seed identity + DB deterministically.
                try? await UITestBootstrap.runIfRequested(for: appState)
                #endif
                try? appState.setup()
                #if DEBUG
                await TestEventInjector.scheduleIfRequested(for: appState)
                #endif
            }
            .onOpenURL { handleDeepLink($0) }
            .alert("Thread Not Found", ...)
        }
    }
}
```
Note: swap existing `.onAppear { try? appState.setup() }` for `.task { ... }` because seed loading is async. Verify this does not break onboarding order — the boot is a no-op when no launch arg is present.

### Pattern 3: Callback-pipeline event injection (matches Phase 12 D-01)

**What:** `TestEventInjector` emits synthetic events into the same callbacks `AINMClient` uses for real server events. Ensures tests exercise the identical UI rendering path as production.

**When to use:** Every scenario where UI reacts to server pushes (streaming, tool calls, presence, context frames, briefings).

**Example pattern:**
```swift
// Source: existing AppState.swift lines 78-149 — the callback closures the injector must match
#if DEBUG
@MainActor
enum TestEventInjector {
    static func scheduleIfRequested(for appState: AppState) async { /* ... */ }

    // Must call the SAME closures AppState registers with client.setToolEventCallback:
    private static func injectToolEvent(threadId: String, event: ToolCallEvent, onto appState: AppState) {
        // Directly invoke the handler AppState would receive from AINMClient.
        appState.handleToolEvent(threadId: threadId, event: event)
    }
}
#endif
```
The injector does NOT go through `AINMClient` — it calls the @MainActor `handleToolEvent` / `handlePresence` / `contextFrames` mutators directly on `AppState`, which is exactly what the real callbacks do (see `AppState.swift` lines 124-128, 131-135, 257-304).

### Pattern 4: Shared identifiers file membership in both targets (D-11)

**What:** `UITestIdentifiers.swift` is a plain Swift file with a single `enum UITestID { ... }` namespace containing `static let` constants. Physically lives in `ios/AINMMessenger/AINMMessenger/App/UITestIdentifiers.swift` and is a member of BOTH the app target (via the existing `PBXFileSystemSynchronizedRootGroup`, since it sits under `AINMMessenger/App/`) AND the UI test target (via an explicit file reference or the UI test target's synchronized-group exceptions set).

**The catch with synchronized root groups:** a single physical file can be in exactly one synchronized root group (its enclosing directory). To make it compile into the UI test target too, add it to the UI test target's `membershipExceptions` list — the `PBXFileSystemSynchronizedBuildFileExceptionSet` (already present at A8000001/A8000002 in pbxproj) supports adding files from outside the target's root group.

**When to use:** For the one shared-constants file. Anything more cross-target (a whole helper framework) should instead go in a Swift Package inside the repo.

**Swift 6 strict concurrency note:** `enum UITestID` with only `static let` string constants is trivially `Sendable`. No annotations needed.

### Anti-Patterns to Avoid

- **Seeding via the real WebSocket path.** Locked against by D-01. Any task action that tries to "just send fake messages through the server" violates the locked architecture.
- **Using `#if DEBUG` gating for app code that UI tests must see.** Identifiers must be visible when the app is launched under the normal test configuration. Since the project build is `SWIFT_ACTIVE_COMPILATION_CONDITIONS = "DEBUG $(inherited)"` only in the Debug config, and UI tests run Debug by default, `#if DEBUG`-guarding identifier assignments is safe — but NEVER guard the identifier STRING constants themselves (they must be referenceable from the UI test target regardless of config).
- **Using `XCUIApplication.open(_:)` for the deep-link scenario.** See Pitfall #2 — documented unreliability on iOS Simulator through iOS 18.
- **Calling `Foundation.Process()` from an XCUITest.** See Pitfall #3 — the XCUITest runner is an iOS process, not macOS; Process is not available.
- **Assuming the legacy demo path exposes `/health`.** It does NOT — see Pitfall #5.
- **Using `Task.sleep(nanoseconds:)` with wall-clock precision for event scheduling.** Use `continuousClock.sleep(until:)` or batched timing per Swift docs (see §Dynamic event provisioning).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Ed25519 deterministic-seed keypair | Custom key derivation | `Curve25519.Signing.PrivateKey(rawRepresentation: Data)` with a 32-byte hardcoded seed | CryptoKit treats the rawRepresentation as the private scalar seed; passing the same 32 bytes produces the same key every time. `KeyManager` already uses `rawRepresentation` for storage (line 156-161) |
| Keychain reset | Enumerate all keychain items | Call existing `KeyManager.deleteKeypair()` (wraps `SecItemDelete` with service="com.ainm.messenger", account="ed25519.privateKey") | Already implemented at `KeyManager.swift` line 91-102 |
| GRDB schema reset between tests | Drop + recreate tables | `migrator.eraseDatabaseOnSchemaChange = true` already in DEBUG (AppDatabase.swift line 44). Pair with deleting the SQLite file in `makeDefault()` for a truly-empty DB per run | Project already has wipe-on-schema-change; we just need to wipe unconditionally in UAT mode |
| JSON-RPC health endpoint | New endpoint | Reuse `/health/live` from `demo/src/server.rs` line 303 (federation mode) — OR expose a parallel `/health` on the plain path | The endpoint handler is one line (`get(|| async { StatusCode::OK })`) — do the minimum |
| Event timing | `DispatchQueue.asyncAfter` + @unchecked closures | `Task { try await Task.sleep(for: .milliseconds(delta)); await injector.fire(event) }` with `@MainActor` on `fire` | Swift 6 concurrency clean; no @unchecked bridge |
| Process orchestration in CI | Write a Swift/Rust test harness runner | A plain `Makefile` per D-05 | POSIX-standard, zero dependencies, works identically in local + CI |
| Simulator UDID lookup | Parse `xcrun simctl list` from Swift | Use the string literal `booted` from shell commands | `xcrun simctl openurl booted ...` has worked since iOS 5; the Makefile is already outside the sandbox |

**Key insight:** Every piece of "unique" infrastructure Phase 14 needs already exists somewhere — Keychain reset in `KeyManager`, migration in `AppDatabase`, callback closures in `AINMClient`/`AppState`, health endpoint in `server.rs`, CLI parsing in `main.rs`, shared CI matrix in `ios-ci.yml`. This phase is 90% wiring, 10% new code.

## Runtime State Inventory

Phase 14 creates new files; it does not rename anything. A runtime-state inventory would be empty. Stated explicitly:

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — Phase 14 resets GRDB + Keychain per test run by design | None |
| Live service config | None — no external services (n8n, Datadog, etc.) in this project | None |
| OS-registered state | Info.plist `ainm://` URL scheme already registered by Phase 13; UAT reuses | None |
| Secrets/env vars | None — no secrets involved; fixed-seed Ed25519 key is a test fixture, not a secret | None |
| Build artifacts | `DerivedData`/Xcode caches will need a clean on target-set changes | Document in Makefile: `make uat-clean` wipes DerivedData (discretionary) |

## Xcode project modification (SC-1)

### Current project shape (confirmed from pbxproj lines 73-99)

The project uses **Xcode 16's `PBXFileSystemSynchronizedRootGroup`** — a 2024 feature where each target has a *directory* as its source root and file membership is determined by presence in that directory rather than per-file pbxproj entries. Existing groups:

- `A3000001 /* AINMMessenger */` — path=`AINMMessenger`, the app target
- `A3000002 /* AINMMessengerTests */` — path=`AINMMessengerTests`, the unit test target (already has `CryptoTests/`, `NetworkingTests/`, `PersistenceTests/` subdirs)

`PBXFileSystemSynchronizedBuildFileExceptionSet` objects (A8000001/A8000002) already exist to let the project exclude specific files from a root group's membership.

### Recommended approach: **Xcode UI** for the structural add, ruby gem for polish

**Why not pure programmatic:** The `xcodeproj` ruby gem (v1.27.0+) added `file_system_synchronized_groups` support after Xcode 16 released, but **UI test target creation in a sync-group project is poorly documented** and prone to edge cases (test host wiring, bundle loader, build phase copies). A one-time Xcode UI add is the safest path.

**Steps (in order):**

1. Open `ios/AINMMessenger/AINMMessenger.xcodeproj` in Xcode 16.
2. File → New → Target → iOS → "UI Testing Bundle".
3. Product Name: `AINMMessengerUITests` (matches CONTEXT specifics).
4. Target to be Tested: `AINMMessenger`. Language: Swift. Organization ID: matches existing (dev.ainm).
5. Xcode creates a new `AINMMessengerUITests/` directory + adds `A2000003 /* AINMMessengerUITests */` as a PBXNativeTarget with `productType = "com.apple.product-type.bundle.ui-testing"`.
6. Under the scheme editor: share the new scheme (`Manage Schemes... → AINMMessengerUITests → Shared ✓`). This writes `xcshareddata/xcschemes/AINMMessengerUITests.xcscheme`.
7. Commit.

After that, **fine-grained pbxproj edits** (adding `UITestIdentifiers.swift` to the UI test target's compile sources) are cheap:

- Option A — Xcode UI: right-click the file → Target Membership → check both. Xcode edits the `membershipExceptions` field on `A8000001 /* PBXFileSystemSynchronizedBuildFileExceptionSet */` for the UI test target to include the file's relative path.
- Option B — direct text edit of pbxproj: see §Shared file membership below.

### Minimum viable `AINMMessengerUITests.xcscheme` (D-06, SC-1)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<Scheme LastUpgradeVersion="1620" version="1.7">
   <BuildAction parallelizeBuildables="YES" buildImplicitDependencies="YES">
      <BuildActionEntries>
         <BuildActionEntry buildForTesting="YES" buildForRunning="NO"
                           buildForProfiling="NO" buildForArchiving="NO" buildForAnalyzing="NO">
            <BuildableReference
               BuildableIdentifier="primary"
               BlueprintIdentifier="A2000003"          <!-- new UI test target -->
               BuildableName="AINMMessengerUITests.xctest"
               BlueprintName="AINMMessengerUITests"
               ReferencedContainer="container:AINMMessenger.xcodeproj"/>
         </BuildActionEntry>
         <BuildActionEntry buildForTesting="YES" buildForRunning="YES"
                           buildForProfiling="YES" buildForArchiving="YES" buildForAnalyzing="YES">
            <BuildableReference
               BuildableIdentifier="primary"
               BlueprintIdentifier="A2000001"          <!-- existing app target (host) -->
               BuildableName="AINMMessenger.app"
               BlueprintName="AINMMessenger"
               ReferencedContainer="container:AINMMessenger.xcodeproj"/>
         </BuildActionEntry>
      </BuildActionEntries>
   </BuildAction>
   <TestAction buildConfiguration="Debug"
               selectedDebuggerIdentifier="Xcode.DebuggerFoundation.Debugger.LLDB"
               selectedLauncherIdentifier="Xcode.DebuggerFoundation.Launcher.LLDB"
               shouldUseLaunchSchemeArgsEnv="YES"
               shouldAutocreateTestPlan="YES">
      <Testables>
         <TestableReference skipped="NO" parallelizable="NO">
            <BuildableReference
               BuildableIdentifier="primary"
               BlueprintIdentifier="A2000003"
               BuildableName="AINMMessengerUITests.xctest"
               BlueprintName="AINMMessengerUITests"
               ReferencedContainer="container:AINMMessenger.xcodeproj"/>
         </TestableReference>
      </Testables>
   </TestAction>
   <LaunchAction buildConfiguration="Debug" launchStyle="0"
                 selectedDebuggerIdentifier="Xcode.DebuggerFoundation.Debugger.LLDB"
                 selectedLauncherIdentifier="Xcode.DebuggerFoundation.Launcher.LLDB"
                 useCustomWorkingDirectory="NO" ignoresPersistentStateOnLaunch="NO"
                 debugDocumentVersioning="YES" debugServiceExtension="internal"
                 allowLocationSimulation="YES">
      <BuildableProductRunnable runnableDebuggingMode="0">
         <BuildableReference
            BuildableIdentifier="primary"
            BlueprintIdentifier="A2000001"
            BuildableName="AINMMessenger.app"
            BlueprintName="AINMMessenger"
            ReferencedContainer="container:AINMMessenger.xcodeproj"/>
      </BuildableProductRunnable>
   </LaunchAction>
   <ProfileAction buildConfiguration="Release"/>
   <AnalyzeAction buildConfiguration="Debug"/>
   <ArchiveAction buildConfiguration="Release" revealArchiveInOrganizer="YES"/>
</Scheme>
```
Crucial: `parallelizable="NO"` on `TestableReference` — the tests share one booted simulator + one booted server; parallel execution will thrash port 8080. Serial is the correct default for this harness.

### Shared file membership in BOTH targets (D-11)

`UITestIdentifiers.swift` physically lives at `ios/AINMMessenger/AINMMessenger/App/UITestIdentifiers.swift`. Because `AINMMessenger/` is a `PBXFileSystemSynchronizedRootGroup`, it is **automatically** in the app target.

To add to the UI test target, add an explicit file reference AND add it to the `membershipExceptions` of the UI test target's `PBXFileSystemSynchronizedBuildFileExceptionSet`. Concretely, the pbxproj needs entries like:

```
A8000003 /* PBXFileSystemSynchronizedBuildFileExceptionSet for UITests */ = {
    isa = PBXFileSystemSynchronizedBuildFileExceptionSet;
    membershipExceptions = (
        "App/UITestIdentifiers.swift",   // relative to the UI test root group's path
    );
    target = A2000003 /* AINMMessengerUITests */;
};
```
In practice: let Xcode do this via the Target Membership checkbox. The file is a pure `enum` of string `static let` constants; Swift 6 strict concurrency is a non-issue (strings are trivially `Sendable`, `enum` namespace has no instance state). No `@testable import AINMMessenger` needed because the identifiers are compiled directly into the test target.

### Host target question (explicit answer)

**No new host target is needed.** `AINMMessenger` remains the host (`TEST_HOST = "$(BUILT_PRODUCTS_DIR)/AINMMessenger.app/..."` — pattern at pbxproj line 429). UI testing bundles always use the production app as the host; the `productType` differs from unit tests (`ui-testing` vs `unit-test`).

Swift 6 strict concurrency in the app target does not affect UI test target linking: the UI test target uses XCUITest which communicates via accessibility identifiers to the hosted app process — it does not `import AINMMessenger` unless it needs types, which for this phase it does not (only identifiers).

### Confidence

- Xcode UI target add: HIGH — standard Apple flow.
- Scheme shape: HIGH — matches existing AINMMessenger.xcscheme + Apple docs.
- Shared file membership via synchronized-group exceptions: MEDIUM — the mechanism is documented by Pedro Piñera's pbxproj blog (linked Sources) but I haven't verified the exact `membershipExceptions` path-spelling in this project's tooling; let Xcode generate it.
- xcodeproj ruby gem as alternative: LOW — gem v1.27+ supports the ISA but UI-test-target creation path is poorly documented; recommending Xcode UI as the primary path sidesteps this risk.

## Seed bootstrap (D-01..D-04)

### GRDB reset pattern (idempotent, safe under active ValueObservation)

Three options, ranked by safety for Phase 14:

| Option | How | Risk | Recommended |
|--------|-----|------|-------------|
| A. Delete SQLite file before `DatabasePool` init | `try? FileManager.default.removeItem(at: dbURL)` inside SeedLoader, BEFORE `AppDatabase.makeDefault()` | None — no pool exists yet | ✅ **YES** |
| B. `dbPool.erase()` after init | `try await dbPool.erase()` | Throws if observations are attached | ❌ Won't work — `AppState.setup()` attaches observations at line 74 (`let db = try AppDatabase.makeDefault()`) |
| C. DELETE FROM every table in a single write | `try await dbPool.write { db in for t in tables { try db.execute(sql: "DELETE FROM \(t)") } }` | Active ValueObservations will fire with empty state then populated state — UI can flicker, but doesn't crash | ⚠️ Fallback only |

**Recommended:** Option A. Sequence is:

```swift
// Source: composed from AppDatabase.swift lines 13-25 + CONTEXT D-01..D-04
#if DEBUG
enum SeedLoader {
    /// Run before AppDatabase.makeDefault(). Returns the fixture name if seeded.
    static func preparePristineDatabase() throws {
        let fileManager = FileManager.default
        let appSupportURL = try fileManager.url(
            for: .applicationSupportDirectory, in: .userDomainMask,
            appropriateFor: nil, create: true)
        let dbURL = appSupportURL.appendingPathComponent("ainm.sqlite")
        // Also remove WAL + SHM sidecars
        let suffixes = ["", "-wal", "-shm"]
        for suffix in suffixes {
            try? fileManager.removeItem(at: dbURL.appendingPathExtension(suffix))
        }
    }

    static func insertFixture(_ fixtureName: String, into db: AppDatabase) async throws {
        guard let url = Bundle.main.url(forResource: fixtureName, withExtension: "json") else {
            throw SeedError.fixtureNotFound(fixtureName)
        }
        let data = try Data(contentsOf: url)
        let fixture = try JSONDecoder().decode(ScenarioFixture.self, from: data)
        try await db.dbPool.write { db in
            for thread in fixture.threads        { try ThreadRecord(from: thread).insert(db) }
            for participant in fixture.participants { try ParticipantRecord(from: participant).insert(db) }
            for message in fixture.messages      { try MessageRecord(from: message).insert(db) }
            for toolCall in fixture.toolCalls    { try ToolCallRecord(from: toolCall).insert(db) }
            for fact in fixture.archivalFacts    { try MemoryFactRecord(from: fact).insert(db) }
            for card in fixture.agentCards       { try AgentCardRecord(from: card).insert(db) }
        }
    }
}
#endif
```

**Complication: Bundle lookup.** Fixtures are in the UI test target bundle, but `SeedLoader` runs in the app target. Solution: bundle fixtures in the app target under `AINMMessenger/Testing/Fixtures/` guarded by a build-phase script (`if $CONFIGURATION == 'Debug'`) — OR simpler, since fixtures are small, ship them with the app in Debug builds unconditionally. File under `AINMMessenger/Testing/Fixtures/scenario-*.json` (automatically copied into the app bundle because it sits inside the synchronized root group). `Bundle.main.url(forResource:withExtension:)` then finds them.

### Keychain reset (D-04)

```swift
// Source: KeyManager.swift lines 91-102, 167
#if DEBUG
import CryptoKit
import Foundation

enum UITestIdentityBootstrap {
    /// 32-byte deterministic seed for the `uat-user@localhost` Ed25519 keypair.
    /// Using UUID-like literal so diffs are legible.
    static let uatSeedBytes: [UInt8] = [
        0x55, 0x41, 0x54, 0x5F, 0x55, 0x53, 0x45, 0x52,  // "UAT_USER"
        0x40, 0x4C, 0x4F, 0x43, 0x41, 0x4C, 0x48, 0x4F,  // "@LOCALHO"
        0x53, 0x54, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,  // "ST" + zero padding
        0xDE, 0xAD, 0xBE, 0xEF, 0xFE, 0xED, 0xFA, 0xCE,  // magic marker
    ]

    static func resetIdentity() async throws {
        // Delete any existing item (reusing KeyManager's existing logic path)
        let keyManager = KeyManager()
        try await keyManager.deleteKeypair()  // No-op if not present per deleteKeypair's errSecItemNotFound handling

        // Provision deterministic keypair
        let seedData = Data(uatSeedBytes)
        let privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: seedData)

        // Persist using the same Keychain service/account KeyManager uses
        let storage = KeyManager.KeychainStorage()
        try storage.save(privateKey.rawRepresentation)
    }
}
#endif
```
**Confidence:** HIGH on the seed-bytes approach — `Curve25519.Signing.PrivateKey(rawRepresentation: Data)` is deterministic (CryptoKit uses the raw 32-byte scalar as the key). Verified via the existing `KeyManager.getOrCreateKeypair()` code path at `KeyManager.swift` line 156-161.

### JSON fixture schema (illustrative — Claude's discretion per CONTEXT)

```json
{
  "version": "1.0",
  "scenario": "scenario-01-tool-call",
  "identity": "uat-user@localhost",
  "threads": [
    {
      "id": "thread-01",
      "title": "Weather Thread",
      "createdAt": "2026-04-18T10:00:00.000Z",
      "createdBy": "uat-user@localhost",
      "parentThreadId": null,
      "parentMessageId": null,
      "hasUnread": false
    }
  ],
  "participants": [
    { "threadId": "thread-01", "identity": "uat-user@localhost", "type": "human", "joinedAt": "2026-04-18T10:00:00.000Z", "role": "creator" },
    { "threadId": "thread-01", "identity": "helper@localhost",   "type": "agent", "joinedAt": "2026-04-18T10:00:00.000Z", "role": "member" }
  ],
  "messages": [
    {
      "id": "msg-01", "threadId": "thread-01", "fromAddress": "uat-user@localhost",
      "timestamp": "2026-04-18T10:00:01.000Z", "type": "message",
      "contentJson": "[{\"type\":\"text\",\"version\":\"1.0\",\"source\":\"user\",\"text\":\"What's the weather?\",\"fallback\":\"What's the weather?\"}]",
      "signature": "AAAA...",
      "serverTimestamp": null
    }
  ],
  "toolCalls": [],
  "archivalFacts": [],
  "agentCards": []
}
```
The schema mirrors the flat table shape of existing GRDB records (`ThreadRecord`, `ParticipantRecord`, `MessageRecord`, `ToolCallRecord`, `MemoryFactRecord`, `AgentCardRecord`). One JSON field per SQLite column — zero impedance mismatch.

### Exact insertion point in AINMMessengerApp.swift (D-03)

Currently at lines 20-23:
```swift
.onAppear {
    try? appState.setup()
}
```

After Phase 14, lines become:
```swift
.task {
    #if DEBUG
    // Step 1: wipe DB file before AppDatabase.makeDefault() inside setup() creates the pool.
    try? SeedLoader.preparePristineDatabase()
    // Step 2: reset Keychain + provision deterministic keypair.
    try? await UITestIdentityBootstrap.resetIdentityIfRequested()
    #endif
    try? appState.setup()
    #if DEBUG
    // Step 3: load fixture records (requires appDatabase already open).
    try? await SeedLoader.loadFixtureIfRequested(into: appState)
    // Step 4: start event injector.
    await TestEventInjector.scheduleIfRequested(for: appState)
    #endif
}
```

Order (critical):
1. Wipe DB file (SeedLoader.preparePristineDatabase) — before any pool is opened
2. Reset Keychain (UITestIdentityBootstrap) — before appState.setup() calls AINMClient.connect which reads keypair
3. `appState.setup()` — creates pool + client + wires callbacks
4. Load fixture into GRDB — after appDatabase is available
5. Start event injector — after callbacks are wired

**Connection to real server:** Per D-01, AINMClient still connects (for live events). The UAT run needs the identity keypair to match what the seeded participant rows use; hence step 2 happens before `setup()`.

Swap `.onAppear` → `.task` because seed loading is async. `.task` runs when the View appears and is the idiomatic iOS 18 SwiftUI async-entry hook.

## Dynamic event provisioning (D-13..D-16)

### Exhaustive AINMClient callback surface (from AINMClient.swift lines 40-79 + AppState.swift setup)

The injector must be able to fire any of these event types:

| Event | Callback | AppState handler | Scenario that needs it |
|-------|----------|------------------|-------------------------|
| Stream start | `onStreamStart` | `streamingViewModel.streamStarted` | Potential — scenario #1 tests indicators pre/post stream, doesn't need stream itself |
| Stream delta | `onStreamDelta` | `streamingViewModel.updateStreamContent` | — |
| Stream end | `onStreamEnd` | `streamingViewModel.streamEnded` | — |
| Stream error | `onStreamError` | `streamingViewModel.removeStream` | — |
| Presence notify | `onPresenceNotify` | `handlePresence` | Potential scenario-2 (typing indicator optional) |
| Message delivered | `onMessageDelivered` | `activePresence.removeValue` | — |
| **Tool event** | **`onToolEvent`** | **`handleToolEvent`** | **#1 — MANDATORY** |
| **Context frame** | **`onContextFrame`** | **`contextFrames[frame.threadId] = frame`** | **#4 — MANDATORY** |
| Briefing | `onBriefing` | `latestBriefing = briefing` | — |
| Compilation | `onCompilation` | `fetchMemoryGraph()` | — |

Scenarios 2, 3, 5, 6 don't need the injector — they exercise seeded-state + user gestures only. The injector is strictly for scenarios **#1 (tool call lifecycle)** and **#4 (context frame push)**.

### Millisecond-precision event scheduling (Swift 6 Sendable-clean)

**Recommended primitive: `Task { try await Task.sleep(for: .milliseconds(offset)) }` with @MainActor isolation.**

```swift
// Source: Swift Concurrency docs — Task.sleep(for:) Duration-based overload, iOS 16+
#if DEBUG
@MainActor
final class TestEventInjector {
    private let appState: AppState
    private var runningTasks: [Task<Void, Never>] = []

    init(appState: AppState) { self.appState = appState }

    func schedule(script: EventScript) {
        for entry in script.events {
            let task = Task { @MainActor [weak self] in
                if entry.atMs > 0 {
                    try? await Task.sleep(for: .milliseconds(entry.atMs))
                }
                guard !Task.isCancelled, let self else { return }
                self.fire(entry.event)
            }
            runningTasks.append(task)
        }
    }

    private func fire(_ event: ScriptedEvent) {
        switch event {
        case .toolCallStarted(let threadId, let call):
            appState.handleToolEvent(threadId: threadId, event: call)
        case .toolCallProgress(let threadId, let call):
            appState.handleToolEvent(threadId: threadId, event: call)
        case .toolCallCompleted(let threadId, let call):
            appState.handleToolEvent(threadId: threadId, event: call)
        case .contextFrame(let frame):
            appState.contextFrames[frame.threadId] = frame
        }
    }
}
#endif
```

**Why not `DispatchQueue.asyncAfter`:** returns from main thread, needs @unchecked Sendable closures, fights strict concurrency.

**Why not `Timer`:** Timers don't fire in some background run-loop modes during test simulation. `Task.sleep(for:)` uses Foundation's monotonic clock and works reliably.

**Jitter budget:** `Task.sleep(for:)` has `clock.minimumResolution` precision (typically 1ms on iOS 18); tests should use `waitForExistence(timeout: 5.0)` or more per assertion to absorb simulator jitter.

### Piggyback vs. parallel dispatch (tested-code-path guarantee)

The injector calls **`appState.handleToolEvent(...)` directly** — exactly what `AppState.setup()` wires as the `AINMClient.onToolEvent` callback (AppState.swift lines 124-128). Same code path, same UI render graph. This satisfies the CONTEXT constraint "Exercises the real UI rendering path from notification → ViewModel → SwiftUI."

Alternative (rejected): push through `AINMClient` itself. Would require an `injectNotification(_:)` method on the actor — more surface area, no additional fidelity.

### Mid-test event triggering (XCUIApplication launchArguments are immutable)

**Limitation:** `XCUIApplication.launchArguments` only affect the initial launch. Any app restart during a test creates a new process with new args.

**For Phase 14 this is NOT a problem:**
- Scenario #1 uses a single event script that fires 0ms / 800ms / 2000ms relative to launch — no mid-test re-triggering needed.
- Scenario #4 uses a single context-frame push at a known offset.
- Scenarios 2, 3, 5, 6 don't use the injector.

**If mid-test triggering ever becomes necessary (deferred per CONTEXT):** the pattern is to add a hidden SwiftUI view that reads `NotificationCenter` and responds to a named notification posted via `app.buttons["hidden.trigger.buttonN"].tap()`. Not needed now.

### Event script JSON shape (Claude's discretion, recommended)

```json
{
  "version": "1.0",
  "scenario": "scenario-01-tool-call",
  "events": [
    { "atMs": 0,    "type": "tool.call.started",   "threadId": "thread-01", "callId": "c1", "name": "weather_lookup", "input": {"city": "SF"} },
    { "atMs": 800,  "type": "tool.call.progress",  "threadId": "thread-01", "callId": "c1", "progress": {"current": 1, "total": 2, "detail": "Querying..."} },
    { "atMs": 2000, "type": "tool.call.completed", "threadId": "thread-01", "callId": "c1", "status": "success", "resultSummary": "SF: 18C", "durationMs": 2000 }
  ]
}
```

## Accessibility identifiers retrofit (D-09..D-12)

### Scenario → element → identifier inventory (the exhaustive retrofit list)

| Scenario | Expected behavior (from 13-HUMAN-UAT.md) | SwiftUI view file | Element | Proposed identifier |
|----------|------------------------------------------|--------------------|---------|----------------------|
| **1 Tool call** | Indicator appears below message list with spinner (running) | `UI/ToolVisibility/ToolCallIndicatorView.swift` | Collapsed row container | `toolCall.indicator.row.{callId}` |
| 1 | Status icon running | same | `ProgressView()` spinner | `toolCall.indicator.status.running` |
| 1 | Updates to green checkmark (completed) | same | `checkmark.circle.fill` | `toolCall.indicator.status.completed` |
| 1 | Red X (error) | same | `xmark.circle.fill` | `toolCall.indicator.status.error` |
| 1 | Clock (timed out) | same | `clock.badge.questionmark` | `toolCall.indicator.status.timedOut` |
| 1 | Tap expands to show input JSON panel | same | Expand chevron (tap target) | `toolCall.indicator.expandButton` |
| 1 | Input JSON section | `UI/ToolVisibility/ToolCallDetailView.swift` | JSON scrollable container | `toolCall.detail.inputJson` |
| 1 | Result summary | same | Result text | `toolCall.detail.result` |
| **2 @-mentions** | Typing `@` in composer → popup appears | `UI/Composer/ComposerView.swift` | Composer text field | `composer.textField` |
| 2 | Popup filters by typed text | `UI/Mentions/MentionAutocompleteView.swift` | Popup scroll container | `composer.mention.autocomplete.list` |
| 2 | Each row | same | Row (one per `MentionCandidate`) | `composer.mention.autocomplete.row.{identity}` |
| 2 | "No matches" empty state | same | Text | `composer.mention.autocomplete.empty` |
| 2 | Mention in rendered message | `UI/ContentBlocks/TextBlockView.swift` | n/a — assert via `XCUIElementQuery` on text content | (no identifier — test asserts via `.staticTexts["@Remy"].exists`) |
| **3 Agent discovery** | Discover Agents view | `UI/AgentDiscovery/AgentDiscoveryView.swift` | Nav title / search | `agentDiscovery.search` |
| 3 | Agent card row | same | Row (one per agent) | `agentDiscovery.list.cell.{agentId}` |
| 3 | Tap agent → profile | `UI/AgentDiscovery/AgentCardDetailView.swift` | Scroll view | `agentDiscovery.detail` |
| 3 | Capability badge (Streaming) | same | Pill | `agentDiscovery.detail.badge.streaming` |
| 3 | Skills section header | same | Text | `agentDiscovery.detail.skillsHeader` |
| 3 | Start Conversation button | same | Button | `agentDiscovery.detail.startConversation` |
| 3 | Add Agent by address | `UI/AgentDiscovery/AgentDiscoveryView.swift` | Text field | `agentDiscovery.addByAddress.textField` |
| 3 | Add Agent button | same | Button | `agentDiscovery.addByAddress.button` |
| **4 Memory** | Context pill in thread toolbar | `UI/MessageThread/MessageThreadView.swift` | Toolbar button | `thread.toolbar.memoryPill` |
| 4 | Summary section | `UI/Memory/MemoryContextView.swift` | Section container | `memory.sheet.summary` |
| 4 | Active Context section | same | Section container | `memory.sheet.activeContext` |
| 4 | Archival Facts preview | same | Section container | `memory.sheet.archivalPreview` |
| 4 | View All button | same | Button | `memory.sheet.viewAllButton` |
| 4 | Search field in archival | `UI/Memory/ArchivalMemoryView.swift` | SearchField | `memory.archival.search` |
| 4 | Archival fact row | same | Row (one per fact) | `memory.archival.row.{factId}` |
| 4 | Swipe-delete confirmation | same | Alert "Delete" button | `memory.archival.deleteConfirm` |
| **5 Sub-thread** | Long-press shows Branch Thread | `UI/MessageThread/MessageBubbleView.swift` or `MessageGroupView.swift` | Context menu item | `thread.message.row.{messageId}` on the bubble (system sets `.menuItems["Branch Thread"]`) |
| 5 | Branch creates sub-thread + navigates | — | n/a (assert via sub-thread banner visible) | — |
| 5 | Sub-thread banner "Thread from {sender}" | `UI/SubThreading/SubThreadBannerView.swift` | HStack container | `subThread.banner` |
| 5 | Back arrow | same | arrow.uturn.backward button | `subThread.banner.backButton` |
| 5 | View Parent | same | Text button | `subThread.banner.viewParent` |
| 5 | Reply count badge | `UI/SubThreading/SubThreadIndicatorView.swift` | Button | `thread.message.row.{messageId}.replyBadge` |
| **6 Deep link** | Opening `ainm://thread/{id}` navigates | `UI/MessageThread/MessageThreadView.swift` | Thread title container | `thread.view.{threadId}` |
| 6 | Scrolls to message | — | Message bubble | `thread.message.row.{messageId}` (reuse) |
| 6 | Not found alert | `App/AINMMessengerApp.swift` | Alert button "OK" | (SwiftUI auto-announces alerts; use `.alerts.buttons["OK"]`) |
| 6 | Message not found toast | `UI/MessageThread/MessageThreadView.swift` | Toast text | `thread.toast.messageNotFound` |

**Count:** ~34 identifiers across ~12 files. Within the D-09 target of 20–40 touches across ~10 files.

### `UITestIdentifiers.swift` structure

```swift
// Source: CONTEXT D-10 + D-11 — D-11 says "a single UITestIdentifiers.swift file".
// Shared with both targets via pbxproj membershipExceptions.
import Foundation

enum UITestID {
    // MARK: - Scenario 1: Tool call visibility
    enum ToolCall {
        static func indicatorRow(callId: String) -> String { "toolCall.indicator.row.\(callId)" }
        static let statusRunning   = "toolCall.indicator.status.running"
        static let statusCompleted = "toolCall.indicator.status.completed"
        static let statusError     = "toolCall.indicator.status.error"
        static let statusTimedOut  = "toolCall.indicator.status.timedOut"
        static let expandButton    = "toolCall.indicator.expandButton"
        static let detailInputJson = "toolCall.detail.inputJson"
        static let detailResult    = "toolCall.detail.result"
    }
    // MARK: - Scenario 2: @-Mentions
    enum Mention {
        static let composerField        = "composer.textField"
        static let autocompleteList     = "composer.mention.autocomplete.list"
        static func autocompleteRow(identity: String) -> String { "composer.mention.autocomplete.row.\(identity)" }
        static let autocompleteEmpty    = "composer.mention.autocomplete.empty"
    }
    // ... similar namespaces for AgentDiscovery, Memory, SubThread, DeepLink
}
```

### Swift 6 strict concurrency (confirmed non-issue)

`enum` namespaces with only `static let String` / `static func(...) -> String` are fully `Sendable`. No annotations, no global variables, no `@MainActor`. The file is trivially safe to include in any target.

Application sites (e.g. `.accessibilityIdentifier(UITestID.ToolCall.statusRunning)`) run on `@MainActor` SwiftUI views, which is the status quo.

### `#if DEBUG` question

CONTEXT D-11 says "gated `#if DEBUG` if needed." Recommended: **do NOT gate the identifier constants file** — leave it in both targets always. Reasons:

1. String constants have zero runtime cost.
2. Gating creates a failure mode where a release-config test build fails to resolve the constants.
3. The constants are never seen by end users (accessibilityIdentifier is not read aloud by VoiceOver — accessibilityLabel is).

Do gate the `.accessibilityIdentifier(UITestID.…)` modifier calls in app views if there is any concern about release binary bloat, but the strings are literal 20-40 byte entries — not worth the macro complexity.

## XCUITest suite architecture (SC-3)

### `AINMUITestCase` base class (recommended)

```swift
import XCTest

class AINMUITestCase: XCTestCase {
    var app: XCUIApplication!

    override func setUpWithError() throws {
        try super.setUpWithError()
        continueAfterFailure = false
        app = XCUIApplication()
    }

    /// Launch the app with seed + optional event script.
    func launchWith(seedFixture: String, eventScript: String? = nil) {
        var args = ["-UITestSeed", seedFixture]
        if let eventScript { args += ["-UITestEvents", eventScript] }
        app.launchArguments = args
        app.launch()

        // Wait for app readiness (see below)
        let readinessSignal = app.otherElements[UITestID.App.ready]
        XCTAssertTrue(readinessSignal.waitForExistence(timeout: 10),
                      "App never signalled ready after seed load")
    }

    override func tearDownWithError() throws {
        app?.terminate()
        try super.tearDownWithError()
    }
}
```

### App readiness signal (critical for seed-based tests)

The seed loader is async; tests must wait until:
1. SeedLoader has inserted fixture rows into GRDB
2. `AppState.setup()` has opened the pool and wired callbacks
3. `AINMClient.connect()` has completed handshake (optional but recommended)

**Mechanism:** Expose a specific accessibility element that is only rendered when the app is ready.

```swift
// In AINMMessengerApp body after setup:
Group { ... }
    .overlay(alignment: .topLeading) {
        // Invisible 0-size ready marker — set only when bootstrap is complete.
        if bootstrapComplete {
            Color.clear
                .frame(width: 0, height: 0)
                .accessibilityElement()
                .accessibilityIdentifier("app.ready")
                .accessibilityHidden(true)   // hidden from VoiceOver, visible to XCUITest queries
        }
    }
```
Add `UITestID.App.ready = "app.ready"` to `UITestIdentifiers.swift`.

### Scenario #1 — Tool call indicator visual states (the script)

```swift
final class ToolCallIndicatorUITests: AINMUITestCase {
    func test_toolCallLifecycle_transitionsRunningToCompleted() {
        launchWith(seedFixture: "scenario-01-tool-call",
                   eventScript: "scenario-01-events")

        // Navigate into the seeded thread
        app.buttons["conversationList.cell.thread-01"].tap()

        // 0ms: started → running state visible
        let running = app.images[UITestID.ToolCall.statusRunning]
        XCTAssertTrue(running.waitForExistence(timeout: 2),
                      "Running spinner did not appear after tool.call.started")

        // 800ms: progress
        // (nothing visible changes for progress in the collapsed indicator — just no-op)

        // 2000ms: completed → checkmark
        let completed = app.images[UITestID.ToolCall.statusCompleted]
        XCTAssertTrue(completed.waitForExistence(timeout: 5),
                      "Completed checkmark did not appear after tool.call.completed")

        // Expand and verify input JSON visible
        app.buttons[UITestID.ToolCall.expandButton].tap()
        XCTAssertTrue(app.staticTexts[UITestID.ToolCall.detailInputJson].waitForExistence(timeout: 2))
    }
}
```

### Scenario #2 — @-mention autocomplete (exact gestures)

```swift
final class MentionAutocompleteUITests: AINMUITestCase {
    func test_atTriggersAutocomplete_selectionInsertsMention() {
        launchWith(seedFixture: "scenario-02-mentions")
        app.buttons["conversationList.cell.thread-01"].tap()

        let composer = app.textFields[UITestID.Mention.composerField]
        XCTAssertTrue(composer.waitForExistence(timeout: 5))
        composer.tap()
        composer.typeText("@Re")

        // Autocomplete appears
        let list = app.otherElements[UITestID.Mention.autocompleteList]
        XCTAssertTrue(list.waitForExistence(timeout: 2))

        // Specific row — fixture seeded a participant "remy@localhost"
        let remyRow = app.buttons[UITestID.Mention.autocompleteRow(identity: "remy@localhost")]
        XCTAssertTrue(remyRow.waitForExistence(timeout: 2))
        remyRow.tap()

        // Mention inserted
        XCTAssertTrue(composer.value as? String == "@Remy ")

        // Send and assert bubble contains @Remy highlighted
        app.buttons["composer.sendButton"].tap()
        // The rendered bubble's static text will contain "@Remy".
        XCTAssertTrue(app.staticTexts["@Remy"].waitForExistence(timeout: 2))
    }
}
```

### Scenario #6 — Deep link (the simctl approach)

**Given Pitfall #3 (Process() unavailable in XCUITest runner) and Pitfall #2 (`XCUIApplication.open(_:)` unreliable on simulator), the canonical path is:**

1. Makefile boots the simulator and installs the app.
2. Makefile invokes `xcrun simctl openurl booted ainm://thread/thread-01` *before* running the specific deep-link test.
3. The deep-link test skips launching the app itself; it attaches to the running instance:

```swift
final class DeepLinkUITests: AINMUITestCase {
    override func setUpWithError() throws {
        try super.setUpWithError()
        // App already launched + deep-link already triggered by external Makefile step.
        // We attach to the running app instead of launching fresh.
    }

    func test_deepLink_navigatesToThread() {
        let threadView = app.otherElements["thread.view.thread-01"]
        XCTAssertTrue(threadView.waitForExistence(timeout: 5))
    }
}
```

**Alternative if external shell-out is too awkward:** launch the app fresh with the deep link embedded via `launchEnvironment["AINM_DEEP_LINK"] = "ainm://thread/thread-01"` and have the `#if DEBUG` bootstrap synthesize a deep-link call via `NotificationCenter.default.post(...)` that wires to the same `onOpenURL` handler. This bypasses the simulator limitation but doesn't exercise the real `.onOpenURL` path.

**Recommended:** Option 1 (Makefile + attach). It matches how `13-HUMAN-UAT.md` describes the manual test ("In Simulator terminal: `xcrun simctl openurl booted ...`").

### Streaming cursor assertion strategy

From CONTEXT: "how to verify a cursor is rendering at 30–60fps without snapshotting."

**Answer:** Don't verify FPS — verify existence. The cursor view has an accessibility identifier (e.g. `streaming.cursor`). During the stream window (after `ainm.stream.start`, before `ainm.stream.end`), `app.otherElements["streaming.cursor"].exists` should be `true`. `waitForExistence(timeout: 1.0)` is sufficient.

FPS verification requires XCTest's `MetricKit` integration and is out of scope per locked deferral (snapshot / visual-regression testing is explicitly deferred).

## Rust server orchestration (D-05..D-08)

### Confirmed state of demo/src/server.rs

**Binary name:** `ainm-demo` (Cargo.toml line 11) — NOT `ainm-server`. Makefile must use:
```makefile
cargo run --bin ainm-demo -- --uat
```

**Health endpoints present:**
- `/health/live` — returns `StatusCode::OK` unconditionally (server.rs line 303, **federation mode only**)
- `/health/ready` — database check (server.rs line 304, federation mode only)

**Health endpoint GAP:** The `run_legacy_demo` path → `server::start` (server.rs line 231-267) binds `127.0.0.1:0` (random port) and has NO `/health` route. Since CONTEXT locks port 8080 and a `/health` poll, the `--uat` flag MUST force federation mode with explicit port 8080 — OR `server::start` must be extended to include the health routes.

**Recommended fix:** extend `server::start` to also register `/health/live` and allow a port parameter:

```rust
// demo/src/server.rs — extend start() signature
pub async fn start_with_port(
    identity_store: IdentityStore,
    agent_card: serde_json::Value,
    port: u16,
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    // ... same as start() but:
    let app = Router::new()
        .route("/ws", get(ws_upgrade_handler))
        .route("/.well-known/ainm/{agent_name}/agent-card.json", get(agent_card_handler))
        .route("/health", get(|| async { StatusCode::OK }))      // ← new
        .route("/health/live", get(|| async { StatusCode::OK }))  // ← new, matches federation
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    // ... unchanged
}
```

Path of least resistance: the `--uat` handler in main.rs calls `start_with_port(.., 8080)` and spawns a minimal agent to respond to messages.

### CLI arg parsing (main.rs uses clap already)

Add to the `Cli` struct (demo/src/main.rs line 28):

```rust
#[derive(Parser)]
#[command(name = "ainm-demo", about = "AINM Protocol Demo")]
struct Cli {
    // ... existing fields ...

    /// Run in UAT automation mode: ephemeral SQLite, fixed port 8080, INFO logs to tmp/uat-server.log
    #[arg(long)]
    uat: bool,
}
```

Then branch in `main`:
```rust
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.uat {
        return run_uat(cli).await;
    }
    // ... existing dispatch
}

async fn run_uat(cli: Cli) -> Result<()> {
    // 1. INFO logging to file
    let log_file = std::fs::File::create("tmp/uat-server.log")?;
    tracing_subscriber::fmt()
        .with_target(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::sync::Mutex::new(log_file))
        .init();

    // 2. Ephemeral SQLite path (UAT uses in-memory ServerState, but if DB configured, override path)
    // Current db.rs only uses PostgreSQL. For UAT the in-memory ServerState is fine.
    // No DB init needed since we only need WebSocket + /health.

    // 3. Setup identities (reuse legacy demo's human+agent at demo.ainm.local)
    let human = identity::generate_keypair("uat-user@localhost")?;
    let agent = identity::generate_keypair("helper@localhost")?;
    let identity_store = identity::IdentityStore::new();
    identity_store.register(&human.address, &human.public_key_b64(), "human", "localhost", None);
    identity_store.register(&agent.address, &agent.public_key_b64(), "agent", "localhost",
        Some("http://localhost:8080/.well-known/ainm/helper/agent-card.json".to_string()));

    let agent_card = /* reuse legacy_demo's agent_card JSON */;

    // 4. Bind fixed port 8080 with /health
    let (addr, handle) = server::start_with_port(identity_store.clone(), agent_card, 8080).await?;
    tracing::info!("[uat] Server ready on ws://{}/ws, GET /health returns 200", addr);

    // 5. Block on Ctrl-C — Makefile manages lifecycle
    tokio::signal::ctrl_c().await?;
    handle.abort();
    Ok(())
}
```

**Confidence:** MEDIUM-HIGH — exact signatures match what's in server.rs and main.rs. The `start_with_port` helper is a new addition; verify via `cargo build` in Plan 1.

### Makefile pattern (POSIX-safe, D-05..D-07)

```makefile
# Source: Makefile best practices for background-process + trap handlers
# See https://www.gnu.org/software/make/manual/html_node/Special-Targets.html

.PHONY: uat uat-clean

UAT_SERVER_LOG := tmp/uat-server.log
UAT_SERVER_PID := tmp/uat-server.pid
UAT_PORT := 8080
UAT_HEALTH_URL := http://localhost:$(UAT_PORT)/health
UAT_HEALTH_TIMEOUT_SECONDS := 30
UAT_SCHEME := AINMMessengerUITests
UAT_DESTINATION := 'platform=iOS Simulator,name=iPhone 16,OS=18.0'

uat: ## Run hermetic UAT suite (boot server, run tests, teardown)
	@mkdir -p tmp
	@# Start server in background, capture PID, trap EXIT for teardown
	@( cd demo && cargo run --quiet --bin ainm-demo -- --uat > ../$(UAT_SERVER_LOG) 2>&1 & echo $$! > ../$(UAT_SERVER_PID) )
	@trap 'echo "[uat] Tearing down server (PID $$(cat $(UAT_SERVER_PID)))"; kill $$(cat $(UAT_SERVER_PID)) 2>/dev/null; rm -f $(UAT_SERVER_PID)' EXIT INT TERM; \
	 echo "[uat] Waiting for server readiness at $(UAT_HEALTH_URL)..." ; \
	 for i in $$(seq 1 $(UAT_HEALTH_TIMEOUT_SECONDS)); do \
	   if curl -sf -o /dev/null "$(UAT_HEALTH_URL)"; then \
	     echo "[uat] Server ready after $$i second(s)"; break; \
	   fi; \
	   if [ $$i -eq $(UAT_HEALTH_TIMEOUT_SECONDS) ]; then \
	     echo "[uat] Server not ready after $(UAT_HEALTH_TIMEOUT_SECONDS)s (see $(UAT_SERVER_LOG))" >&2; exit 1; \
	   fi; \
	   sleep 1; \
	 done ; \
	 echo "[uat] Running xcodebuild test -scheme $(UAT_SCHEME)..." ; \
	 ( cd ios/AINMMessenger && xcodebuild test \
	     -project AINMMessenger.xcodeproj \
	     -scheme $(UAT_SCHEME) \
	     -destination $(UAT_DESTINATION) \
	     CODE_SIGNING_ALLOWED=NO \
	     -resultBundlePath ../../tmp/UAT-Results )

uat-clean: ## Remove UAT artifacts
	rm -rf tmp/
```

**Notes:**
- `trap … EXIT INT TERM` guarantees teardown on Ctrl-C, test failure, and normal exit.
- `curl -sf` fails silently on non-2xx; the loop retries.
- `CODE_SIGNING_ALLOWED=NO` matches `ios-ci.yml` line 42.
- `parallelizable="NO"` in scheme + serial test execution means ports don't collide.
- `iPhone 16 / iOS 18.0` matches `ios-ci.yml` line 41.

### Placeholder CI workflow (D-08)

```yaml
# .github/workflows/ios-uat.yml
name: iOS UAT (manual only)

# Phase 14 ships this as workflow_dispatch-only.
# Flipping to push/PR triggers is a future phase per ROADMAP deferred items.
on:
  workflow_dispatch:

jobs:
  uat:
    runs-on: macos-14
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@v4
      - name: Select Xcode 16
        run: sudo xcode-select -s /Applications/Xcode_16.0.app/Contents/Developer
      - name: Cache cargo target
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            demo/target
          key: ${{ runner.os }}-cargo-${{ hashFiles('demo/Cargo.lock') }}
      - name: Resolve SPM dependencies
        working-directory: ios/AINMMessenger
        run: xcodebuild -resolvePackageDependencies -project AINMMessenger.xcodeproj -scheme AINMMessenger
      - name: Run UAT via Makefile
        run: make uat
```

## HUMAN-UAT status update (SC-5)

### Template for updated `13-HUMAN-UAT.md`

The current frontmatter (lines 1-7) is:
```yaml
---
status: partial
phase: 13-ai-native-features-platform
source: [13-VERIFICATION.md]
started: 2026-03-30T16:05:00Z
updated: 2026-03-30T16:05:00Z
---
```

Post-Phase-14 template:
```yaml
---
status: complete
phase: 13-ai-native-features-platform
source: [13-VERIFICATION.md]
started: 2026-03-30T16:05:00Z
updated: <timestamp of green run>
automation: .planning/phases/14-uat-automation-harness/
result_bundle: tmp/UAT-Results.xcresult
---
```
Each of the six `### N.` tests sets `result: [pass]` with `verified_by: automated (AINMMessengerUITests.{ClassName}.{methodName})`.

Summary block becomes:
```
total: 6
passed: 6
issues: 0
pending: 0
skipped: 0
blocked: 0
```

### Update mechanism

**Recommendation:** Manual edit in the last task of the phase after the green run, NOT a script. Reasons:

1. Editing the Markdown requires human confirmation that the run was truly green (avoids false-positive status flip).
2. `/gsd:verify-work` at phase completion is already the canonical way to promote status — Phase 14's summary commit should include the HUMAN-UAT edit.
3. A script for a one-time status flip is over-engineered. Can be added later if multiple phases need similar flips.

If later a script is wanted (discretion): bash + `awk` could replace `status: partial` with `status: complete` and the six `result: [pending]` lines with `result: [pass]` — trivially one-shot.

## Protocol fidelity assertions

### Invariants from spec/*.md that fixtures must respect

(I have not read every spec in depth — planner should read the cited sections when authoring the fixture generation tasks.)

| Spec | Invariant | Fixture constraint |
|------|-----------|---------------------|
| `spec/tool-visibility.md` | Tool call lifecycle: `started` → 0..N `progress` → exactly one `completed` (with status=success or error) | Event script `scenario-01-events.json` must sequence these; fixture rows for `ToolCallRecord` must not exist in-progress at fixture time (indicator should appear via injector, not seed) |
| `spec/streaming.md` | Stream events: `started` → 0..N `delta` → exactly one `end` (status=completed/cancelled/error) | Any scripted streams must sequence this; injector emits matching StreamEvent model types |
| `spec/memory.md` | Context frame schema v1 includes `threadId`, `summary`, `activeMessages[]`, `archivalFacts[]`, `participants[]` | Fixture `ContextFrame` rows must include all five fields |
| `spec/threading.md` | Sub-thread: one-level-only; each sub-thread has `parentThreadId` + `parentMessageId` | Fixture for scenario 5 creates parent thread + parent message first, child thread second, references the parent |
| `spec/transport.md` | `ainm.initialize` handshake precedes all other messages | Not a fixture constraint — AINMClient handles this |

Fixture generation can go deeper (generate actual Ed25519 signatures via the fixed-seed key) but the UAT GRDB path skips signature verification because GRDB writes bypass `AINMClient.sendMessage`. Fixture `signature` field can be a placeholder string if the test doesn't assert on it; MessageRecord requires the field (`signature: .text).notNull()` in `v1_create_messages`).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Xcode | xcodebuild test, simctl | Assumed available on dev + macos-14 CI runner | 16.x (project pbxproj requires 16.2+) | — |
| iOS 18 Simulator | xcodebuild -destination | Assumed (Xcode 16.0 ships iOS 18.0) | 18.0 | — |
| iPhone 16 simulator runtime | -destination name | Assumed per existing `ios-ci.yml` usage | 18.0 | iPhone 16 Pro if missing |
| Rust toolchain | cargo run --bin ainm-demo | Assumed | 1.85+ | — |
| cargo | Makefile boots server | Assumed | bundled with rustup | — |
| curl | health poll in Makefile | ✓ (macOS built-in) | — | wget (less portable) |
| GNU Make | orchestrator | ✓ (macOS ships BSD Make which is compatible for this scope) | 3.81+ | bash script (more verbose but works identically) |
| SQLite (`libsqlite3.dylib`) | GRDB runtime | ✓ (macOS built-in) | — | — |
| ports 8080 | fixed UAT port | ⚠ **potential conflict** — common dev port | — | Makefile must fail fast if port is in use; document in `docs/testing.md` |

**Missing dependencies with no fallback:** None.

**Missing dependencies with fallback:** Port 8080 availability — document a preflight `lsof -i :8080` step in the Makefile for diagnostic output.

## Common Pitfalls

### Pitfall 1: Xcode 16 synchronized groups + ruby gems

**What goes wrong:** Adding the UI test target via an older `xcodeproj` gem version (< 1.27) fails with `unknown ISA PBXFileSystemSynchronizedRootGroup`.
**Why it happens:** Apple introduced the folder-sync feature in Xcode 16; tool ecosystem lagged.
**How to avoid:** Use Xcode UI (File → New → Target) for the one-time target add. Do not try to generate the target programmatically in this phase.
**Warning signs:** `pod install`-style errors mentioning unknown ISAs; `xcodeproj` gem erroring at project open.

### Pitfall 2: `XCUIApplication.open(_:)` is unreliable on iOS 18 simulator

**What goes wrong:** Test calls `app.open(URL(string: "ainm://thread/x")!)` — app launches but deep link handler never fires.
**Why it happens:** Known Apple Simulator bug through iOS 18 (forum threads active from 2023 through 2026).
**How to avoid:** Use `xcrun simctl openurl booted ainm://thread/{id}` invoked from the Makefile (outside the XCUITest process), then have the deep-link test attach to the already-running app.
**Warning signs:** Deep-link test passes individually but fails in a suite run; app foregrounds but stays on its launch screen.

### Pitfall 3: `Foundation.Process()` unavailable in iOS XCUITest runner

**What goes wrong:** Test calls `Process()` in `setUpWithError()` to invoke `simctl` — fails at compile or runtime.
**Why it happens:** The XCUITest runner process is itself an iOS app. iOS does not include `Foundation.Process` (macOS-only API).
**How to avoid:** Shell out from the Makefile before running the test. The test reads seed state from the running app only.
**Warning signs:** `No such module 'Foundation.Process'` or `Process is unavailable in this environment` at compile.

### Pitfall 4: Launch arguments are one-shot

**What goes wrong:** Test triggers more events mid-flight and expects them to arrive — doesn't happen.
**Why it happens:** `XCUIApplication.launchArguments` only affect the process's initial launch via `ProcessInfo.processInfo.arguments`. Changing them after launch has no effect.
**How to avoid:** Design event scripts to cover the entire timeline of a single test from launch. If future scenarios truly need mid-test re-triggering, add a hidden "fire next event" button via `UITestID` (deferred).
**Warning signs:** Test sometimes passes (if event delivery happens before assertion) but flakes at higher iOS versions with stricter launch-time budgets.

### Pitfall 5: Legacy demo mode has no /health

**What goes wrong:** `cargo run --bin ainm-demo` (no `--mode`) hits `run_legacy_demo` → `server::start` which binds `127.0.0.1:0` (random port, NO /health). Makefile `curl http://localhost:8080/health` loops forever.
**Why it happens:** The federation refactor added health routes only on the federation mode's `server::start_federation` path.
**How to avoid:** The `--uat` flag must route through a code path that binds port 8080 AND registers `/health`. Extend `server::start` (or add `start_with_port`) rather than piggybacking on legacy.
**Warning signs:** `make uat` hangs at "Waiting for server readiness"; `lsof -i :8080` shows nothing; `tmp/uat-server.log` shows `ws://127.0.0.1:<random>/ws`.

### Pitfall 6: GRDB observations fire with stale+fresh state during seed

**What goes wrong:** Seed fixture inserts 10 messages; UI briefly shows 0 messages then 10 because `ValueObservation` fired between writes.
**Why it happens:** GRDB `ValueObservation` fires once per transaction. If seed inserts happen after pool is open and observations are attached, intermediate states can render.
**How to avoid:** Delete the SQLite file BEFORE `AppDatabase.makeDefault()` opens the pool (recommended). Alternative: wrap all seed inserts in a single `dbPool.write { db in ... }` transaction so observers see one atomic change.
**Warning signs:** XCUITest assertions flake with race conditions; initial "empty state" view briefly visible in debug runs.

### Pitfall 7: Keychain state persists across simulator boots

**What goes wrong:** Test A runs, stores uat-user key. Test B runs, `KeyManager.getOrCreateKeypair()` loads the same key — but if the seed regenerates the Ed25519 private key, signatures mismatch.
**Why it happens:** Simulator Keychain persists unless `Erase All Content and Settings` runs.
**How to avoid:** `UITestIdentityBootstrap.resetIdentity()` **always** runs on `-UITestSeed` launch (D-04). Delete the existing item, then insert the fixed-seed bytes. Subsequent runs see the same deterministic key.
**Warning signs:** First CI run passes, re-run fails with signature verification errors.

### Pitfall 8: Port 8080 already in use (dev workflow collision)

**What goes wrong:** Developer has another service on 8080 (common); `make uat` boots server, Rust binary errors with "Address already in use"; Makefile doesn't detect; health poll times out.
**Why it happens:** Tokio binding fails, but the background `cargo run` process is still alive (printing the error to log, not exiting).
**How to avoid:** Makefile preflight: `@if lsof -i :8080 > /dev/null 2>&1; then echo "[uat] Port 8080 in use; aborting" >&2; exit 1; fi`
**Warning signs:** `uat-server.log` contains "failed to bind ... Address already in use"; health poll times out.

## Code Examples

### Reset SQLite file (idempotent)

```swift
// Source: FileManager API + AppDatabase.swift line 20
try? FileManager.default.removeItem(at: dbURL)                        // main file
try? FileManager.default.removeItem(at: dbURL.appendingPathExtension("wal"))  // WAL
try? FileManager.default.removeItem(at: dbURL.appendingPathExtension("shm"))  // SHM
```

### Deterministic Ed25519 keypair from fixed seed

```swift
// Source: CryptoKit Curve25519.Signing.PrivateKey(rawRepresentation:) — iOS 13+
// https://developer.apple.com/documentation/cryptokit/curve25519/signing/privatekey
let seed = Data([/* 32 bytes */])
let privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: seed)
// privateKey.publicKey and signatures are now deterministic
```

### XCUITest launch with args, wait for readiness

```swift
// Source: XCUIApplication docs + Swift by Sundell patterns
let app = XCUIApplication()
app.launchArguments = ["-UITestSeed", "scenario-01-tool-call",
                       "-UITestEvents", "scenario-01-events"]
app.launch()
XCTAssertTrue(app.otherElements["app.ready"].waitForExistence(timeout: 10))
```

### Makefile background process with trap

```makefile
# Source: POSIX shell + GNU Make best practice
@( cd demo && cargo run --bin ainm-demo -- --uat > ../tmp/uat-server.log 2>&1 & echo $$! > ../tmp/uat-server.pid )
@trap 'kill $$(cat tmp/uat-server.pid) 2>/dev/null; rm -f tmp/uat-server.pid' EXIT INT TERM; \
 <run tests here>
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Per-file pbxproj entries (pre-Xcode 16) | `PBXFileSystemSynchronizedRootGroup` (folder-sync) | Xcode 16.0 (Sept 2024) | `xcodeproj` ruby gem must be v1.27.0+; older CI tooling breaks until upgraded |
| `.onAppear { setup() }` for launch | `.task { await setup() }` | SwiftUI iOS 15+ (`.task` modifier) | Clean async bootstrap; required for this phase's async seed loading |
| Deep link via Safari workaround | `xcrun simctl openurl` shell invocation | Always the simulator canonical path | `XCUIApplication.open(_:)` is still unreliable on iOS 18 simulator — shell-out is the only robust option |
| In-memory GRDB for tests | `DatabasePool` with file path + wipe between runs | GRDB 5+ requires file-based pool (WAL mode) | `AppDatabase.makeInMemory()` already uses a tempdir file path — same pattern here |

**Deprecated / outdated:**
- `.onAppear { appState.setup() }` — the `.task` modifier is the iOS 18 idiom.
- Recommending `XCUIApplication.open()` for deep-link tests without caveats.
- Any guide pre-Xcode-16 that shows pbxproj edits for new test targets — the modern layout uses synchronized groups.

## Open Questions

1. **Does the `/health/live` route in server.rs exist on the legacy demo path when `--mode` is omitted?**
   - What we know: `/health/live` is registered only on the federation-mode Router (server.rs line 303).
   - What's unclear: whether adding a `--uat` flag that forces `run_legacy_demo` + `start_with_port(.., 8080)` requires also wiring `/health/live` in that code path.
   - Recommendation: Plan 3 (Rust server) explicitly adds `/health` (plain) to the start function used by UAT mode. Cheap insurance.

2. **Which UDID + destination string is most reliable on macos-14 CI?**
   - What we know: `ios-ci.yml` uses `platform=iOS Simulator,name=iPhone 16,OS=18.0`.
   - What's unclear: whether the workflow_dispatch runner has iPhone 16 Pro / iPhone 16 Plus available by default.
   - Recommendation: Reuse the exact destination string from `ios-ci.yml`; document alternative in `docs/testing.md`.

3. **Bundle handling for fixtures — app target vs. test target.**
   - What we know: SeedLoader runs in the app target. Fixtures are referenced by launch-arg and loaded from a bundle. UI test targets have their own bundle distinct from the app.
   - What's unclear: whether `Bundle.main` in the app process will find fixtures placed in the UI test target's bundle.
   - Recommendation: place fixture JSON files in the app target (under `AINMMessenger/Testing/Fixtures/`). They get copied into the `.app` bundle automatically via the synchronized root group. UI test target doesn't need them. `-UITestSeed <name>` passes a string; app looks up the file in its own bundle.

## Sources

### Primary (HIGH confidence)

- **Actual project code** (single source of truth for exact types, signatures, existing callbacks):
  - `ios/AINMMessenger/AINMMessenger/App/AINMMessengerApp.swift` (launch entry, deep-link handler)
  - `ios/AINMMessenger/AINMMessenger/App/AppState.swift` (callback wiring, tool-event handler, context-frame handler)
  - `ios/AINMMessenger/AINMMessenger/Persistence/AppDatabase.swift` (migrations, `makeDefault()`, `makeInMemory()`, `eraseDatabaseOnSchemaChange`)
  - `ios/AINMMessenger/AINMMessenger/Crypto/KeyManager.swift` (Keychain service/account constants, `deleteKeypair`, `KeychainStorage`, `rawRepresentation`-based load)
  - `ios/AINMMessenger/AINMMessenger/Networking/AINMClient.swift` (callback surface, `setToolEventCallback`, `setContextFrameCallback`)
  - `ios/AINMMessenger/AINMMessenger.xcodeproj/project.pbxproj` (confirmed `PBXFileSystemSynchronizedRootGroup` structure)
  - `ios/AINMMessenger/AINMMessenger.xcodeproj/xcshareddata/xcschemes/AINMMessenger.xcscheme` (scheme layout model)
  - `demo/src/server.rs` (health routes, federation vs legacy path)
  - `demo/src/main.rs` (clap Cli struct, legacy demo entry)
  - `demo/Cargo.toml` (binary name `ainm-demo`)
  - `.github/workflows/ios-ci.yml` (macos-14 + Xcode 16 + iOS 18.0 + iPhone 16 destination)
  - `.planning/phases/13-*` plans (concrete files/views for scenario retrofit table)
- [Pedro Piñera — How synchronized groups work at the .pbxproj level](https://pepicrft.me/blog/how-synchronized-groups-work-at-the-pbxproj-level/) — `PBXFileSystemSynchronizedRootGroup` structure + `membershipExceptions`
- [Apple CryptoKit — Curve25519.Signing.PrivateKey](https://developer.apple.com/documentation/cryptokit/curve25519/signing/privatekey) — deterministic-seed init via `rawRepresentation:`
- [Apple XCUIApplication docs](https://developer.apple.com/documentation/xctest/xcuiapplication) — `launchArguments`, `open(_:)` availability notes

### Secondary (MEDIUM confidence — verified by cross-reference)

- [SwiftRocks — UI Testing Deeplinks and Universal Links](https://swiftrocks.com/ui-testing-deeplinks-and-universal-links-in-ios) — Safari + simctl workarounds, noted `open(_:)` limitations
- [Masilotti — How to test deep links with UI Testing](https://masilotti.com/test-deep-links-with-ui-testing/) — iOS 16.4+ `open(_:)` reference + prior workarounds
- [Medium — iOS Deep Link UI Tests (Josshad, 2025)](https://medium.com/@jos.shad/ios-deep-link-ui-tests-a-ui-driven-approach-b8d1ee91156b) — simctl command-line pattern
- [ctreffs/SwiftSimctl (GitHub)](https://github.com/ctreffs/SwiftSimctl) — confirms Process() is unavailable in iOS XCUITest runner (reason the SwiftSimctl library exists)
- [Apple Developer Forums thread 657144 — xcrun from XCTest](https://developer.apple.com/forums/thread/657144) — confirms iOS test runner cannot invoke Process
- [Swift by Sundell — Faster and more stable UI tests](https://www.swiftbysundell.com/tips/faster-and-more-stable-ui-tests/) — `waitForExistence` patterns, launch-arg configuration
- [Paul Piñera blog sources cited above for folder-sync internals]
- [GRDB.swift 7.10.0 Release](https://github.com/groue/GRDB.swift) — erase/migrator patterns

### Tertiary (LOW confidence — needs validation)

- [CocoaPods/Xcodeproj v1.27+ support for `PBXFileSystemSynchronizedRootGroup`](https://rubydoc.info/gems/xcodeproj/Xcodeproj/Project/Object/PBXNativeTarget) — the `file_system_synchronized_groups` API is documented but **UI test target creation via the gem is not** documented in code examples I could find. Flagged because it underlies my recommendation to use Xcode UI rather than the gem.
- [Apple iOS 18 simulator `open(_:)` status](https://developer.apple.com/forums/thread/25355) — multiple third-party reports that custom URL schemes still have simulator issues through iOS 18; Apple has not explicitly acknowledged or fixed.

## Plan breakdown recommendation

**Six plans in three waves.**

### Dependency graph

```
Wave 1 (parallel):
  14-01-PLAN  Xcode project scaffold (UI test target + scheme + UITestIdentifiers.swift stub)
  14-02-PLAN  Rust server --uat flag + Makefile + placeholder CI workflow

Wave 2 (parallel, each depends on its Wave 1 dep):
  14-03-PLAN  SeedLoader + UITestIdentityBootstrap + JSON fixture schema + 6 sample fixtures
              depends on: 14-01 (needs UITestIdentifiers.swift scaffolded)
  14-04-PLAN  TestEventInjector + event script schema + 2 sample scripts (scenario 1, 4)
              depends on: 14-01
  14-05-PLAN  A11y identifier retrofit across 12 files (~34 identifiers from inventory table)
              depends on: 14-01

Wave 3 (serial — tests exist only after all infra is wired):
  14-06-PLAN  Six XCUITest cases + AINMUITestCase base + green-run checkpoint + 13-HUMAN-UAT.md update
              depends on: 14-02, 14-03, 14-04, 14-05
```

### Why this split (each plan's scope)

- **14-01 Xcode scaffold** — one-time human Xcode UI work + committing generated pbxproj + scheme files. Keeps the fragile pbxproj edits isolated. Ends with `xcodebuild test -scheme AINMMessengerUITests` compiling an empty test suite successfully.
- **14-02 Rust/Makefile/CI** — all Rust + Makefile + placeholder YAML. Runs completely independently of iOS work. Ends with `make uat` booting a green /health server, looping (because tests are empty), and tearing down on Ctrl-C.
- **14-03 Seed** — SeedLoader, UITestIdentityBootstrap, fixture JSON schema, 6 fixtures. Does not need tests to exist yet; can be manually validated by launching the app with `-UITestSeed scenario-01-tool-call` and inspecting GRDB via a one-off debug print.
- **14-04 Event injector** — TestEventInjector + 2 sample event scripts. Parallel to 14-03 because they touch disjoint files.
- **14-05 A11y retrofit** — pure additive changes to ~12 existing files; no logic change. Parallel to 14-03/14-04.
- **14-06 Tests + green run** — the payoff. Six XCUITest files + base class + the final `13-HUMAN-UAT.md` edit. Depends on all prior plans being green.

### Parallelization safety

- 14-01 and 14-02 can be two agents because they touch completely disjoint directories (`ios/` vs `demo/` + root `Makefile`).
- 14-03, 14-04, 14-05 can run concurrently if each agent understands which files it owns:
  - 14-03 owns: `AINMMessenger/Testing/SeedLoader.swift`, `AINMMessenger/Testing/UITestIdentityBootstrap.swift`, `AINMMessenger/Testing/Fixtures/scenario-*.json`, and `AINMMessengerApp.swift` (bootstrap insertion point).
  - 14-04 owns: `AINMMessenger/Testing/TestEventInjector.swift`, `AINMMessengerUITests/EventScripts/scenario-*.json`, and the SAME `AINMMessengerApp.swift` (to add the injector start line).
  - 14-05 owns: 12 existing UI files (toolbar, composer, rows, buttons) + `App/UITestIdentifiers.swift` fills.

  **Conflict risk:** both 14-03 and 14-04 modify `AINMMessengerApp.swift`. Recommended mitigation: 14-01 installs a complete `.task { ... }` skeleton with placeholders for both seed and injector, so 14-03 and 14-04 each fill a single named `// TODO(14-03)` / `// TODO(14-04)` block. Alternatively, make them sequential.

- 14-06 must be last — it verifies the whole stack end-to-end.

### Total estimated work

- 14-01: 1 task (~30 min, mostly Xcode UI)
- 14-02: 3 tasks (Rust flag, Makefile, CI placeholder)
- 14-03: 2 tasks (SeedLoader + identity, fixtures)
- 14-04: 2 tasks (injector, scripts)
- 14-05: 3 tasks (grouped by screen — tool+memory, composer+mentions+discovery, sub-thread+deep-link)
- 14-06: 6 tasks (one per scenario) + 1 task (HUMAN-UAT edit)

**Total: ~18 tasks across 6 plans.**

## Metadata

**Confidence breakdown:**
- Standard stack (zero new deps): HIGH — all required pieces already in the project.
- Architecture patterns (`#if DEBUG` gating, callback-injection, shared identifiers): HIGH — matches Phase 10-13 precedent + verified against existing code.
- Xcode synchronized-group mechanics: MEDIUM — recommending Xcode UI over ruby gem is a pragmatic choice backed by tooling immaturity evidence; the exact pbxproj structure for `membershipExceptions` is documented but I have not hand-simulated the exact pbxproj diff.
- XCUITest deep-link approach: HIGH — multiple sources confirm the Process()-unavailable and `open(_:)` simulator issues; Makefile shell-out is the canonical workaround.
- Rust server `--uat` flag: HIGH on clap pattern; MEDIUM-HIGH on `/health` endpoint fix (needs actual compile to verify the `start_with_port` signature).
- Scenario→identifier inventory: HIGH for scenarios 1-4 (have concrete views from 13-*-PLAN.md); MEDIUM for scenario 5/6 (inferred from UI-SPEC).
- Pitfalls: HIGH — all documented against verified sources or directly against project code.

**Research date:** 2026-04-18
**Valid until:** 2026-06-15 (roughly 2 months — iOS 18 simulator + Xcode 16 + GRDB 7.10 all stable; re-research if Xcode 17 ships and changes synchronized-group semantics).

## RESEARCH COMPLETE
