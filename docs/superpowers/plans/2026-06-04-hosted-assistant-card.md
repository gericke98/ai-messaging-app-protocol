# Hosted Assistant Card Publishing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Serve a real, schema-conformant `AgentCard` for the hosted assistant at `/.well-known/ainm/assistant/agent-card.json`, and reconcile the iOS decoder to the canonical camelCase shape so the app renders the real card instead of the placeholder.

**Architecture:** A pure Rust builder (`build_assistant_card`) produces the canonical camelCase card from the hosted agent's address; `published_card_for` gains a hosted-assistant branch that serves it when an assistant is provisioned. The iOS `AgentCard` decoder drops its snake_case `CodingKeys` so Swift's synthesized coding uses the canonical camelCase keys; fixtures and one unit test are updated to match.

**Tech Stack:** Rust (Axum/serde_json, binary-only `server.rs`), Swift 6 (Codable, XCTest), GRDB.

**Conventions for this plan:**
- All git operations use `rtk proxy git` (never bare `git`). Commits go **directly to `main`** (project's established pattern).
- Server tests: `cd server && cargo test <filter>`. Two pre-existing failures are expected and unrelated: `test_graph_traversal`, `test_compilation_dedup_updates_confidence` in `tests/integration.rs`.
- iOS build/test: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test`. Target one class with `-only-testing:AINMMessengerTests/<Class>`.
- The Xcode project uses `PBXFileSystemSynchronizedRootGroup` — new files under the synchronized group are auto-added; no `.pbxproj` edit is needed.
- `server.rs` is **binary-only** (declared `mod server;` in `main.rs`, NOT in `lib.rs`). So `crate::` inside `server.rs` resolves against the **binary** crate.

---

## Task 1: Server `build_assistant_card` pure builder + unit test

Create a pure function that produces the canonical, camelCase, schema-valid assistant card. Schema validation against `schemas/agent-card.schema.json` is **not** done in this unit test: the `jsonschema` crate is not a server dependency, and the schema `$ref`s an external `common.schema.json` (awkward to resolve standalone). Instead assert every required key/value explicitly (the spec's sanctioned fallback in §8).

**Files:**
- Create: `server/src/agent_card.rs`
- Modify: `server/src/main.rs` (add `mod agent_card;`)

- [ ] **Step 1: Create the builder file with the function and a failing-style unit test**

Create `server/src/agent_card.rs`:

```rust
//! Canonical Agent Card builder for the hosted first-party assistant.
//!
//! Produces a schema-conformant (camelCase, A2A-compatible superset) `AgentCard`
//! JSON value from the assistant's AINM address. Pure: no DB, no async, no I/O.
//! The published JSON Schema (`schemas/agent-card.schema.json`) is authoritative;
//! keys here are camelCase to match it. `tools: []` for v1 — enumerating the live
//! MCP toolset in the card is a future enhancement.

use serde_json::{json, Value};

/// Build the canonical Agent Card for the hosted assistant at `address`
/// (an AINM address, `assistant@<host>`). The card's `url` is derived from the
/// address host (`https://<host>`).
pub fn build_assistant_card(address: &str) -> Value {
    let host = address.split('@').nth(1).unwrap_or(address);
    json!({
        "name": "Assistant",
        "description": "Your home server's built-in AI assistant.",
        "url": format!("https://{host}"),
        "version": "0.1.0",
        "protocolVersion": "0.1.0",
        "capabilities": {
            "streaming": true,
            "pushNotifications": false
        },
        "skills": [],
        "ainm": {
            "protocolVersion": "0.1.0",
            "address": address,
            "type": "agent",
            "contentTypes": ["text", "code"],
            "streaming": {
                "supported": true,
                "parallelBlocks": false,
                "preferredChunkSize": "token"
            },
            "threading": {
                "subThreads": true,
                "maxConcurrentThreads": null
            },
            "memory": {
                "archivalSupported": true,
                "contextFramePreferred": true
            },
            "tools": []
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_assistant_card_is_canonical_camelcase() {
        let card = build_assistant_card("assistant@example.test");

        // Top-level required fields (schema: name, description, url, version,
        // protocolVersion, capabilities, skills, ainm).
        assert_eq!(card["name"], "Assistant");
        assert!(card["description"].as_str().is_some_and(|s| !s.is_empty()));
        assert_eq!(card["url"], "https://example.test");
        assert!(card["version"].as_str().is_some_and(|s| !s.is_empty()));
        assert_eq!(card["protocolVersion"], "0.1.0");
        assert_eq!(card["capabilities"]["streaming"], true);
        assert_eq!(card["capabilities"]["pushNotifications"], false);
        assert!(card["skills"].is_array());

        // AINM extensions (camelCase keys, schema $defs/AINMExtensions).
        let ainm = &card["ainm"];
        assert_eq!(ainm["protocolVersion"], "0.1.0");
        assert_eq!(ainm["address"], "assistant@example.test");
        assert_eq!(ainm["type"], "agent");
        assert_eq!(ainm["contentTypes"], json!(["text", "code"]));
        assert_eq!(ainm["streaming"]["supported"], true);
        assert_eq!(ainm["streaming"]["parallelBlocks"], false);
        assert_eq!(ainm["streaming"]["preferredChunkSize"], "token");
        assert_eq!(ainm["threading"]["subThreads"], true);
        assert!(ainm["threading"]["maxConcurrentThreads"].is_null());
        assert_eq!(ainm["memory"]["archivalSupported"], true);
        assert_eq!(ainm["memory"]["contextFramePreferred"], true);
        assert_eq!(ainm["tools"], json!([]));

        // No snake_case leakage anywhere in the serialized form.
        let s = serde_json::to_string(&card).unwrap();
        for snake in [
            "push_notifications", "protocol_version", "content_types",
            "parallel_blocks", "preferred_chunk_size", "sub_threads",
            "max_concurrent_threads", "archival_supported", "context_frame_preferred",
            "input_schema",
        ] {
            assert!(!s.contains(snake), "card must not contain snake_case key `{snake}`: {s}");
        }
    }

    #[test]
    fn build_assistant_card_address_without_host_falls_back() {
        // Defensive: a malformed address (no `@host`) still yields a valid card.
        let card = build_assistant_card("assistant");
        assert_eq!(card["ainm"]["address"], "assistant");
        assert_eq!(card["url"], "https://assistant");
    }
}
```

- [ ] **Step 2: Register the module in the binary crate**

In `server/src/main.rs`, add the module declaration next to the existing `mod hosted_agent;` (line 16):

```rust
mod agent_card;
```

- [ ] **Step 3: Run the unit test to verify it passes**

Run: `cd server && cargo test build_assistant_card`
Expected: PASS — `build_assistant_card_is_canonical_camelcase` and `build_assistant_card_address_without_host_falls_back` both pass. (They appear under the `ainm-server` bin test target.)

- [ ] **Step 4: Commit**

```bash
rtk proxy git add server/src/agent_card.rs server/src/main.rs
rtk proxy git commit -m "feat(server): build_assistant_card canonical Agent Card builder"
```

---

## Task 2: Wire the builder into `published_card_for`

Add a hosted-assistant branch to `published_card_for`, checked BEFORE the existing static-card identity match, so the well-known route serves the real assistant card when an assistant is provisioned.

**Files:**
- Modify: `server/src/server.rs:983-1006` (`published_card_for`)

- [ ] **Step 1: Add the hosted-assistant branch**

In `server/src/server.rs`, replace the body of `published_card_for` (currently lines ~990-1006) with the version below. Keep the existing doc comment above it, but update it to mention the new branch:

```rust
/// The agent card this server publishes for `agent_name`, or `None` (→ 404).
///
/// Resolution order:
/// 1. **Hosted assistant:** if a hosted agent is provisioned and its address local-part
///    matches `agent_name`, serve its freshly built canonical card.
/// 2. **Static card:** the static `state.agent_card` is published only when its own declared
///    address local-part matches `agent_name`.
///
/// A null / identity-less card with no matching hosted agent publishes nothing, so every name
/// 404s. This keeps the well-known route from returning a card under an arbitrary (typo'd or
/// not-yet-connected) agent name. Per-agent card publishing for live connector agents is a
/// separate, larger change and is intentionally NOT handled here.
fn published_card_for(state: &ServerState, agent_name: &str) -> Option<serde_json::Value> {
    // (1) Hosted assistant: build its canonical card on demand.
    if let Some(agent) = &state.hosted_agent {
        let local = agent.address.split('@').next().unwrap_or("");
        if !local.is_empty() && local == agent_name {
            return Some(crate::agent_card::build_assistant_card(&agent.address));
        }
    }

    // (2) Static card: serve only under its own declared identity.
    let card = &state.agent_card;
    let addr = card
        .get("address")
        .and_then(|v| v.as_str())
        .or_else(|| {
            card.get("ainm")
                .and_then(|a| a.get("address"))
                .and_then(|v| v.as_str())
        })?;
    let local = addr.split('@').next().unwrap_or("");
    if !local.is_empty() && local == agent_name {
        Some(card.clone())
    } else {
        None
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd server && cargo build --bin ainm-server`
Expected: compiles with no new warnings.

- [ ] **Step 3: Run the existing route regression test to confirm no regression**

Run: `cd server && cargo test --test agent_card_route`
Expected: `canonical_agent_card_route_serves_hosted_card` still PASSES (helper→200, ghost→404, `agents/`→not-200). That test uses `--mode agent` with no DB, so `hosted_agent` is `None` and only branch (2) applies — behavior unchanged.

- [ ] **Step 4: Commit**

```bash
rtk proxy git add server/src/server.rs
rtk proxy git commit -m "feat(server): serve hosted assistant card from published_card_for"
```

---

## Task 3: Integration test — assistant card route (DB-or-skip)

Boot the binary with a DB (so `run_federation` provisions `assistant@<identity>`) in `--mode human` (where `state.agent_card` is `null`, proving the card comes from the new hosted-assistant branch), and assert the assistant card resolves at the canonical route.

**Files:**
- Modify: `server/tests/agent_card_route.rs` (add a second `#[tokio::test]` + a DB-or-skip helper)

- [ ] **Step 1: Add a DB-or-skip helper to the test file**

At the top of `server/tests/agent_card_route.rs`, after the existing `free_port` helper (after line 18), add:

```rust
/// Resolve a test Postgres URL (TEST_DATABASE_URL → AINM_DATABASE__URL → DATABASE_URL),
/// connect, and run migrations. Returns the URL the spawned binary should use. None → skip.
async fn db_url_or_skip() -> Option<String> {
    let url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("AINM_DATABASE__URL"))
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()?;
    let pool = sqlx::PgPool::connect(&url).await.ok()?;
    let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    sqlx::migrate::Migrator::new(migrations_dir)
        .await
        .ok()?
        .run(&pool)
        .await
        .ok()?;
    Some(url)
}
```

- [ ] **Step 2: Add the assistant-card integration test**

Append this `#[tokio::test]` to `server/tests/agent_card_route.rs`:

```rust
/// With a DB present, the binary provisions `assistant@<identity>` (run_federation).
/// In `--mode human` the static `agent_card` is `null`, so a 200 here proves the card is
/// produced by the hosted-assistant branch of `published_card_for`, not the static fallback.
#[tokio::test]
async fn assistant_card_route_serves_hosted_card() {
    let Some(db_url) = db_url_or_skip().await else {
        eprintln!("no DB; skipping assistant_card_route_serves_hosted_card");
        return;
    };

    // Stable domain: provisioning is idempotent (ON CONFLICT (address) DO NOTHING) and the
    // `assistant` username is globally unique, so a fixed domain re-uses the same row across runs.
    let domain = "assistant-card-e2e.local";
    let port = free_port().await;
    let certs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("certs");

    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_ainm-server"))
        .args([
            "--mode", "human",
            "--port", &port.to_string(),
            "--identity", domain,
            "--cert", certs.join("server-a.pem").to_str().unwrap(),
            "--key", certs.join("server-a-key.pem").to_str().unwrap(),
            "--ca-cert", certs.join("ca.pem").to_str().unwrap(),
        ])
        .env("AINM_DATABASE__URL", &db_url)
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
    if !ready {
        let _ = child.kill().await;
        let mut stderr = String::new();
        if let Some(mut e) = child.stderr.take() {
            use tokio::io::AsyncReadExt;
            let _ = e.read_to_string(&mut stderr).await;
        }
        panic!("server did not become ready in 15s. stderr tail:\n{}", stderr);
    }

    // The hosted assistant card resolves at the canonical route.
    let card_url = format!("http://127.0.0.1:{port}/.well-known/ainm/assistant/agent-card.json");
    let resp = http.get(&card_url).send().await.expect("GET assistant card");
    assert_eq!(resp.status().as_u16(), 200, "assistant card route must return 200 with a DB");

    let body: serde_json::Value = resp.json().await.expect("assistant card JSON");
    assert_eq!(body["name"], "Assistant", "assistant card name; got {body}");
    assert_eq!(
        body["capabilities"]["pushNotifications"], false,
        "camelCase capability key must be present; got {body}"
    );
    assert_eq!(body["ainm"]["memory"]["archivalSupported"], true, "got {body}");

    // Unknown names still 404 even with an assistant provisioned.
    let unknown = format!("http://127.0.0.1:{port}/.well-known/ainm/ghost/agent-card.json");
    let unknown_resp = http.get(&unknown).send().await.expect("GET unknown");
    assert_eq!(unknown_resp.status().as_u16(), 404, "unknown agent name must 404");

    child.start_kill().ok();
}
```

- [ ] **Step 3: Run the test**

Run: `cd server && cargo test --test agent_card_route`
Expected: BOTH tests pass when a DB is configured. If no DB env var is set, `assistant_card_route_serves_hosted_card` prints the skip line and returns (counts as passed), while `canonical_agent_card_route_serves_hosted_card` still passes.

- [ ] **Step 4: Commit**

```bash
rtk proxy git add server/tests/agent_card_route.rs
rtk proxy git commit -m "test(server): assistant card resolves at canonical well-known route"
```

---

## Task 4: iOS decoder camelCase reconciliation + decode test

Drop the snake_case `CodingKeys` from `AgentCard.swift` so Swift's synthesized Codable uses the canonical camelCase property names. Add a unit test that decodes a canonical camelCase card.

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/Models/AgentCard.swift`
- Create: `ios/AINMMessenger/AINMMessengerTests/ModelTests/AgentCardDecodeTests.swift`

- [ ] **Step 1: Write the failing decode test**

Create `ios/AINMMessenger/AINMMessengerTests/ModelTests/AgentCardDecodeTests.swift`:

```swift
import XCTest
@testable import AINMMessenger

final class AgentCardDecodeTests: XCTestCase {
    /// Decodes a canonical (camelCase) Agent Card exactly as the server publishes it.
    func testDecodesCanonicalCamelCaseCard() throws {
        let json = """
        {
          "name": "Assistant",
          "description": "Your home server's built-in AI assistant.",
          "url": "https://example.test",
          "version": "0.1.0",
          "protocolVersion": "0.1.0",
          "capabilities": { "streaming": true, "pushNotifications": false },
          "skills": [],
          "ainm": {
            "protocolVersion": "0.1.0",
            "address": "assistant@example.test",
            "type": "agent",
            "contentTypes": ["text", "code"],
            "streaming": { "supported": true, "parallelBlocks": false, "preferredChunkSize": "token" },
            "threading": { "subThreads": true, "maxConcurrentThreads": null },
            "memory": { "archivalSupported": true, "contextFramePreferred": true },
            "tools": []
          }
        }
        """
        let card = try JSONDecoder().decode(AgentCard.self, from: Data(json.utf8))

        XCTAssertEqual(card.name, "Assistant")
        XCTAssertEqual(card.protocolVersion, "0.1.0")
        XCTAssertTrue(card.capabilities.streaming)
        XCTAssertFalse(card.capabilities.pushNotifications)

        let ainm = try XCTUnwrap(card.ainm)
        XCTAssertEqual(ainm.protocolVersion, "0.1.0")
        XCTAssertEqual(ainm.contentTypes, ["text", "code"])
        XCTAssertFalse(ainm.streaming.parallelBlocks)
        XCTAssertEqual(ainm.streaming.preferredChunkSize, "token")
        XCTAssertTrue(ainm.threading.subThreads)
        XCTAssertNil(ainm.threading.maxConcurrentThreads)
        XCTAssertTrue(ainm.memory.archivalSupported)
        XCTAssertTrue(ainm.memory.contextFramePreferred)
        XCTAssertTrue(ainm.tools.isEmpty)
    }

    /// A card round-trips through encode→decode with camelCase keys (saveAgent stores cardJson
    /// by encoding an AgentCard; the stored snapshot must decode back).
    func testRoundTripsThroughEncoding() throws {
        let json = """
        {"name":"Assistant","description":"d","url":"","version":"1.0","protocolVersion":"0.1.0",
         "capabilities":{"streaming":true,"pushNotifications":false},"skills":[],
         "ainm":{"protocolVersion":"0.1.0","address":"assistant@h","type":"agent",
          "contentTypes":["text"],"streaming":{"supported":true,"parallelBlocks":false,"preferredChunkSize":"token"},
          "threading":{"subThreads":true,"maxConcurrentThreads":null},
          "memory":{"archivalSupported":true,"contextFramePreferred":false},"tools":[]}}
        """
        let card = try JSONDecoder().decode(AgentCard.self, from: Data(json.utf8))
        let reencoded = try JSONEncoder().encode(card)
        let again = try JSONDecoder().decode(AgentCard.self, from: reencoded)
        XCTAssertEqual(again.ainm?.memory.archivalSupported, true)
        XCTAssertEqual(again.ainm?.streaming.preferredChunkSize, "token")
        // Encoded form must be camelCase (no snake_case keys).
        let encodedString = String(decoding: reencoded, as: UTF8.self)
        XCTAssertFalse(encodedString.contains("push_notifications"))
        XCTAssertFalse(encodedString.contains("protocol_version"))
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test -only-testing:AINMMessengerTests/AgentCardDecodeTests`
Expected: FAIL — `testDecodesCanonicalCamelCaseCard` fails to decode (`keyNotFound` for `pushNotifications` / `protocolVersion`, since the current decoder expects `push_notifications` / `protocol_version`).

- [ ] **Step 3: Drop the snake_case CodingKeys**

Replace the entire contents of `ios/AINMMessenger/AINMMessenger/Models/AgentCard.swift` with the version below. Every `enum CodingKeys` block is removed so Swift synthesizes coding from the camelCase property names directly (which equal the canonical schema keys). Extra JSON keys the model omits (`ainm.address`, `ainm.type`, `ainm.bridgeType`) are ignored by the synthesized decoder.

```swift
import Foundation

/// Agent Card model per AINM protocol agent-card.md specification.
/// Compatible with A2A Agent Cards with AINM-specific extensions.
///
/// Keys are canonical camelCase (matching `schemas/agent-card.schema.json` and the
/// server's published cards). Swift's synthesized Codable maps property names directly —
/// no `CodingKeys` overrides. Unmodeled JSON keys (e.g. `ainm.address`, `ainm.type`) are
/// ignored on decode.
struct AgentCard: Codable, Sendable {
    let name: String
    let description: String
    let url: String
    let version: String
    let protocolVersion: String
    let capabilities: A2ACapabilities
    let skills: [Skill]
    let ainm: AINMExtension?

    struct A2ACapabilities: Codable, Sendable {
        let streaming: Bool
        let pushNotifications: Bool
    }

    struct Skill: Codable, Sendable, Identifiable {
        let id: String
        let name: String
        let description: String
        let tags: [String]?
    }

    struct AINMExtension: Codable, Sendable {
        let protocolVersion: String
        let contentTypes: [String]
        let streaming: StreamingCapability
        let threading: ThreadingCapability
        let memory: MemoryCapability
        let tools: [Tool]
    }

    struct StreamingCapability: Codable, Sendable {
        let supported: Bool
        let parallelBlocks: Bool
        let preferredChunkSize: String
    }

    struct ThreadingCapability: Codable, Sendable {
        let subThreads: Bool
        let maxConcurrentThreads: Int?
    }

    struct MemoryCapability: Codable, Sendable {
        let archivalSupported: Bool
        let contextFramePreferred: Bool
    }

    struct Tool: Codable, Sendable, Identifiable {
        var id: String { name }
        let name: String
        let description: String
        let inputSchema: AnyCodableValue
    }
}
```

- [ ] **Step 4: Run the decode test to verify it passes**

Run: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test -only-testing:AINMMessengerTests/AgentCardDecodeTests`
Expected: PASS — both tests pass.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Models/AgentCard.swift ios/AINMMessenger/AINMMessengerTests/ModelTests/AgentCardDecodeTests.swift
rtk proxy git commit -m "feat(ios): decode canonical camelCase AgentCard (drop snake_case CodingKeys)"
```

---

## Task 5: Update fixtures + existing test to camelCase

The snake_case JSON in the scenario-03 UITest fixture and the `SavedAgentStoreTests` inline cardJson must become camelCase, or they stop decoding after Task 4.

**Files:**
- Modify: `ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-03-agent-discovery.json`
- Modify: `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/SavedAgentStoreTests.swift:8`

- [ ] **Step 1: Convert the fixture card JSON keys to camelCase**

The fixture embeds 4 card JSON strings (2 under `agentCards`, 2 under `savedAgents`), each with snake_case keys. Convert every snake_case AgentCard key to camelCase across the file. These tokens appear only as JSON keys (no value strings contain them), so token replacement is safe:

```bash
cd /Users/santiagogerickeparga/Proyectos/ai-native-messaging
F=ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-03-agent-discovery.json
sed -i '' \
  -e 's/protocol_version/protocolVersion/g' \
  -e 's/push_notifications/pushNotifications/g' \
  -e 's/content_types/contentTypes/g' \
  -e 's/parallel_blocks/parallelBlocks/g' \
  -e 's/preferred_chunk_size/preferredChunkSize/g' \
  -e 's/sub_threads/subThreads/g' \
  -e 's/max_concurrent_threads/maxConcurrentThreads/g' \
  -e 's/archival_supported/archivalSupported/g' \
  -e 's/context_frame_preferred/contextFramePreferred/g' \
  -e 's/input_schema/inputSchema/g' \
  "$F"
```

- [ ] **Step 2: Verify no snake_case AgentCard keys remain in the fixture**

Run: `grep -nE 'protocol_version|push_notifications|content_types|parallel_blocks|preferred_chunk_size|sub_threads|max_concurrent_threads|archival_supported|context_frame_preferred|input_schema' ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-03-agent-discovery.json`
Expected: no matches (exit 1 / empty output).

- [ ] **Step 3: Update the SavedAgentStoreTests inline cardJson**

In `ios/AINMMessenger/AINMMessengerTests/PersistenceTests/SavedAgentStoreTests.swift`, replace line 8:

```swift
        let cardJson = "{\"name\":\"Helper\",\"description\":\"d\",\"url\":\"\",\"version\":\"1.0\",\"protocol_version\":\"0.1.0\",\"capabilities\":{\"streaming\":true,\"push_notifications\":false},\"skills\":[]}"
```

with:

```swift
        let cardJson = "{\"name\":\"Helper\",\"description\":\"d\",\"url\":\"\",\"version\":\"1.0\",\"protocolVersion\":\"0.1.0\",\"capabilities\":{\"streaming\":true,\"pushNotifications\":false},\"skills\":[]}"
```

- [ ] **Step 4: Run the affected iOS suites**

Run: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test -only-testing:AINMMessengerTests/SavedAgentStoreTests`
Expected: PASS — `testRecordPersistsAndDecodesCard` decodes the camelCase cardJson and `fetched?.agentCard()?.name == "Helper"`.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add ios/AINMMessenger/AINMMessenger/Testing/Fixtures/scenario-03-agent-discovery.json ios/AINMMessenger/AINMMessengerTests/PersistenceTests/SavedAgentStoreTests.swift
rtk proxy git commit -m "test(ios): camelCase card JSON in scenario-03 fixture + SavedAgentStore test"
```

---

## Task 6: Full suites green + Definition of Done

Run the complete suites and verify the DoD.

**Files:** none (verification only).

- [ ] **Step 1: Full server suite**

Run: `cd server && cargo test`
Expected: all pass EXCEPT the two known pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence` in `tests/integration.rs`). The new `build_assistant_card` unit tests and `assistant_card_route_serves_hosted_card` (or its skip) pass.

- [ ] **Step 2: Clippy clean**

Run: `cd server && cargo clippy --all-targets -- -D warnings`
Expected: no warnings introduced by `agent_card.rs` or the `published_card_for` change.

- [ ] **Step 3: Full iOS unit suite**

Run: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test -only-testing:AINMMessengerTests`
Expected: all pass (AgentCardDecodeTests, SavedAgentStoreTests, and the rest).

- [ ] **Step 4: AgentDiscovery UITests**

Run: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test -only-testing:AINMMessengerUITests/AgentDiscoveryUITests`
Expected: PASS — the scenario-03 fixture (now camelCase) decodes and the saved-agents list renders.

- [ ] **Step 5: Confirm the DoD**

Verify against the spec §9:
1. `build_assistant_card` produces a canonical camelCase card; unit-tested. ✅ (Task 1)
2. `/.well-known/ainm/assistant/agent-card.json` returns the card (200) when provisioned; unknown names 404. ✅ (Tasks 2–3)
3. iOS `AgentCard` decodes canonical camelCase; fixtures + `SavedAgentStoreTests` updated; decode test green; full iOS unit + UITest green. ✅ (Tasks 4–6)
4. Server suite green except the two known pre-existing failures. ✅ (Step 1)
5. Live: seeded Assistant renders a real card — **deferred to Task 7 (manual)**.
6. Documented limitations (connector-agent cards, federation-listener parity, DB-configurable cards, live-tool enumeration) remain future — recorded in the spec §1/§9.

No commit (verification only). If any step reveals a fix, commit it with a `fix(...)` message.

---

## Task 7: MANUAL live verification (human-run)

This step requires a running DB-backed server, the iOS simulator, and human eyes. The dev session cannot self-verify it.

**Runbook (human):**

1. Start a DB-backed server (so the assistant is provisioned), e.g. `AINM_DATABASE__URL=<url> cargo run -p ainm-server -- --mode human --identity localhost --port 8080 --cert server/certs/server-a.pem --key server/certs/server-a-key.pem --ca-cert server/certs/ca.pem`.
2. Confirm the card serves: `curl http://localhost:8080/.well-known/ainm/assistant/agent-card.json` → 200 with `"name":"Assistant"` and camelCase keys.
3. In the app (against `ws://localhost:8080`), open the Agents list. The seeded **Assistant** contact should now show the **real** card — name "Assistant", description "Your home server's built-in AI assistant.", and capability badges (streaming / memory) — **not** the placeholder ("Built-in assistant on your home server.").
4. Tap the Assistant, start a thread, send a message, and confirm a reply (regression check that routing still works).

- [ ] **Step 1:** Human runs the runbook above and confirms the real card renders. Mark complete when verified.
