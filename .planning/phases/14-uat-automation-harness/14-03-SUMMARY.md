---
phase: 14-uat-automation-harness
plan: 03
subsystem: ios-seed-bootstrap
tags:
  - grdb
  - cryptokit
  - ed25519
  - keychain
  - fixtures
  - swift-6
  - debug-gated
dependency-graph:
  requires:
    - 14-01 (UITestIdentifiers.swift, TODO(14-03) anchors in AINMMessengerApp.swift)
    - phase-10..13 complete (ThreadRecord / ParticipantRecord / MessageRecord /
      ToolCallRecord / MemoryFactRecord / AgentCardRecord schemas)
    - iOS 18 / GRDB 7.10 / CryptoKit Curve25519 (project-wide stack)
  provides:
    - SeedLoader.preparePristineDatabase() — wipes ainm.sqlite + -wal + -shm
      BEFORE AppDatabase.makeDefault() opens a pool
    - SeedLoader.loadFixtureIfRequested(fixture:into:) — decodes a bundled
      scenario-NN-*.json and inserts every row in one dbPool.write transaction
    - UITestIdentityBootstrap.resetIdentity() — default Keychain path for app launch
    - UITestIdentityBootstrap.resetIdentity(storage:) — DI path for tests
    - UITestIdentityBootstrap.uatSeedBytes / identityAddress / deterministicPublicKeyBase64
    - ScenarioFixture Codable schema covering six record types with
      per-type .asRecord() GRDB adapters
    - Six scenario-NN-*.json fixtures bundled at AINMMessenger.app root
    - docs/testing.md "Fixture JSON schema" section documenting the top-level
      shape + fixture <-> scenario <-> event-script cross-reference
  affects:
    - AINMMessengerApp.swift .task closure — TODO(14-03) blocks replaced with
      live SeedLoader.preparePristineDatabase + UITestIdentityBootstrap.resetIdentity
      (pre-setup) and SeedLoader.loadFixtureIfRequested (post-setup)
tech-stack:
  added: []
  patterns:
    - "#if DEBUG gating for dev-only hooks (SeedLoader, UITestIdentityBootstrap,
       ScenarioFixture, SeedLoaderIntegrationTests, UITestIdentityBootstrapTests)"
    - "Curve25519.Signing.PrivateKey(rawRepresentation:) with a fixed 32-byte scalar
       seed for cross-run-reproducible Ed25519 keypairs"
    - "GRDB single-transaction seed pattern: fixture decode followed by one
       dbPool.write { db in ... } closure that inserts every row so
       ValueObservation fires once with the final state (Pitfall 6 avoidance)"
    - "Option A GRDB reset (14-RESEARCH.md): FileManager.removeItem on ainm.sqlite
       + -wal + -shm BEFORE pool open, not dbPool.erase(), to avoid the observation
       attachment race"
    - "Storage-parameterized reset API: resetIdentity(storage:) takes any
       KeyManager.KeyStorage so unit tests inject KeyManager.InMemoryStorage and
       bypass iOS Simulator's errSecMissingEntitlement (-34018) on SecItemDelete"
    - "Swift Testing parametrized @Test with arguments: tuple array for table-driven
       per-fixture row-count assertions"
    - "nonisolated static let on @MainActor suites so Swift Testing macro expansion
       (which runs on the runner context) can access arguments"
key-files:
  created:
    - ios/AINMMessenger/AINMMessenger/Testing/UITestIdentityBootstrap.swift
    - ios/AINMMessenger/AINMMessenger/Testing/SeedLoader.swift
    - ios/AINMMessenger/AINMMessenger/Testing/ScenarioFixture.swift
    - ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-01-tool-call.json
    - ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-02-mentions.json
    - ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-03-agent-discovery.json
    - ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-04-memory.json
    - ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-05-sub-thread.json
    - ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-06-deep-link.json
    - ios/AINMMessenger/AINMMessengerTests/CryptoTests/UITestIdentityBootstrapTests.swift
    - ios/AINMMessenger/AINMMessengerTests/PersistenceTests/ScenarioFixtureTests.swift
    - ios/AINMMessenger/AINMMessengerTests/PersistenceTests/SeedLoaderIntegrationTests.swift
  modified:
    - ios/AINMMessenger/AINMMessenger/App/AINMMessengerApp.swift
    - docs/testing.md
decisions:
  - "Fixture bundle path: AINMMessenger/Testing/Fixtures/ inside the app target's
     synchronized root group — Xcode auto-copies JSON resources to the .app root,
     so Bundle.main.url(forResource:withExtension:) succeeds without any pbxproj
     changes. Validated by inspecting /tmp/uat-plan14-03-DerivedData/.../AINMMessenger.app
     and finding all six scenario-NN-*.json files at the bundle root."
  - "ScenarioFixture schema is flat-per-record: every fixture struct
     (FixtureThread, FixtureParticipant, FixtureMessage, FixtureToolCall,
     FixtureArchivalFact, FixtureAgentCard) mirrors its GRDB record's column
     set one-to-one. Only deviation is FixtureParticipant.displayName —
     fixture-local metadata for @-mention autocomplete seeding that does not
     persist to ParticipantRecord (which has no displayName column)."
  - "No custom ISO8601 parsing layer — ThreadRecord.createdAt is stored as the
     RFC 3339 String verbatim. Fixtures specify timestamps as ISO8601 strings
     with millisecond precision and UTC 'Z' suffix (e.g.
     '2026-04-18T10:00:00.000Z'). SeedLoader passes them through unchanged."
  - "KeychainStorage accessed directly (no new installPrivateKey helper added to
     KeyManager): the nested KeyManager.KeychainStorage struct is non-isolated
     and Sendable — callable from any context — and Xcode build confirmed it
     compiles from UITestIdentityBootstrap without touching the KeyManager actor.
     Plan 14-03's fallback suggestion to add installPrivateKey(rawRepresentation:)
     to KeyManager was NOT needed."
  - "resetIdentity split into storage-less default (Keychain-backed) and
     storage-parameterized DI variant. Rationale: iOS Simulator unit-test hosts
     return errSecMissingEntitlement (-34018) from SecItemDelete for the
     com.ainm.messenger service; end-to-end Keychain coverage must live in 14-06's
     XCUITest pass against the real app host. Unit tests use
     KeyManager.InMemoryStorage for deterministic per-process isolation."
  - "CryptoKit Curve25519.Signing signatures are NOT byte-deterministic — Apple's
     implementation uses a randomized nonce for side-channel hardening, contrary
     to RFC 8032's deterministic scheme. The original plan asked for a signature-
     equality assertion; that was infeasible. Replaced with a sign+verify round-trip
     assertion (keypair.publicKey.isValidSignature(signature, for: payload)) which
     proves the public key corresponds to the installed private key without
     assuming byte-identical signatures."
  - "Swift Testing .serialized suite trait NOT used: after switching to
     InMemoryStorage injection, all tests are hermetic — no shared Keychain
     state to race on. Parallel execution works correctly. The earlier
     .serialized annotation was a diagnostic; removed in the final revision."
metrics:
  duration_minutes: 55
  tasks_completed: 4
  files_created: 12
  files_modified: 2
  commits: 4
  completed_date: "2026-04-18"
---

# Phase 14 Plan 03: Seed Bootstrap Summary

Hermetic UAT seed bootstrap delivered: a `SeedLoader` that wipes GRDB and inserts
scenario-specific fixture rows, a `UITestIdentityBootstrap` that installs a
deterministic `uat-user@localhost` Ed25519 keypair from a fixed 32-byte seed, a
`ScenarioFixture` Codable schema with per-record adapters, six scenario fixtures
(one per Phase 13 acceptance test), and the AINMMessengerApp wiring that runs
all of them BEFORE `AppState.setup()` so the UI observes only seeded state on first
paint.

## Tasks Completed

| Task | Name                                                                          | Commit    | Files                                                                                                                                                                                                                        |
| ---- | ----------------------------------------------------------------------------- | --------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1    | UITestIdentityBootstrap.swift with deterministic fixed-seed Ed25519 keypair   | `8385d82` | `Testing/UITestIdentityBootstrap.swift`, `AINMMessengerTests/CryptoTests/UITestIdentityBootstrapTests.swift`                                                                                                                 |
| 2    | ScenarioFixture.swift schema + SeedLoader.swift (DB wipe + fixture load)      | `619abd1` | `Testing/ScenarioFixture.swift`, `Testing/SeedLoader.swift`, `AINMMessengerTests/PersistenceTests/ScenarioFixtureTests.swift`                                                                                               |
| 3    | Six fixture JSON files + docs/testing.md schema section                       | `b68911c` | `Testing/Fixtures/scenario-{01-tool-call,02-mentions,03-agent-discovery,04-memory,05-sub-thread,06-deep-link}.json`, `docs/testing.md`                                                                                         |
| 4    | Replace TODO(14-03) blocks in AINMMessengerApp.swift; SeedLoaderIntegrationTests | `149b028` | `App/AINMMessengerApp.swift`, `Testing/UITestIdentityBootstrap.swift`, `AINMMessengerTests/PersistenceTests/SeedLoaderIntegrationTests.swift`                                                                               |

## Deterministic public key for `uat-user@localhost`

The fixed 32-byte seed
(`0x55 0x41 0x54 0x5F 0x55 0x53 0x45 0x52 0x40 0x4C 0x4F 0x43 0x41 0x4C 0x48 0x4F`
`0x53 0x54 0x00 0x00 0x00 0x00 0x00 0x00 0xDE 0xAD 0xBE 0xEF 0xFE 0xED 0xFA 0xCE`)
produces the following reproducible public key. Plan 14-06 can assert signatures
against this value without recomputing the keypair at the test site.

| Encoding   | Value                                                            |
| ---------- | ---------------------------------------------------------------- |
| base64     | `tFIlTgLccxHnpbEMNTDC39a8iCzbP1+kJW2Ii5WJWQI=`                   |
| base64url  | `tFIlTgLccxHnpbEMNTDC39a8iCzbP1-kJW2Ii5WJWQI`                    |
| hex        | `b452254e02dc7311e7a5b10c3530c2dfd6bc882cdb3f5fa4256d888b95895902` |

## Fixture row-count table

Each row is asserted by
`SeedLoaderIntegrationTests.test_seedLoader_loadsEachFixtureWithExpectedRowCounts`
(six-argument parametrized test) against a throwaway `AppDatabase.makeInMemory()` database.

| Fixture                            | threads | participants | messages | toolCalls | archival | agentCards |
| ---------------------------------- | ------- | ------------ | -------- | --------- | -------- | ---------- |
| `scenario-01-tool-call`            | 1       | 2            | 1        | 0         | 0        | 0          |
| `scenario-02-mentions`             | 1       | 3            | 1        | 0         | 0        | 0          |
| `scenario-03-agent-discovery`      | 0       | 0            | 0        | 0         | 0        | 2          |
| `scenario-04-memory`               | 1       | 2            | 0        | 0         | 4        | 0          |
| `scenario-05-sub-thread`           | 1       | 2            | 1        | 0         | 0        | 0          |
| `scenario-06-deep-link`            | 1       | 2            | 1        | 0         | 0        | 0          |

## Record-column ↔ fixture-field mapping (authoritative)

Below is the exact GRDB column layout discovered during fixture authoring. Plans
14-04 and 14-06 must reference these names verbatim; any drift here breaks
signed envelope assembly and deep-link navigation.

| Record              | Table name   | Columns (as seen in fixtures)                                                                                                                          |
| ------------------- | ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `ThreadRecord`      | `thread`     | `id`, `title?`, `createdAt`, `createdBy`, `parentThreadId?`, `parentMessageId?`, `hasUnread`                                                           |
| `ParticipantRecord` | `participant`| `threadId`, `identity`, `type`, `joinedAt`, `role`  (composite PK: `threadId` + `identity`)                                                            |
| `MessageRecord`     | `message`    | `id`, `threadId`, `fromAddress`, `timestamp`, `type`, `contentJson`, `signature`, `serverTimestamp?`                                                   |
| `ToolCallRecord`    | `toolCall`   | `callId` (PK), `threadId`, `name`, `status`, `input?`, `resultSummary?`, `durationMs?`, `errorMessage?`, `startedAt`, `completedAt?`                    |
| `MemoryFactRecord`  | `memoryFact` | `factId` (PK), `key`, `value`, `tags?`, `storedBy`, `storedAt`  (no `agentId` column — fixture authors use `storedBy` for the storing agent's AINM address) |
| `AgentCardRecord`   | `agentCard`  | `url` (PK), `identity`, `cardJson`, `etag?`, `fetchedAt`  (PK is the card URL, NOT the agent identity; `cardJson` contains the full serialized AgentCard) |

## Timestamp format

Every fixture uses RFC 3339 / ISO 8601 strings with millisecond precision and the
UTC `Z` suffix:

```
"2026-04-18T10:00:00.000Z"
```

`SeedLoader.loadFixtureIfRequested(fixture:into:)` passes these strings to the
GRDB records verbatim — no `Date` conversion, no `ISO8601DateFormatter`. Downstream
tests comparing timestamps can rely on string-equality against these literals.

## AINMMessengerApp.swift wiring

The `.task` closure now reads (trimmed to the essentials):

```swift
.task {
    #if DEBUG
    let args = ProcessInfo.processInfo.arguments
    let seedFixture: String? = /* -UITestSeed parsing */
    let eventScript: String? = /* -UITestEvents parsing */

    // Step 1 + 2: Plan 14-03
    if seedFixture != nil {
        try? SeedLoader.preparePristineDatabase()
        try? await UITestIdentityBootstrap.resetIdentity()
    }
    #endif

    try? appState.setup()

    #if DEBUG
    // Step 3: Plan 14-03
    if let seedFixture {
        try? await SeedLoader.loadFixtureIfRequested(fixture: seedFixture, into: appState)
    }

    // Step 4: Plan 14-04 owns this anchor
    // TODO(14-04): if let eventScript { ... }
    _ = eventScript
    #endif

    bootstrapComplete = true
}
```

Order is strict and matches the 14-RESEARCH §"Exact insertion point in
AINMMessengerApp.swift" contract: wipe DB file → reset identity → `setup()` opens
a fresh pool and AINMClient reads the deterministic keypair → load fixture rows
into the open pool → (14-04) start event injector → render readiness marker.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] CryptoKit Curve25519 signatures are not byte-deterministic**
- **Found during:** Task 4, running SeedLoaderIntegrationTests the first time
- **Issue:** The plan's `test_deterministicKey_signsDeterministically` asserted
  that signing the same payload twice with the deterministic key yields the
  same signature bytes (RFC 8032 behavior). Apple's CryptoKit Curve25519.Signing
  implementation instead uses a randomized nonce for side-channel hardening —
  two calls to `privateKey.signature(for: payload)` produce different bytes.
- **Fix:** Replaced the signature-equality assertion with a sign+verify
  round-trip check: `keypair.publicKey.isValidSignature(sig, for: payload)`.
  This still proves the public key matches the installed private key (the
  property Plan 14-06 actually needs) without making a false claim about
  byte-determinism.
- **Files modified:** `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/SeedLoaderIntegrationTests.swift`
- **Commit:** `149b028`

**2. [Rule 3 - Blocking] iOS Simulator unit-test host cannot touch the system Keychain**
- **Found during:** Task 4, first run of `test_identityBootstrap_regeneratesDeterministicKey`
  against the iPhone 17 / iOS 26.2 simulator
- **Issue:** `KeyManager.KeychainStorage().delete()` returned
  `errSecMissingEntitlement` (-34018) when invoked from the unit-test host.
  This is a documented simulator limitation: unit tests attached to an app
  host do not inherit the app's keychain-access-group entitlement, so
  `SecItemDelete` fails. The plan's acceptance criterion required this test
  to pass.
- **Fix:** Added a storage-parameterized `resetIdentity(storage:)` entry point
  to `UITestIdentityBootstrap` that accepts any `KeyManager.KeyStorage`. The
  production no-arg `resetIdentity()` keeps the Keychain default; the
  integration test injects `KeyManager.InMemoryStorage()` for hermetic per-
  process isolation. End-to-end Keychain coverage now lives in 14-06's XCUITest
  pass which runs against the real app host (entitlement-inheriting).
- **Files modified:** `ios/AINMMessenger/AINMMessenger/Testing/UITestIdentityBootstrap.swift`,
  `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/SeedLoaderIntegrationTests.swift`
- **Commit:** `149b028`

**3. [Rule 3 - Blocking] Environment mismatch: no iOS 18.0 simulator runtime installed**
- **Found during:** Task 1, running `xcodebuild test -destination 'platform=iOS Simulator,name=iPhone 16,OS=18.0'`
- **Issue:** The plan's automated verify commands pin `iPhone 16 / OS=18.0` but
  only `iOS 26.2 (iPhone 17)` is available on this worktree's execution host.
  Same gap noted in 14-01-SUMMARY.
- **Fix:** Substituted `iPhone 17 / OS=26.2` as the destination for all test
  runs in this plan. Ran `xcodebuild test -only-testing:AINMMessengerTests/{UITestIdentityBootstrapTests,ScenarioFixtureTests,SeedLoaderIntegrationTests}`
  and the full `-only-testing:AINMMessengerTests` suite — all passed. CI runners
  will still use the iOS 18.0 runtime per 14-02's Makefile/CI template; executor
  environment divergence only.
- **Files modified:** (none — verification only)
- **Commit:** (none)

### Auth Gates

None encountered — this plan is pure iOS code + bundled JSON fixtures with no
external service, CLI login, or API key requirement.

## Known Stubs

None. The plan's `TODO(14-04)` anchor in `AINMMessengerApp.swift` is NOT a
stub introduced by 14-03 — it's the reserved extension point installed by
14-01 and owned by Plan 14-04. Left verbatim.

## Verification Results

| Check                                                                                          | Result |
| ---------------------------------------------------------------------------------------------- | ------ |
| `ios/AINMMessenger/AINMMessenger/Testing/UITestIdentityBootstrap.swift` exists + `#if DEBUG`   | PASS   |
| `uatSeedBytes.count == 32` via `test_uatSeedBytes_exactly32()`                                 | PASS   |
| `deterministicPublicKeyBase64` stable across calls                                             | PASS   |
| Seed ASCII prefix equals `UAT_USER@LOCALHOST`                                                  | PASS   |
| `identityAddress == "uat-user@localhost"`                                                      | PASS   |
| `ScenarioFixture.swift` has 6 fixture sub-structs, all Decodable + Sendable                    | PASS   |
| `SeedLoader.swift` has `preparePristineDatabase()`, `loadFixtureIfRequested(fixture:into:)`, uses `dbPool.write` | PASS |
| `SeedLoader.preparePristineDatabase()` idempotent on missing files                             | PASS   |
| `SeedLoader.SeedError.fixtureNotFound` thrown for unknown fixture name                         | PASS   |
| Minimum-viable ScenarioFixture literal decodes                                                 | PASS   |
| All 6 fixture JSONs present under `AINMMessenger/Testing/Fixtures/`                            | PASS   |
| All 6 fixtures parse as JSON with top-level keys version/scenario/identity/threads/participants/messages/toolCalls/archivalFacts/agentCards | PASS |
| Every fixture `identity == "uat-user@localhost"`                                               | PASS   |
| scenario-02 contains `"remy@localhost"`                                                        | PASS   |
| scenario-03 has 2 agent cards + 0 threads                                                      | PASS   |
| scenario-04 has 4 archival facts all with `storedBy: "helper@localhost"`                       | PASS   |
| scenario-05 contains `thread-parent-01` + `msg-parent-01`                                      | PASS   |
| scenario-06 contains `thread-deeplink-01`                                                      | PASS   |
| `docs/testing.md` contains `Fixture JSON schema` section                                       | PASS   |
| All 6 fixtures present in `/tmp/uat-plan14-03-DerivedData/.../AINMMessenger.app/` bundle root | PASS   |
| `ScenarioFixtureTests.test_decodesScenario01IfAuthored()` passes (post-Task-3)                 | PASS   |
| `AINMMessengerApp.swift` does NOT contain `TODO(14-03)`                                        | PASS (0 hits) |
| `AINMMessengerApp.swift` contains `TODO(14-04)` exactly once                                   | PASS (1 hit) |
| `AINMMessengerApp.swift` contains `SeedLoader.preparePristineDatabase`                         | PASS   |
| `AINMMessengerApp.swift` contains `UITestIdentityBootstrap.resetIdentity`                      | PASS   |
| `AINMMessengerApp.swift` contains `SeedLoader.loadFixtureIfRequested`                          | PASS   |
| `AINMMessengerApp.swift` does NOT contain `_ = seedFixture`                                    | PASS (0 hits) |
| `AINMMessengerApp.swift` contains `_ = eventScript` (until 14-04)                              | PASS (1 hit) |
| `xcodebuild test -only-testing:AINMMessengerTests/SeedLoaderIntegrationTests` exits 0          | PASS   |
| `test_seedLoader_loadsEachFixtureWithExpectedRowCounts` passes across all 6 fixtures           | PASS (6/6) |
| `test_identityBootstrap_regeneratesDeterministicKey` passes                                    | PASS   |
| `xcodebuild test -only-testing:AINMMessengerTests` (full suite) exits 0                        | PASS   |
| `xcodebuild build-for-testing -scheme AINMMessengerUITests` exits 0                            | PASS   |
| `xcodebuild build -configuration Release -scheme AINMMessenger` exits 0                        | PASS   |

## Unblocks

- **Plan 14-04 (TestEventInjector):** now has a guaranteed GRDB-seeded state
  to push events into. `scenario-01-tool-call.json` seeds `thread-01` with
  `helper@localhost`, and 14-04's event script will use `callId: "call-01"`
  (documented in `docs/testing.md` fixture table). `scenario-04-memory.json`
  seeds the thread; 14-04 pushes the context frame.
- **Plan 14-06 (XCUITest cases):** can launch with `-UITestSeed scenario-NN-*`,
  assert the seeded thread via `UITestID.ConversationList.cell(threadId:
  "thread-01")` / `"thread-parent-01"` / `"thread-deeplink-01"`, reference
  `helper@localhost` and `remy@localhost` by seeded identity, and (where
  needed) verify signatures against the public key recorded above.
- The deterministic keypair and seed fixtures guarantee every UAT run starts
  from the same cryptographic + database state — cross-run signature drift
  will never be the source of a flaky test.

## Self-Check: PASSED

- [x] `ios/AINMMessenger/AINMMessenger/Testing/UITestIdentityBootstrap.swift` (new)
- [x] `ios/AINMMessenger/AINMMessenger/Testing/SeedLoader.swift` (new)
- [x] `ios/AINMMessenger/AINMMessenger/Testing/ScenarioFixture.swift` (new)
- [x] `ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-01-tool-call.json` (new)
- [x] `ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-02-mentions.json` (new)
- [x] `ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-03-agent-discovery.json` (new)
- [x] `ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-04-memory.json` (new)
- [x] `ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-05-sub-thread.json` (new)
- [x] `ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-06-deep-link.json` (new)
- [x] `ios/AINMMessenger/AINMMessengerTests/CryptoTests/UITestIdentityBootstrapTests.swift` (new)
- [x] `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/ScenarioFixtureTests.swift` (new)
- [x] `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/SeedLoaderIntegrationTests.swift` (new)
- [x] `ios/AINMMessenger/AINMMessenger/App/AINMMessengerApp.swift` (modified — TODO(14-03) blocks replaced)
- [x] `docs/testing.md` (modified — Fixture JSON schema section appended)
- [x] Commit `8385d82` exists in git log
- [x] Commit `619abd1` exists in git log
- [x] Commit `b68911c` exists in git log
- [x] Commit `149b028` exists in git log
