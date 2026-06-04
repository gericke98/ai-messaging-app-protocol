# Agent Discovery / Add-Agent UX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a user add a home-server agent by `name@host`/bare-name/card-URL, save it as a per-account contact, and start a thread that actually routes to that agent — turning the existing non-functional Agent Discovery UI shell into a working feature.

**Architecture:** Evolve the existing `UI/AgentDiscovery/` views in place (per the approved decision) rather than build a parallel `UI/Agents/`. Add a per-account GRDB `savedAgent` table + `SavedAgentStore`; fix the `AgentCardService` URL path/scheme so home-server agents resolve; add `AppState.resolveAgent`/`saveAgent`/`removeAgent`/`savedAgents` and generalize `startAssistantThread()` → `startThread(withAgent:)`; repoint the list at saved agents and wire "Start Conversation" through agent routing; seed the hosted assistant as a default saved agent; keep the existing `AgentDiscoveryUITests` green by seeding the new table in fixtures.

**Tech Stack:** Swift 6 / SwiftUI / GRDB 7 (iOS); Rust / Axum (server regression test). `xcodebuild` for iOS build+test, `cargo nextest`/`cargo test` for server.

---

## Context the engineer needs (read once)

**This is an EVOLVE-IN-PLACE plan.** A full Agent Discovery UI already exists but does NOT do what we want:
- `ios/AINMMessenger/AINMMessenger/UI/AgentDiscovery/AgentDiscoveryView.swift` — lists cards from the **shared** `agentCard` ETag cache via `AgentCardService.loadCachedCards()`. We will repoint it at a **per-account `savedAgent` table**.
- `ios/AINMMessenger/AINMMessenger/UI/AgentDiscovery/AgentCardDetailView.swift` — "Start Conversation" calls `appState.createThread(...)`, which does **NOT** stamp `agentAddress`, so messages never route to the agent. We will change it to `appState.startThread(withAgent:)`.
- `ios/AINMMessenger/AINMMessenger/Networking/AgentCardService.swift` — `fetchCardByAddress(_:)` builds `…/.well-known/ainm/agents/{local}/agent-card.json` (extra `agents/`) and hardcodes `https://`. The server's real route is `/.well-known/ainm/{agent_name}/agent-card.json` (no `agents/`). We fix the client.
- Entry point already wired: `ConversationListView` toolbar `NavigationLink` (`person.2.badge.gearshape`, accessibilityLabel "Discover agents") → `AgentDiscoveryView`. KEEP IT.
- `UITestID.AgentDiscovery.*` identifiers exist and are used by `AINMMessengerUITests/AgentDiscoveryUITests.swift`. KEEP all identifiers and the entry point so that UITest still passes.

**Established patterns to follow:**
- DB tables are camelCase (`agentCard`, `toolCall`, `memoryFact`). Use table name `savedAgent`, columns `address`/`displayName`/`cardJson`/`addedAt`. Migrations are registered in `AppDatabase.migrate()` (last is `v7_add_thread_agentAddress`); add `v8_create_saved_agents`.
- Per-account isolation is automatic: `AppDatabase.make(forAccount:)` opens an address-keyed DB file; each account has its own `savedAgent` rows.
- `AppState` is `@MainActor @Observable final class`. `serverURL` is a WebSocket URL like `ws://localhost:8080` or `wss://host:443`. `connectedIdentity` is `"username@host"`. `ISO8601DateFormatter.ainm` is the standard timestamp formatter used across records.
- `startAssistantThread()` (AppState ~line 624) is the exact template for `startThread(withAgent:)`: it ALWAYS does a local GRDB write with `agentAddress` set (even when connected) — the first message then stamps `to` via `sendMessage(envelope:)`. Mirror this; do NOT route agent-thread creation through `client.createThread`.
- `AppState.assistantAddress(forServerURL:)` (nonisolated static, ~line 542) derives `assistant@<host>`.

**Tooling commands (exact):**
- iOS build: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build`
- iOS unit tests (single class): append `test -only-testing:AINMMessengerTests/<ClassName>` to the same invocation (replace `build`).
- iOS full unit suite: `... -only-testing:AINMMessengerTests test`
- Server single test: `cd server && cargo test --test agent_card_route -- --nocapture` (or `cargo nextest run --test agent_card_route`).
- Server full suite: `cd server && cargo test` (two pre-existing failures in `tests/integration.rs` — `test_graph_traversal`, `test_compilation_dedup_updates_confidence` — are unrelated and expected).
- All git via `rtk proxy git ...`. Commit directly to `main` (project pattern). Do NOT push (outward-facing — user confirms separately).

---

## File Structure

**Create:**
- `ios/AINMMessenger/AINMMessenger/Persistence/SavedAgentRecord.swift` — GRDB row for a saved agent contact.
- `ios/AINMMessenger/AINMMessenger/Persistence/SavedAgentStore.swift` — CRUD actor over the active account's DB.
- `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/SavedAgentStoreTests.swift` — store + per-account isolation tests.
- `ios/AINMMessenger/AINMMessengerTests/NetworkingTests/AgentCardURLTests.swift` — canonical URL + scheme mapping tests.
- `ios/AINMMessenger/AINMMessengerTests/AppTests/AgentResolveTests.swift` — `parseAgentInput` tests (bare/addr/url, host guard).
- `server/tests/agent_card_route.rs` — server regression test locking the canonical agent-card route.

**Modify:**
- `ios/AINMMessenger/AINMMessenger/Persistence/AppDatabase.swift` — add migration `v8_create_saved_agents`.
- `ios/AINMMessenger/AINMMessenger/Networking/AgentCardService.swift` — canonical URL + scheme helpers; `fetchCard(forAddress:scheme:)`; remove buggy `fetchCardByAddress`.
- `ios/AINMMessenger/AINMMessenger/App/AppState.swift` — `ResolvedAgent`, `AgentResolveError`, `parseAgentInput`, `resolveAgent`, `savedAgents`, `loadSavedAgents`, `saveAgent`, `removeAgent`, `startThread(withAgent:)`, `startAssistantThread` (thin), `seedDefaultAssistantIfNeeded`, `placeholderAssistantCard`; call seed in `connect()`.
- `ios/AINMMessenger/AINMMessenger/UI/AgentDiscovery/AgentDiscoveryView.swift` — back the list with `appState.savedAgents`; add via `resolveAgent` → preview → `saveAgent`; swipe-to-remove → `removeAgent`.
- `ios/AINMMessenger/AINMMessenger/UI/AgentDiscovery/AgentCardDetailView.swift` — "Start Conversation" → `startThread(withAgent:)`.
- `ios/AINMMessenger/AINMMessenger/Testing/ScenarioFixture.swift` — add `savedAgents: [FixtureSavedAgent]`.
- `ios/AINMMessenger/AINMMessenger/Testing/SeedLoader.swift` — seed `savedAgent` rows.
- `ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-03-agent-discovery.json` — add `savedAgents` (helper + assistant).
- `ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-0{1,2,4,5,6}-*.json` — add `"savedAgents": []`.

**Intentional deviations from spec §2 (recorded):** Spec listed new `UI/Agents/AgentsListView.swift` + `AddAgentView.swift`; per the approved evolve-in-place decision we instead evolve the existing `UI/AgentDiscovery/` views (add UI folded into the existing view's add section). Spec said table `saved_agents`; we use camelCase `savedAgent` to match the existing schema convention. Spec said no server route change — confirmed (route already canonical).

---

## Slice 1 — Per-account saved-agents persistence

### Task 1: `savedAgent` migration + `SavedAgentRecord`

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/Persistence/SavedAgentRecord.swift`
- Modify: `ios/AINMMessenger/AINMMessenger/Persistence/AppDatabase.swift` (add migration after `v7_add_thread_agentAddress`, before `try migrator.migrate(dbPool)`)
- Test: `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/SavedAgentStoreTests.swift` (record-only test in this task; store CRUD added in Task 2)

- [ ] **Step 1: Create `SavedAgentRecord.swift`**

```swift
import GRDB
import Foundation

/// GRDB record for a per-account saved agent contact.
/// Maps to the "savedAgent" table created in the v8 migration. The user's chosen
/// agent contacts live in the ACTIVE account's database (multi-account isolation gives
/// each account its own DB file). Distinct from the ETag-keyed `agentCard` HTTP cache:
/// a saved agent is a deliberate contact whose card snapshot lets the list render offline.
struct SavedAgentRecord: Codable, Sendable, FetchableRecord, PersistableRecord, Identifiable {
    static let databaseTableName = "savedAgent"

    let address: String        // Primary key -- canonical name@host
    let displayName: String    // From the card's `name`, for list display
    let cardJson: String       // Serialized AgentCard snapshot (renders offline)
    let addedAt: String        // RFC 3339

    var id: String { address }

    /// Decode the cached AgentCard snapshot from JSON, if it is well-formed.
    func agentCard() -> AgentCard? {
        guard let data = cardJson.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(AgentCard.self, from: data)
    }
}
```

- [ ] **Step 2: Add the migration in `AppDatabase.migrate()`**

Insert immediately after the `v7_add_thread_agentAddress` migration block and before `try migrator.migrate(dbPool)`:

```swift
        migrator.registerMigration("v8_create_saved_agents") { db in
            try db.create(table: "savedAgent") { t in
                t.primaryKey("address", .text).notNull()   // canonical name@host
                t.column("displayName", .text).notNull()
                t.column("cardJson", .text).notNull()
                t.column("addedAt", .text).notNull()       // RFC 3339
            }
        }
```

- [ ] **Step 3: Write the failing test** (`SavedAgentStoreTests.swift`)

```swift
import XCTest
import GRDB
@testable import AINMMessenger

final class SavedAgentStoreTests: XCTestCase {
    func testRecordPersistsAndDecodesCard() throws {
        let db = try AppDatabase.makeInMemory()
        let cardJson = "{\"name\":\"Helper\",\"description\":\"d\",\"url\":\"\",\"version\":\"1.0\",\"protocol_version\":\"0.1.0\",\"capabilities\":{\"streaming\":true,\"push_notifications\":false},\"skills\":[]}"
        let record = SavedAgentRecord(
            address: "helper@localhost",
            displayName: "Helper",
            cardJson: cardJson,
            addedAt: "2026-06-04T00:00:00.000Z"
        )
        try db.dbPool.write { db in try record.save(db) }

        let fetched = try db.dbPool.read { db in
            try SavedAgentRecord.fetchOne(db, key: "helper@localhost")
        }
        XCTAssertEqual(fetched?.displayName, "Helper")
        XCTAssertEqual(fetched?.agentCard()?.name, "Helper")
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd -only-testing:AINMMessengerTests/SavedAgentStoreTests test`
Expected: PASS (1 test).

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Persistence/SavedAgentRecord.swift ios/AINMMessenger/AINMMessenger/Persistence/AppDatabase.swift ios/AINMMessenger/AINMMessengerTests/PersistenceTests/SavedAgentStoreTests.swift
rtk proxy git commit -m "feat(agent-discovery): add per-account savedAgent table + record"
```

---

### Task 2: `SavedAgentStore` CRUD + per-account isolation

**Files:**
- Create: `ios/AINMMessenger/AINMMessenger/Persistence/SavedAgentStore.swift`
- Test: extend `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/SavedAgentStoreTests.swift`

- [ ] **Step 1: Create `SavedAgentStore.swift`**

```swift
import GRDB
import Foundation

/// CRUD over the active account's saved-agent contacts. A thin actor around the
/// account's `DatabasePool` (mirrors the access style of `AgentCardService`).
actor SavedAgentStore {
    private let dbPool: DatabasePool

    init(dbPool: DatabasePool) {
        self.dbPool = dbPool
    }

    /// Insert or update (upsert on the `address` primary key) a saved agent.
    func add(_ record: SavedAgentRecord) async throws {
        try await dbPool.write { db in try record.save(db) }
    }

    /// Remove a saved agent by address. No-op if absent.
    func remove(address: String) async throws {
        _ = try await dbPool.write { db in
            try SavedAgentRecord.deleteOne(db, key: address)
        }
    }

    /// All saved agents, oldest first (stable list order).
    func all() async throws -> [SavedAgentRecord] {
        try await dbPool.read { db in
            try SavedAgentRecord.order(Column("addedAt").asc).fetchAll(db)
        }
    }

    /// Whether an agent with this address is already saved.
    func contains(address: String) async throws -> Bool {
        try await dbPool.read { db in
            try SavedAgentRecord.fetchOne(db, key: address) != nil
        }
    }
}
```

- [ ] **Step 2: Write the failing tests** (append to `SavedAgentStoreTests.swift`)

```swift
    func testAddRemoveAllContains() async throws {
        let db = try AppDatabase.makeInMemory()
        let store = SavedAgentStore(dbPool: db.dbPool)
        let rec = SavedAgentRecord(address: "a@localhost", displayName: "A",
                                   cardJson: "{}", addedAt: "2026-06-04T00:00:00.000Z")
        try await store.add(rec)
        let isThere = try await store.contains(address: "a@localhost")
        XCTAssertTrue(isThere)
        XCTAssertEqual(try await store.all().count, 1)
        try await store.remove(address: "a@localhost")
        XCTAssertFalse(try await store.contains(address: "a@localhost"))
        XCTAssertEqual(try await store.all().count, 0)
    }

    func testAddIsUpsertNoDuplicate() async throws {
        let db = try AppDatabase.makeInMemory()
        let store = SavedAgentStore(dbPool: db.dbPool)
        try await store.add(SavedAgentRecord(address: "a@localhost", displayName: "A",
                                             cardJson: "{}", addedAt: "2026-06-04T00:00:00.000Z"))
        try await store.add(SavedAgentRecord(address: "a@localhost", displayName: "A2",
                                             cardJson: "{}", addedAt: "2026-06-04T00:00:01.000Z"))
        let all = try await store.all()
        XCTAssertEqual(all.count, 1)
        XCTAssertEqual(all.first?.displayName, "A2")
    }

    func testPerAccountIsolation() async throws {
        // Two distinct DBs model two accounts; a row in A is absent in B.
        let dbA = try AppDatabase.makeInMemory()
        let dbB = try AppDatabase.makeInMemory()
        let storeA = SavedAgentStore(dbPool: dbA.dbPool)
        let storeB = SavedAgentStore(dbPool: dbB.dbPool)
        try await storeA.add(SavedAgentRecord(address: "a@localhost", displayName: "A",
                                              cardJson: "{}", addedAt: "2026-06-04T00:00:00.000Z"))
        XCTAssertTrue(try await storeA.contains(address: "a@localhost"))
        XCTAssertFalse(try await storeB.contains(address: "a@localhost"))
    }
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `xcodebuild ... -only-testing:AINMMessengerTests/SavedAgentStoreTests test`
Expected: PASS (4 tests).

- [ ] **Step 4: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Persistence/SavedAgentStore.swift ios/AINMMessenger/AINMMessengerTests/PersistenceTests/SavedAgentStoreTests.swift
rtk proxy git commit -m "feat(agent-discovery): SavedAgentStore CRUD + per-account isolation"
```

---

## Slice 2 — Agent-card seam fix (load-bearing)

### Task 3: Canonical card URL + scheme in `AgentCardService`

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/Networking/AgentCardService.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/NetworkingTests/AgentCardURLTests.swift`

- [ ] **Step 1: Write the failing test** (`AgentCardURLTests.swift`)

```swift
import XCTest
@testable import AINMMessenger

/// Locks the client/server agent-card contract: the client must build the SAME path the
/// server serves (`/.well-known/ainm/{name}/agent-card.json` — no `agents/` segment) and
/// must map the WebSocket scheme to HTTP correctly (ws→http for the localhost dev server).
final class AgentCardURLTests: XCTestCase {
    func testCanonicalCardURLMatchesServerRoute() {
        XCTAssertEqual(
            AgentCardService.canonicalCardURL(forAddress: "assistant@localhost", scheme: "http"),
            "http://localhost/.well-known/ainm/assistant/agent-card.json"
        )
        XCTAssertEqual(
            AgentCardService.canonicalCardURL(forAddress: "claude-code@ainm.example.com", scheme: "https"),
            "https://ainm.example.com/.well-known/ainm/claude-code/agent-card.json"
        )
    }

    func testCanonicalCardURLRejectsMalformed() {
        XCTAssertNil(AgentCardService.canonicalCardURL(forAddress: "nohost", scheme: "https"))
        XCTAssertNil(AgentCardService.canonicalCardURL(forAddress: "@localhost", scheme: "https"))
        XCTAssertNil(AgentCardService.canonicalCardURL(forAddress: "name@", scheme: "https"))
    }

    func testHttpSchemeFromServerURL() {
        XCTAssertEqual(AgentCardService.httpScheme(forServerURL: "ws://localhost:8080"), "http")
        XCTAssertEqual(AgentCardService.httpScheme(forServerURL: "wss://ainm.example.com:443"), "https")
        XCTAssertEqual(AgentCardService.httpScheme(forServerURL: "https://x"), "https")
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `xcodebuild ... -only-testing:AINMMessengerTests/AgentCardURLTests test`
Expected: FAIL — `canonicalCardURL`/`httpScheme` don't exist yet (compile error).

- [ ] **Step 3: Add the helpers and fix the fetch path in `AgentCardService.swift`**

Add these two static helpers inside `actor AgentCardService` (e.g. just above `fetchCard(agentCardURL:identity:)`):

```swift
    /// Map a WebSocket server URL scheme to the HTTP scheme used for agent-card fetches.
    /// `ws → http`, `wss → https`. Defaults to `https` for anything else.
    static func httpScheme(forServerURL serverURL: String) -> String {
        let lower = serverURL.lowercased()
        if lower.hasPrefix("ws://") { return "http" }
        if lower.hasPrefix("wss://") { return "https" }
        return "https"
    }

    /// Build the canonical agent-card URL for an AINM address, matching the SERVER route
    /// `/.well-known/ainm/{name}/agent-card.json`. Returns nil for a malformed address.
    static func canonicalCardURL(forAddress address: String, scheme: String) -> String? {
        let parts = address.components(separatedBy: "@")
        guard parts.count == 2, !parts[0].isEmpty,
              let domain = parts.last, !domain.isEmpty else { return nil }
        return "\(scheme)://\(domain)/.well-known/ainm/\(parts[0])/agent-card.json"
    }
```

Replace the buggy `fetchCardByAddress(_:)` method entirely with a scheme-aware version:

```swift
    /// Fetch an Agent Card by AINM address using the canonical home-server route.
    /// `scheme` is derived from the active account's `serverURL` (see `httpScheme(forServerURL:)`).
    func fetchCard(forAddress address: String, scheme: String) async throws -> AgentCard {
        guard let cardURL = AgentCardService.canonicalCardURL(forAddress: address, scheme: scheme) else {
            throw AgentCardError.invalidAddress
        }
        return try await fetchCard(agentCardURL: cardURL, identity: address)
    }
```

(Delete the old `fetchCardByAddress(_:)` — its only caller, `AgentDiscoveryView.addAgent`, is replaced in Task 8.)

- [ ] **Step 4: Run test to verify it passes**

Run: `xcodebuild ... -only-testing:AINMMessengerTests/AgentCardURLTests test`
Expected: PASS (3 tests). Note: the view that called the deleted method is fixed in Task 8 — if the project fails to compile here because `AgentDiscoveryView` still references `fetchCardByAddress`, that is expected; this task's TEST target compiles independently of the app target only if SwiftPM/Xcode allows. If the whole module must compile, temporarily leave `fetchCardByAddress` as a thin deprecated shim that calls `fetchCard(forAddress:scheme: "https")` and remove it in Task 8. Prefer the shim to keep the build green between tasks:

```swift
    @available(*, deprecated, message: "Use fetchCard(forAddress:scheme:) — removed in Task 8")
    func fetchCardByAddress(_ address: String) async throws -> AgentCard {
        try await fetchCard(forAddress: address, scheme: "https")
    }
```

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Networking/AgentCardService.swift ios/AINMMessenger/AINMMessengerTests/NetworkingTests/AgentCardURLTests.swift
rtk proxy git commit -m "fix(agent-discovery): canonical agent-card URL path + scheme from serverURL"
```

---

### Task 4: Server agent-card route regression test

**Files:**
- Create: `server/tests/agent_card_route.rs`

Context: `server` is binary-only (`mod server` in `main.rs`), so this black-boxes the compiled binary exactly like `server/tests/ws_conversation_e2e.rs`. The agent-card handler serves `state.agent_card` when no live agent is registered and needs no DB, so this test does NOT require Postgres. The binary is spawned in `--mode human` with the repo test certs; `/health/ready` returns 200 even with no DB.

- [ ] **Step 1: Write the test** (`server/tests/agent_card_route.rs`)

```rust
//! Regression test locking the client/server agent-card contract: the canonical route
//! `/.well-known/ainm/{agent_name}/agent-card.json` (no `agents/` segment) serves the
//! hosted agent card. Mirrors the RC-5.1 envelope-decode regression style. Black-boxes the
//! compiled binary because `ServerState`/`agent_card_handler` are binary-only (B-8 split).

use std::process::Stdio;
use std::time::Duration;

/// Bind 127.0.0.1:0, read the assigned port, drop the listener so the server can take it.
async fn free_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}

#[tokio::test]
async fn canonical_agent_card_route_serves_hosted_card() {
    let port = free_port().await;
    let certs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("certs");

    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_ainm-server"))
        .args([
            "--mode", "human",
            "--port", &port.to_string(),
            "--identity", "localhost",
            "--cert", certs.join("server-a.pem").to_str().unwrap(),
            "--key", certs.join("server-a-key.pem").to_str().unwrap(),
            "--ca-cert", certs.join("ca.pem").to_str().unwrap(),
        ])
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn ainm-server");

    // Poll /health/ready until 200 (~15s budget).
    let http = reqwest::Client::new();
    let ready_url = format!("http://127.0.0.1:{port}/health/ready");
    let mut ready = false;
    for _ in 0..150 {
        if let Ok(resp) = http.get(&ready_url).send().await {
            if resp.status().is_success() {
                ready = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ready, "server did not become ready");

    // Canonical route — NO `agents/` segment.
    let card_url = format!("http://127.0.0.1:{port}/.well-known/ainm/assistant/agent-card.json");
    let resp = http.get(&card_url).send().await.expect("GET agent-card");
    assert_eq!(resp.status().as_u16(), 200, "canonical agent-card route must return 200");

    let body: serde_json::Value = resp.json().await.expect("agent-card JSON body");
    assert!(body.get("name").and_then(|v| v.as_str()).is_some(),
            "agent card must carry a `name` field; got {body}");

    // A bogus `agents/`-prefixed path must NOT serve the card (locks the no-`agents/` contract).
    let bogus = format!("http://127.0.0.1:{port}/.well-known/ainm/agents/assistant/agent-card.json");
    let bogus_resp = http.get(&bogus).send().await.expect("GET bogus");
    assert_ne!(bogus_resp.status().as_u16(), 200,
               "the legacy `agents/`-prefixed path must not resolve");

    child.start_kill().ok();
}
```

- [ ] **Step 2: Run the test**

Run: `cd server && cargo test --test agent_card_route -- --nocapture`
Expected: PASS (1 test). If the binary in `--mode human` requires a DB to reach readiness in this environment, fall back to setting `AINM_DATABASE__URL` from `TEST_DATABASE_URL` the way `ws_conversation_e2e.rs` does and skip gracefully when absent — but first try without DB, since `/health/ready` is OK in in-memory mode.

- [ ] **Step 3: Commit**

```bash
rtk proxy git add server/tests/agent_card_route.rs
rtk proxy git commit -m "test(agent-discovery): lock canonical agent-card route contract"
```

---

## Slice 3 — AppState resolve / save / start-thread

### Task 5: `parseAgentInput` (pure) + tests

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/App/AppState.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/AppTests/AgentResolveTests.swift`

- [ ] **Step 1: Write the failing test** (`AgentResolveTests.swift`)

```swift
import XCTest
@testable import AINMMessenger

final class AgentResolveTests: XCTestCase {
    func testBareNameDefaultsToServerHost() {
        let p = AppState.parseAgentInput("claude-code", serverHost: "localhost")
        XCTAssertEqual(p?.address, "claude-code@localhost")
        XCTAssertNil(p?.directCardURL)
    }

    func testFullAddressPassesThrough() {
        let p = AppState.parseAgentInput("helper@localhost", serverHost: "localhost")
        XCTAssertEqual(p?.address, "helper@localhost")
        XCTAssertNil(p?.directCardURL)
    }

    func testCardURLDerivesAddressAndKeepsURL() {
        let url = "http://localhost/.well-known/ainm/helper/agent-card.json"
        let p = AppState.parseAgentInput(url, serverHost: "localhost")
        XCTAssertEqual(p?.address, "helper@localhost")
        XCTAssertEqual(p?.directCardURL, url)
    }

    func testWhitespaceTrimmedAndEmptyRejected() {
        XCTAssertEqual(AppState.parseAgentInput("  helper@localhost  ", serverHost: "localhost")?.address,
                       "helper@localhost")
        XCTAssertNil(AppState.parseAgentInput("   ", serverHost: "localhost"))
        XCTAssertNil(AppState.parseAgentInput("name@", serverHost: "localhost"))
        XCTAssertNil(AppState.parseAgentInput("@host", serverHost: "localhost"))
    }

    func testMalformedCardURLRejected() {
        XCTAssertNil(AppState.parseAgentInput("http://localhost/wrong/path.json", serverHost: "localhost"))
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `xcodebuild ... -only-testing:AINMMessengerTests/AgentResolveTests test`
Expected: FAIL — `parseAgentInput` / `ParsedAgentInput` undefined.

- [ ] **Step 3: Add `ParsedAgentInput` + `parseAgentInput` to `AppState.swift`**

Add near `assistantAddress(forServerURL:)` (both pure/nonisolated static so they're unit-testable without the MainActor):

```swift
    /// Result of parsing user "add agent" input into a canonical target.
    struct ParsedAgentInput: Equatable, Sendable {
        let address: String          // canonical name@host
        let directCardURL: String?   // non-nil only when the user pasted a card URL
    }

    /// Parse "add agent" input. Accepts a bare name (defaults host to `serverHost`), a
    /// `name@host` address, or a card URL `scheme://host/.well-known/ainm/{name}/agent-card.json`.
    /// Returns nil for malformed input. Pure — no host guard here (that lives in `resolveAgent`).
    nonisolated static func parseAgentInput(_ raw: String, serverHost: String) -> ParsedAgentInput? {
        let input = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !input.isEmpty else { return nil }

        let lower = input.lowercased()
        if lower.hasPrefix("http://") || lower.hasPrefix("https://") {
            guard let url = URL(string: input), let host = url.host, !host.isEmpty else { return nil }
            let comps = url.path.split(separator: "/").map(String.init)
            // Expect [".well-known", "ainm", "{name}", "agent-card.json"]
            guard comps.count >= 4,
                  comps[comps.count - 4] == ".well-known",
                  comps[comps.count - 3] == "ainm",
                  comps[comps.count - 1] == "agent-card.json" else { return nil }
            let name = comps[comps.count - 2]
            guard !name.isEmpty else { return nil }
            return ParsedAgentInput(address: "\(name)@\(host)", directCardURL: input)
        }

        if input.contains("@") {
            let parts = input.components(separatedBy: "@")
            guard parts.count == 2, !parts[0].isEmpty, !parts[1].isEmpty else { return nil }
            return ParsedAgentInput(address: input, directCardURL: nil)
        }

        guard !serverHost.isEmpty else { return nil }
        return ParsedAgentInput(address: "\(input)@\(serverHost)", directCardURL: nil)
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `xcodebuild ... -only-testing:AINMMessengerTests/AgentResolveTests test`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/App/AppState.swift ios/AINMMessenger/AINMMessengerTests/AppTests/AgentResolveTests.swift
rtk proxy git commit -m "feat(agent-discovery): pure parseAgentInput (bare/address/card-URL)"
```

---

### Task 6: `resolveAgent` + `ResolvedAgent` + `AgentResolveError`

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/App/AppState.swift`

No new automated test (the network fetch is exercised live in Task 12 and the pure parse/host-guard logic is covered in Task 5). Keep `resolveAgent` thin so all branching logic is in the tested `parseAgentInput`.

- [ ] **Step 1: Add the result/error types + `resolveAgent` to `AppState.swift`**

Place the types near `RecoveryOutcome` and the method in the agent section of AppState:

```swift
    /// A resolved, fetched agent ready to preview/save.
    struct ResolvedAgent: Sendable {
        let address: String
        let card: AgentCard
    }

    /// Why an "add agent" attempt failed, with user-facing messaging.
    enum AgentResolveError: Error, Sendable {
        case malformedInput
        case remoteUnsupported(host: String)
        case unreachable

        var userMessage: String {
            switch self {
            case .malformedInput:
                return "Enter an agent address like name@host or paste its agent-card URL."
            case .remoteUnsupported(let host):
                return "Only agents on your server (@\(host)) are supported yet."
            case .unreachable:
                return "Couldn't reach that agent — make sure it's connected to your server and the address is correct."
            }
        }
    }

    /// Resolve "add agent" input to a fetched card: parse → home-server guard → fetch.
    /// Home-server-only (v1): the address host must equal the active account's server host.
    func resolveAgent(_ rawInput: String) async -> Result<ResolvedAgent, AgentResolveError> {
        guard let host = URL(string: serverURL)?.host, !host.isEmpty else {
            return .failure(.malformedInput)
        }
        guard let parsed = AppState.parseAgentInput(rawInput, serverHost: host) else {
            return .failure(.malformedInput)
        }
        let addrHost = parsed.address.components(separatedBy: "@").last ?? ""
        guard addrHost == host else { return .failure(.remoteUnsupported(host: host)) }
        guard let db = appDatabase else { return .failure(.unreachable) }

        let service = AgentCardService(appDatabase: db)
        do {
            let card: AgentCard
            if let cardURL = parsed.directCardURL {
                card = try await service.fetchCard(agentCardURL: cardURL, identity: parsed.address)
            } else {
                let scheme = AgentCardService.httpScheme(forServerURL: serverURL)
                card = try await service.fetchCard(forAddress: parsed.address, scheme: scheme)
            }
            return .success(ResolvedAgent(address: parsed.address, card: card))
        } catch {
            return .failure(.unreachable)
        }
    }
```

- [ ] **Step 2: Verify it compiles** (build the test target)

Run: `xcodebuild ... -only-testing:AINMMessengerTests/AgentResolveTests test`
Expected: PASS (module compiles; existing 5 tests still pass).

- [ ] **Step 3: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/App/AppState.swift
rtk proxy git commit -m "feat(agent-discovery): AppState.resolveAgent with home-server guard"
```

---

### Task 7: `savedAgents` state + `startThread(withAgent:)` + default seed

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/App/AppState.swift`
- Test: `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/SavedAgentStoreTests.swift` (add an AppState-level start-thread test)

- [ ] **Step 1: Add observable state + CRUD wrappers to `AppState.swift`**

Add a stored property near the other `@Observable` vars (e.g. by `selectedThreadId`):

```swift
    /// Saved agent contacts for the ACTIVE account (mirrors `SavedAgentStore`). Refreshed
    /// on bootstrap/connect, add, and remove. The agents list UI reads this directly.
    var savedAgents: [SavedAgentRecord] = []
```

Add these methods in the agent section:

```swift
    /// Reload `savedAgents` from the active account's store. Clears it when there is no DB.
    func loadSavedAgents() async {
        guard let db = appDatabase else { savedAgents = []; return }
        let store = SavedAgentStore(dbPool: db.dbPool)
        savedAgents = (try? await store.all()) ?? []
    }

    /// Persist a resolved agent as a contact, then refresh `savedAgents`. Upsert — saving an
    /// already-saved address updates it in place (no duplicate row).
    func saveAgent(address: String, card: AgentCard) async {
        guard let db = appDatabase else { return }
        let store = SavedAgentStore(dbPool: db.dbPool)
        let json = (try? JSONEncoder().encode(card)).flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
        let record = SavedAgentRecord(
            address: address,
            displayName: card.name,
            cardJson: json,
            addedAt: ISO8601DateFormatter.ainm.string(from: Date())
        )
        try? await store.add(record)
        await loadSavedAgents()
    }

    /// Remove a saved agent contact. Existing threads with that agent are untouched.
    func removeAgent(address: String) async {
        guard let db = appDatabase else { return }
        let store = SavedAgentStore(dbPool: db.dbPool)
        try? await store.remove(address: address)
        await loadSavedAgents()
    }

    /// Create + persist a thread addressed to `address`. Mirrors `startAssistantThread()`'s
    /// local-write path exactly (UUID id, `ISO8601DateFormatter.ainm` timestamp, direct
    /// `ThreadRecord.save`) but parameterized by agent address. The only behavioral effect is
    /// that `sendMessage(envelope:)` then stamps `to = agentAddress` for every message here.
    /// Returns the new threadId, or nil if there is no identity/database.
    @discardableResult
    func startThread(withAgent address: String) async -> String? {
        guard let identity = connectedIdentity else { return nil }
        guard let db = appDatabase else { return nil }
        let title = savedAgents.first(where: { $0.address == address })?.displayName ?? address
        let threadId = UUID().uuidString
        let timestamp = ISO8601DateFormatter.ainm.string(from: Date())
        let record = ThreadRecord(
            id: threadId,
            title: title,
            createdAt: timestamp,
            createdBy: identity,
            parentThreadId: nil,
            parentMessageId: nil,
            agentAddress: address
        )
        do {
            try await db.dbPool.write { db in try record.save(db) }
        } catch {
            return nil
        }
        return threadId
    }

    /// Seed the built-in `assistant@<host>` as a default saved agent when the active account
    /// has none, so the existing "Chat with Assistant" affordance maps onto the saved list.
    /// Idempotent (guarded on empty store). Best-effort card fetch; falls back to a placeholder.
    func seedDefaultAssistantIfNeeded() async {
        guard let db = appDatabase else { return }
        guard let assistant = AppState.assistantAddress(forServerURL: serverURL) else { return }
        let store = SavedAgentStore(dbPool: db.dbPool)
        let isEmpty = ((try? await store.all()) ?? []).isEmpty
        guard isEmpty else { await loadSavedAgents(); return }
        let card: AgentCard
        if case .success(let resolved) = await resolveAgent(assistant) {
            card = resolved.card
        } else {
            card = AppState.placeholderAssistantCard()
        }
        await saveAgent(address: assistant, card: card)
    }

    /// A minimal stand-in card for the assistant when its real card can't be fetched
    /// (e.g. offline at first connect). Lets the saved-agents row render with a name.
    nonisolated static func placeholderAssistantCard() -> AgentCard {
        AgentCard(
            name: "Assistant",
            description: "Built-in assistant on your home server.",
            url: "",
            version: "1.0",
            protocolVersion: "0.1.0",
            capabilities: AgentCard.A2ACapabilities(streaming: true, pushNotifications: false),
            skills: [],
            ainm: nil
        )
    }
```

- [ ] **Step 2: Make `startAssistantThread()` a thin caller**

Replace the body of the existing `startAssistantThread()` (AppState ~line 624) with:

```swift
    @discardableResult
    func startAssistantThread() async -> String? {
        guard let agent = AppState.assistantAddress(forServerURL: serverURL) else { return nil }
        return await startThread(withAgent: agent)
    }
```

- [ ] **Step 3: Seed + load on connect**

In `connect()`, immediately after `connectionState = .connected` (inside the `do` block, before the closing `}`), add:

```swift
            await seedDefaultAssistantIfNeeded()
            await loadSavedAgents()
```

- [ ] **Step 4: Write the failing test** (append to `SavedAgentStoreTests.swift`)

```swift
    @MainActor
    func testStartThreadStampsAgentAddress() async throws {
        let appState = AppState()
        try appState.setup()                       // makeDefault DB + client
        appState.connectedIdentity = "alice@localhost"
        let threadId = await appState.startThread(withAgent: "helper@localhost")
        let id = try XCTUnwrap(threadId)
        let db = try XCTUnwrap(appState.appDatabase)
        let fetched = try await db.dbPool.read { db in try ThreadRecord.fetchOne(db, key: id) }
        XCTAssertEqual(fetched?.agentAddress, "helper@localhost")
    }
```

Note: `setup()` uses `AppDatabase.makeDefault()` (Application Support). If a prior run's threads interfere, the assertion still holds because it fetches by the fresh UUID. `connectedIdentity` is settable (it is a plain `var` on AppState).

- [ ] **Step 5: Run tests to verify they pass**

Run: `xcodebuild ... -only-testing:AINMMessengerTests/SavedAgentStoreTests test`
Expected: PASS (5 tests now). Also build the whole test target to confirm AppState compiles.

- [ ] **Step 6: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/App/AppState.swift ios/AINMMessenger/AINMMessengerTests/PersistenceTests/SavedAgentStoreTests.swift
rtk proxy git commit -m "feat(agent-discovery): savedAgents state, startThread(withAgent:), default assistant seed"
```

---

## Slice 4 — UI evolve-in-place + fixture seeding

### Task 8: Repoint `AgentDiscoveryView` at saved agents + add/preview/remove

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/AgentDiscovery/AgentDiscoveryView.swift`
- Modify: `ios/AINMMessenger/AINMMessenger/Networking/AgentCardService.swift` (remove the Task-3 deprecation shim now that the last caller is gone)

- [ ] **Step 1: Rewrite `AgentDiscoveryView.swift`**

The list now reads `appState.savedAgents`; add resolves → previews → saves; rows support swipe-to-remove. Keep ALL existing `UITestID.AgentDiscovery.*` identifiers and the navigation into `AgentCardDetailView` so `AgentDiscoveryUITests` still passes.

```swift
import SwiftUI

/// Saved agents screen. Lists the active account's saved agent contacts, supports adding a
/// new agent by address or card URL (with a verified preview), starting a thread with one,
/// and removing one. Reads `appState.savedAgents` (per-account `SavedAgentStore`).
struct AgentDiscoveryView: View {
    @Bindable var appState: AppState

    @State private var searchText = ""
    @State private var addAgentInput = ""
    @State private var addAgentError: String?
    @State private var isResolving = false
    @State private var preview: AppState.ResolvedAgent?

    private var saved: [SavedAgentRecord] { appState.savedAgents }

    private var filtered: [SavedAgentRecord] {
        guard !searchText.isEmpty else { return saved }
        return saved.filter { rec in
            rec.displayName.localizedCaseInsensitiveContains(searchText) ||
            rec.address.localizedCaseInsensitiveContains(searchText)
        }
    }

    var body: some View {
        Group {
            if saved.isEmpty {
                emptyState
            } else {
                agentList
            }
        }
        .navigationTitle("Agents")
        .searchable(text: $searchText, prompt: "Search agents...")
        .accessibilityIdentifier(UITestID.AgentDiscovery.search)
        .task { await appState.loadSavedAgents() }
        .refreshable { await appState.loadSavedAgents() }
    }

    // MARK: - Empty state

    private var emptyState: some View {
        VStack(spacing: 16) {
            Image(systemName: "person.2.badge.gearshape")
                .font(.system(size: 48))
                .foregroundStyle(Color(.tertiaryLabel))
            Text("No agents yet")
                .font(.title2.bold())
            Text("Add an agent on your server by address or card URL")
                .font(.subheadline)
                .foregroundStyle(Color(.secondaryLabel))
                .multilineTextAlignment(.center)
            addAgentField
        }
        .padding(.horizontal, 32)
    }

    // MARK: - Add field + preview

    private var addAgentField: some View {
        VStack(spacing: 8) {
            TextField("agent@server.example  ·  or card URL", text: $addAgentInput)
                .textFieldStyle(.roundedBorder)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .keyboardType(.emailAddress)
                .accessibilityIdentifier(UITestID.AgentDiscovery.addByAddressField)

            if let error = addAgentError {
                Text(error).font(.caption).foregroundStyle(Color.red)
            }

            if let preview {
                VStack(alignment: .leading, spacing: 4) {
                    Text(preview.card.name).font(.headline)
                    Text(preview.address).font(.caption).foregroundStyle(.secondary)
                    Text(preview.card.description).font(.subheadline).foregroundStyle(.secondary)
                        .lineLimit(2)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(8)
                .background(Color(.tertiarySystemFill))
                .clipShape(RoundedRectangle(cornerRadius: 8))

                Button("Add \(preview.card.name)") {
                    Task { await confirmAdd(preview) }
                }
                .buttonStyle(.borderedProminent)
                .accessibilityIdentifier(UITestID.AgentDiscovery.addByAddressButton)
            } else {
                Button(isResolving ? "Looking up…" : "Look up") {
                    Task { await lookUp() }
                }
                .buttonStyle(.borderedProminent)
                .disabled(addAgentInput.isEmpty || isResolving)
                .accessibilityIdentifier(UITestID.AgentDiscovery.addByAddressButton)
            }
        }
    }

    // MARK: - List

    private var agentList: some View {
        List {
            ForEach(filtered) { rec in
                NavigationLink {
                    AgentCardDetailView(
                        identity: rec.address,
                        card: rec.agentCard() ?? AppState.placeholderAssistantCard(),
                        appState: appState
                    )
                } label: {
                    agentRow(rec)
                }
                .accessibilityIdentifier(UITestID.AgentDiscovery.listCell(agentId: rec.address))
            }
            .onDelete { offsets in
                let targets = offsets.map { filtered[$0].address }
                Task { for address in targets { await appState.removeAgent(address: address) } }
            }

            Section { addAgentField }
        }
        .listStyle(.plain)
    }

    private func agentRow(_ rec: SavedAgentRecord) -> some View {
        let card = rec.agentCard()
        return HStack(spacing: 12) {
            AvatarView(participantType: "agent", size: 44)
            VStack(alignment: .leading, spacing: 4) {
                Text(rec.displayName).font(.headline).lineLimit(1)
                Text(rec.address).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                if let card {
                    let tags = card.skills.flatMap { $0.tags ?? [] }
                    if !tags.isEmpty {
                        HStack(spacing: 6) {
                            ForEach(Array(tags.prefix(3)), id: \.self) { tag in
                                Text(tag)
                                    .font(.caption.bold())
                                    .padding(.horizontal, 8).padding(.vertical, 4)
                                    .background(Color(.tertiarySystemFill))
                                    .clipShape(Capsule())
                            }
                        }
                    }
                }
            }
        }
        .padding(.vertical, 4)
    }

    // MARK: - Actions

    private func lookUp() async {
        isResolving = true
        addAgentError = nil
        preview = nil
        let result = await appState.resolveAgent(addAgentInput)
        switch result {
        case .success(let resolved): preview = resolved
        case .failure(let err): addAgentError = err.userMessage
        }
        isResolving = false
    }

    private func confirmAdd(_ resolved: AppState.ResolvedAgent) async {
        await appState.saveAgent(address: resolved.address, card: resolved.card)
        addAgentInput = ""
        preview = nil
        addAgentError = nil
    }
}
```

- [ ] **Step 2: Remove the deprecation shim** added in Task 3 Step 4 (`fetchCardByAddress`), now that no caller remains. Confirm with a grep:

Run: `grep -rn "fetchCardByAddress" ios/AINMMessenger`
Expected: no matches.

- [ ] **Step 3: Build the app target**

Run: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd build`
Expected: BUILD SUCCEEDED.

- [ ] **Step 4: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/AgentDiscovery/AgentDiscoveryView.swift ios/AINMMessenger/AINMMessenger/Networking/AgentCardService.swift
rtk proxy git commit -m "feat(agent-discovery): saved-agents list with add/preview/remove"
```

---

### Task 9: `AgentCardDetailView` Start Conversation → `startThread(withAgent:)`

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/UI/AgentDiscovery/AgentCardDetailView.swift`

- [ ] **Step 1: Rewrite the `startConversationButton` action**

Replace the `Button { Task { ... } }` action body (which calls `appState.createThread(...)`) with one that routes to the agent via the persisted `agentAddress`:

```swift
    private var startConversationButton: some View {
        Button {
            Task {
                if let threadId = await appState.startThread(withAgent: identity) {
                    appState.selectedThreadId = threadId
                    dismiss()
                }
            }
        } label: {
            Text("Start Conversation")
                .frame(maxWidth: .infinity)
                .frame(height: 44)
        }
        .buttonStyle(.borderedProminent)
        .padding(.horizontal, 16)
        .padding(.bottom, 8)
        .accessibilityLabel("Start conversation with \(card.name)")
        .accessibilityIdentifier(UITestID.AgentDiscovery.detailStartConversation)
    }
```

(`identity` is the view's existing `let identity: String` property — the agent's address. Everything else in the view is unchanged.)

- [ ] **Step 2: Build the app target**

Run: `xcodebuild ... build`
Expected: BUILD SUCCEEDED.

- [ ] **Step 3: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/UI/AgentDiscovery/AgentCardDetailView.swift
rtk proxy git commit -m "fix(agent-discovery): Start Conversation routes to the agent address"
```

---

### Task 10: Seed `savedAgent` rows in test fixtures (keep UITest green)

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/Testing/ScenarioFixture.swift`
- Modify: `ios/AINMMessenger/AINMMessenger/Testing/SeedLoader.swift`
- Modify: `ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-03-agent-discovery.json`
- Modify: `ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-0{1,2,4,5,6}-*.json`

Context: `AgentDiscoveryUITests` seeds via `scenario-03` and expects a `helper@localhost` row + an `assistant@localhost` row. Now that the list reads `savedAgent`, the fixture must seed that table. `ScenarioFixture` is `Decodable` with a synthesized initializer, so a non-optional `savedAgents` array must be PRESENT in every fixture JSON (every fixture already carries an `agentCards` key — add `savedAgents` the same way).

- [ ] **Step 1: Add `FixtureSavedAgent` + property to `ScenarioFixture.swift`**

Add the struct near `FixtureAgentCard`:

```swift
// MARK: - Saved agent

struct FixtureSavedAgent: Decodable, Sendable {
    // Mirrors SavedAgentRecord (Persistence/SavedAgentRecord.swift).
    let address: String
    let displayName: String
    let cardJson: String
    let addedAt: String

    func asRecord() -> SavedAgentRecord {
        SavedAgentRecord(
            address: address,
            displayName: displayName,
            cardJson: cardJson,
            addedAt: addedAt
        )
    }
}
```

Add the property to the `ScenarioFixture` struct (next to `let agentCards: [FixtureAgentCard]`):

```swift
    let savedAgents: [FixtureSavedAgent]
```

- [ ] **Step 2: Seed the rows in `SeedLoader.swift`**

In the `dbPool.write { db in ... }` block (after the `for card in fixture.agentCards` loop), add:

```swift
            for agent in fixture.savedAgents {
                try agent.asRecord().insert(db)
            }
```

- [ ] **Step 3: Add `savedAgents` to `scenario-03-agent-discovery.json`**

Add a top-level `savedAgents` array (use the SAME `cardJson` strings already present in that file's `agentCards` entries so the detail view renders skills/badges). Insert after the `agentCards` array:

```json
  ,
  "savedAgents": [
    {
      "address": "helper@localhost",
      "displayName": "Helper",
      "cardJson": "{\"name\":\"Helper\",\"description\":\"General-purpose assistant agent with streaming, memory, and sub-thread support.\",\"url\":\"https://localhost/.well-known/ainm/helper/agent-card.json\",\"version\":\"1.0\",\"protocol_version\":\"0.1.0\",\"capabilities\":{\"streaming\":true,\"push_notifications\":false},\"skills\":[{\"id\":\"summarize\",\"name\":\"Summarize\",\"description\":\"Summarize long-form content into concise highlights.\",\"tags\":[\"text\",\"productivity\"]},{\"id\":\"draft-email\",\"name\":\"Draft Email\",\"description\":\"Compose professional email drafts from bullet-point intent.\",\"tags\":[\"email\",\"productivity\"]}],\"ainm\":{\"protocol_version\":\"0.1.0\",\"content_types\":[\"text\"],\"streaming\":{\"supported\":true,\"parallel_blocks\":false,\"preferred_chunk_size\":\"medium\"},\"threading\":{\"sub_threads\":true,\"max_concurrent_threads\":10},\"memory\":{\"archival_supported\":true,\"context_frame_preferred\":true},\"tools\":[]}}",
      "addedAt": "2026-04-18T10:00:00.000Z"
    },
    {
      "address": "assistant@localhost",
      "displayName": "Assistant",
      "cardJson": "{\"name\":\"Assistant\",\"description\":\"Focused coding-support agent. Answers technical questions and reviews diffs.\",\"url\":\"https://localhost/.well-known/ainm/assistant/agent-card.json\",\"version\":\"1.0\",\"protocol_version\":\"0.1.0\",\"capabilities\":{\"streaming\":true,\"push_notifications\":false},\"skills\":[{\"id\":\"code-review\",\"name\":\"Code Review\",\"description\":\"Review code changes for style and correctness.\",\"tags\":[\"code\"]},{\"id\":\"explain-code\",\"name\":\"Explain Code\",\"description\":\"Explain unfamiliar source code in plain language.\",\"tags\":[\"code\"]}],\"ainm\":{\"protocol_version\":\"0.1.0\",\"content_types\":[\"text\"],\"streaming\":{\"supported\":true,\"parallel_blocks\":false,\"preferred_chunk_size\":\"medium\"},\"threading\":{\"sub_threads\":false,\"max_concurrent_threads\":1},\"memory\":{\"archival_supported\":false,\"context_frame_preferred\":false},\"tools\":[]}}",
      "addedAt": "2026-04-18T10:00:01.000Z"
    }
  ]
```

(Place the leading `,` correctly: it joins the existing `agentCards` array to the new key. Validate the file is well-formed JSON with `python3 -m json.tool < <file>` after editing.)

- [ ] **Step 4: Add `"savedAgents": []` to the other five fixtures**

For each of `scenario-01-tool-call.json`, `scenario-02-mentions.json`, `scenario-04-memory.json`, `scenario-05-sub-thread.json`, `scenario-06-deep-link.json`, add a `"savedAgents": []` key alongside the existing `"agentCards": []`. Validate each with `python3 -m json.tool`.

- [ ] **Step 5: Build + run the unit suite (fixtures decode)**

Run: `xcodebuild ... -only-testing:AINMMessengerTests test`
Expected: BUILD SUCCEEDED; all unit tests pass (including `ScenarioFixtureTests` / `SeedLoaderIntegrationTests`, which now decode the new key).

- [ ] **Step 6: Run the Agent Discovery UITest**

Run: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd -only-testing:AINMMessengerUITests/AgentDiscoveryUITests test`
Expected: PASS — taps Helper row → detail (Skills header + Streaming badge) → Start Conversation → lands in a thread with the composer visible.

- [ ] **Step 7: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Testing/ScenarioFixture.swift ios/AINMMessenger/AINMMessenger/Testing/SeedLoader.swift ios/AINMMessenger/AINMMessenger/Testing/Fixtures/
rtk proxy git commit -m "test(agent-discovery): seed savedAgent rows in fixtures; keep UITest green"
```

---

## Slice 5 — Verify + live

### Task 11: Full suites green + DoD checklist

**Files:** none (verification only).

- [ ] **Step 1: iOS full unit suite**

Run: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd -only-testing:AINMMessengerTests test`
Expected: all pass. Capture the summary line.

- [ ] **Step 2: iOS UITests (Agent Discovery at minimum)**

Run: `xcodebuild ... -only-testing:AINMMessengerUITests/AgentDiscoveryUITests test`
Expected: pass.

- [ ] **Step 3: Server suite**

Run: `cd server && cargo test`
Expected: green EXCEPT the two known pre-existing failures (`tests/integration.rs::test_graph_traversal`, `::test_compilation_dedup_updates_confidence`). `agent_card_route` passes. (If `agent_card_route` needs a DB in this environment, it is allowed to skip with a printed message — note that in the commit/handoff.)

- [ ] **Step 4: DoD checklist (verify against spec §10)**

Confirm each, citing the implementing task:
- Add by `name@host`/bare-name/card-URL; foreign host rejected; unreachable shows clear error. (Tasks 5–6, 8)
- Added agents persist per-account, appear in the list, survive relaunch; removing deletes only the contact. (Tasks 1–2, 7–8)
- Tapping a saved agent starts a thread targeting its address; messages deliver (verified live Task 12). (Tasks 7, 9)
- Hosted assistant appears as a default saved agent, no regression to the chat flow. (Task 7)
- Client/server agent-card contract reconciled (path + scheme) + locked by server regression test; iOS unit + contract tests green; server suite green except the two known failures. (Tasks 3–4)
- v1 limitations documented: home-server-only, no client key-pinning, no directory. (Spec §1/§10 — already recorded.)

- [ ] **Step 5: Commit (if any verification touched files; otherwise skip)**

```bash
rtk proxy git commit --allow-empty -m "chore(agent-discovery): verify full suites green; DoD checklist passed"
```

---

### Task 12: MANUAL live add-agent (human)

**Files:** none (live validation; the dev session cannot self-verify a real WS round-trip).

- [ ] **Step 1:** Boot the server (mock or real LLM) and run the app in the simulator against it (`ws://localhost:8080`), signed in to an account.
- [ ] **Step 2:** Open the Agents screen. Confirm the hosted **Assistant** appears as a default saved agent.
- [ ] **Step 3:** Add an agent by address (`assistant@localhost`, or a connector agent's `name@localhost`). Confirm the preview renders (name/description), then **Add**.
- [ ] **Step 4:** Tap the saved agent → **Start Conversation** → send a message → confirm a reply arrives (validates `agentAddress` → `to` routing end-to-end).
- [ ] **Step 5:** Add a foreign host (`x@example.org`) → confirm the "Only agents on your server" error. Remove a saved agent → confirm it disappears but its thread remains.
- [ ] **Step 6:** Record the outcome in the project memory / handoff note.

---

## Self-Review (completed by plan author)

**Spec coverage:** §1 goal → Tasks 5–9; §3 data model → Tasks 1–2; §4 add/resolve/verify → Tasks 5–6, 8; §5 server seam → Tasks 3–4 (server route already canonical — client-only fix + regression test); §6 thread start + default seed → Task 7, 9; §7 UI → Tasks 8–9 (evolve-in-place per approved decision; add UI folded into the existing view); §8 error handling → Tasks 6, 8; §9 testing → Tasks 1–5, 10, server Task 4; §10 DoD → Task 11; §11 build order honored (persistence → seam fix → AppState → UI → tests → live).

**Placeholder scan:** none — every code step shows complete code; commands are exact.

**Type consistency:** `SavedAgentRecord(address:displayName:cardJson:addedAt:)` used identically in Tasks 1, 2, 7, 10. `AppState.ResolvedAgent`/`ParsedAgentInput`/`AgentResolveError` defined in Tasks 5–6, consumed in Tasks 7–8. `AgentCardService.canonicalCardURL(forAddress:scheme:)` / `httpScheme(forServerURL:)` / `fetchCard(forAddress:scheme:)` defined Task 3, consumed Task 6. `startThread(withAgent:)` defined Task 7, consumed Tasks 9, 7 (`startAssistantThread`). `AgentCard` memberwise init (`name:description:url:version:protocolVersion:capabilities:skills:ainm:`) matches `Models/AgentCard.swift`.
