# Connector-Agent Card Publishing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A live connector agent publishes a real, schema-conformant camelCase AgentCard (incl. its tools) at `/.well-known/ainm/{name}/agent-card.json` while connected; the route 404s once it disconnects.

**Architecture:** The TS connector SDK sends a structured `card` object (name, description, version, skills, tools) in the `ainm.initialize` params. The server assembles the canonical card via a new pure `build_connector_card`, stores it in the ephemeral `AgentRegistry` at `handle_agent_connection`, and `published_card_for` gains a registry-lookup branch. No iOS change (decoder is already canonical camelCase).

**Tech Stack:** TypeScript (connector SDK, vitest), Rust (Axum/serde_json server, binary-only `server.rs`).

**Conventions for this plan:**
- All git operations use `rtk proxy git` (never bare `git`). Commits go **directly to `main`**.
- Server tests: `cd server && cargo test <filter>`. Two pre-existing failures are expected and unrelated: `test_graph_traversal`, `test_compilation_dedup_updates_confidence` (`tests/integration.rs`).
- Connector tests: `cd packages/connector && npm test` (vitest; tests in `packages/connector/tests/*.test.ts`).
- `server.rs` is **binary-only** (`mod server;` in `main.rs`, not in `lib.rs`); `crate::agent_card::*` resolves there because `mod agent_card;` is declared in `main.rs`.
- The hosted-assistant card (shipped) lives in `server/src/agent_card.rs` as `build_assistant_card`; this plan adds a sibling `build_connector_card` in the same file.
- `published_card_for` is currently **sync** at `server.rs:995`; `agent_card_handler` (async) calls it at `:1032`. The `AgentRegistry` lock is async, so a registry lookup forces `published_card_for` to become `async` (Task 4).

---

## Task 1: Connector SDK — send `card` in the handshake

Extend the connector so `ainm.initialize` carries a structured `card` object built from the agent's config + registered tools. Cohesive TS change across types, handshake, and agent wiring, with vitest coverage.

**Files:**
- Modify: `packages/connector/src/protocol/types.ts` (`AINMAgentConfig`)
- Modify: `packages/connector/src/protocol/handshake.ts` (`HandshakeOptions` + `performHandshake`)
- Modify: `packages/connector/src/agent.ts` (collect tools, pass card fields)
- Modify: `packages/connector/tests/handshake.test.ts`

- [ ] **Step 1: Write the failing handshake test for `params.card`**

Append this test inside the `describe("handshake", ...)` block in `packages/connector/tests/handshake.test.ts`:

```ts
  it("includes a card object with name, description, version, skills, and tools", async () => {
    const transport = mockTransport();
    const kp = await generateKeypair();

    const handshakePromise = performHandshake(transport, {
      identity: "coder@server.local",
      name: "Coder",
      description: "Writes and reviews code.",
      version: "2.1.0",
      capabilities: ["text", "code"],
      skills: [{ name: "review", description: "Reviews diffs." }],
      tools: [
        { name: "run_tests", description: "Runs the test suite.", parameters: { type: "object", properties: {} } },
      ],
      keypair: kp,
    });

    await new Promise((r) => setTimeout(r, 10));
    const initReq = JSON.parse(transport.sent[0]);
    expect(initReq.params.card).toBeDefined();
    expect(initReq.params.card.name).toBe("Coder");
    expect(initReq.params.card.description).toBe("Writes and reviews code.");
    expect(initReq.params.card.version).toBe("2.1.0");
    expect(initReq.params.card.skills).toEqual([{ name: "review", description: "Reviews diffs." }]);
    expect(initReq.params.card.tools).toEqual([
      { name: "run_tests", description: "Runs the test suite.", parameters: { type: "object", properties: {} } },
    ]);
    // capabilities object is unchanged
    expect(initReq.params.capabilities.participant_type).toBe("agent");
    expect(initReq.params.capabilities.contentTypes).toEqual(["text", "code"]);

    // Complete the handshake so the promise resolves cleanly.
    transport.triggerMessage(JSON.stringify({ jsonrpc: "2.0", id: initReq.id, result: { protocolVersion: "0.1.0", capabilities: {} } }));
    await handshakePromise;
  });
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd packages/connector && npm test -- handshake`
Expected: FAIL — `initReq.params.card` is `undefined` (current handshake omits it), and `HandshakeOptions` has no `description`/`version`/`tools` (TS compile error on the test). Both confirm the gap.

- [ ] **Step 3: Add `description?`/`version?` to `AINMAgentConfig`**

In `packages/connector/src/protocol/types.ts`, extend the interface (keep existing fields):

```ts
export interface AINMAgentConfig {
  server: string;
  identity: string;
  name: string;
  description?: string;
  version?: string;
  capabilities?: string[];
  skills?: Array<{ name: string; description: string }>;
  privateKey?: Uint8Array;
  reconnect?: boolean;
  reconnectMaxDelay?: number;
}
```

- [ ] **Step 4: Extend `HandshakeOptions` + emit `params.card`**

In `packages/connector/src/protocol/handshake.ts`, replace the `HandshakeOptions` interface and the `initReq` construction:

```ts
export interface HandshakeOptions {
  identity: string;
  name: string;
  description?: string;
  version?: string;
  capabilities: string[];
  skills?: Array<{ name: string; description: string }>;
  tools?: Array<{ name: string; description: string; parameters: Record<string, unknown> }>;
  keypair: Keypair;
}
```

Then, inside `performHandshake`, after computing `signature` and before `buildRequest`, assemble the card and include it in the params:

```ts
  const card: Record<string, unknown> = {
    name: options.name,
    skills: options.skills ?? [],
    tools: options.tools ?? [],
  };
  if (options.description !== undefined) card.description = options.description;
  if (options.version !== undefined) card.version = options.version;

  const initReq = buildRequest("ainm.initialize", {
    protocolVersion: "0.1.0",
    identity: options.identity,
    signature,
    capabilities: {
      streaming: true,
      subThreads: true,
      contentTypes: options.capabilities,
      participant_type: "agent",
      toolVisibility: true,
    },
    card,
  });
```

(Leave the rest of `performHandshake` — `waitForResponse`, the `ainm.initialized` notification, the return — unchanged.)

- [ ] **Step 5: Collect tools + pass card fields in `agent.ts`**

In `packages/connector/src/agent.ts`, replace the `performHandshake(...)` call (currently at ~line 83) with one that derives the tool list from the registered-tools map and forwards the card fields:

```ts
    const tools = Array.from(this.tools.entries()).map(([name, { definition }]) => ({
      name,
      description: definition.description,
      parameters: definition.parameters,
    }));

    this.handshakeResult = await performHandshake(this.transport, {
      identity: this.config.identity,
      name: this.config.name,
      description: this.config.description,
      version: this.config.version,
      capabilities: this.config.capabilities ?? [],
      skills: this.config.skills,
      tools,
      keypair: this.keypair,
    });
```

- [ ] **Step 6: Run the connector test suite**

Run: `cd packages/connector && npm test`
Expected: PASS — the new handshake test passes, and the existing handshake/agent/rpc/etc. tests still pass.

- [ ] **Step 7: Commit**

```bash
rtk proxy git add packages/connector/src/protocol/types.ts packages/connector/src/protocol/handshake.ts packages/connector/src/agent.ts packages/connector/tests/handshake.test.ts
rtk proxy git commit -m "feat(connector): send agent card (name/description/skills/tools) in handshake"
```

---

## Task 2: Server — `build_connector_card` pure builder + unit tests

Add a pure builder beside `build_assistant_card` that assembles a canonical camelCase card from the handshake params. Explicit key/value assertions (no live schema validation — `jsonschema` isn't a server dependency; the schema has an external `$ref`).

**Files:**
- Modify: `server/src/agent_card.rs`

- [ ] **Step 1: Add the function**

Append to `server/src/agent_card.rs` (after `build_assistant_card`, before the `#[cfg(test)]` module):

```rust
/// Build a canonical Agent Card for a live connector agent from its `ainm.initialize` params.
///
/// `address` is the agent's AINM address (`name@host`); `init_params` is the `params` object of
/// the initialize request. Card fields come from `params.card` (sent by the connector SDK);
/// transport capabilities come from `params.capabilities`. All keys camelCase. Pure: no I/O.
///
/// Robust to partial/absent input: a missing `card` yields a card built from capabilities +
/// defaults (name = address local-part, description = "{name} agent"). Malformed tools/skills are
/// dropped (logged), never panicking and never failing the caller.
pub fn build_connector_card(address: &str, init_params: &Value) -> Value {
    let local = address.split('@').next().unwrap_or(address);
    let host = address.split('@').nth(1).unwrap_or(address);
    let card = init_params.get("card");
    let caps = init_params.get("capabilities");

    let name = card
        .and_then(|c| c.get("name"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(local)
        .to_string();
    let description = card
        .and_then(|c| c.get("description"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{name} agent"));
    let version = card
        .and_then(|c| c.get("version"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("0.1.0")
        .to_string();

    let streaming = caps
        .and_then(|c| c.get("streaming"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let sub_threads = caps
        .and_then(|c| c.get("subThreads"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let content_types: Vec<Value> = caps
        .and_then(|c| c.get("contentTypes"))
        .and_then(|v| v.as_array())
        .cloned()
        .filter(|a| !a.is_empty())
        .unwrap_or_else(|| vec![json!("text")]);

    let skills = normalize_skills(card.and_then(|c| c.get("skills")));
    let tools = normalize_tools(card.and_then(|c| c.get("tools")));

    json!({
        "name": name,
        "description": description,
        "url": format!("https://{host}"),
        "version": version,
        "protocolVersion": "0.1.0",
        "capabilities": { "streaming": streaming, "pushNotifications": false },
        "skills": skills,
        "ainm": {
            "protocolVersion": "0.1.0",
            "address": address,
            "type": "agent",
            "contentTypes": content_types,
            "streaming": { "supported": streaming, "parallelBlocks": false, "preferredChunkSize": "token" },
            "threading": { "subThreads": sub_threads, "maxConcurrentThreads": null },
            "memory": { "archivalSupported": false, "contextFramePreferred": false },
            "tools": tools
        }
    })
}

/// Normalize `card.skills` into valid `AgentSkill` objects. The schema requires id/name/description;
/// derive a slug `id` from `name` when absent. Drop entries lacking name or description.
fn normalize_skills(skills: Option<&Value>) -> Vec<Value> {
    let Some(arr) = skills.and_then(|v| v.as_array()) else { return vec![] };
    arr.iter()
        .filter_map(|s| {
            let name = s.get("name").and_then(|v| v.as_str()).filter(|x| !x.is_empty())?;
            let description = s.get("description").and_then(|v| v.as_str()).filter(|x| !x.is_empty())?;
            let id = s
                .get("id")
                .and_then(|v| v.as_str())
                .filter(|x| !x.is_empty())
                .map(|x| x.to_string())
                .unwrap_or_else(|| slug(name));
            Some(json!({ "id": id, "name": name, "description": description }))
        })
        .collect()
}

/// Normalize `card.tools` into valid `AgentTool` objects. The schema requires
/// name/description/inputSchema, with `inputSchema` a JSON object. Drop entries whose `parameters`
/// is missing or not an object (logged), or lacking name/description.
fn normalize_tools(tools: Option<&Value>) -> Vec<Value> {
    let Some(arr) = tools.and_then(|v| v.as_array()) else { return vec![] };
    arr.iter()
        .filter_map(|t| {
            let name = t.get("name").and_then(|v| v.as_str()).filter(|x| !x.is_empty())?;
            let description = t.get("description").and_then(|v| v.as_str()).filter(|x| !x.is_empty())?;
            match t.get("parameters") {
                Some(p) if p.is_object() => {
                    Some(json!({ "name": name, "description": description, "inputSchema": p.clone() }))
                }
                _ => {
                    tracing::warn!("[connector-card] dropping tool `{name}`: parameters missing or not an object");
                    None
                }
            }
        })
        .collect()
}

/// Lowercase alphanumeric slug with `-` separators (for deriving a skill id from its name).
fn slug(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}
```

Ensure the file's top `use` already imports `json` and `Value` (it does: `use serde_json::{json, Value};` from `build_assistant_card`). `tracing` is a server dependency; reference it as `tracing::warn!` (no `use` needed).

- [ ] **Step 2: Add unit tests**

Inside the existing `#[cfg(test)] mod tests { ... }` in `server/src/agent_card.rs`, add:

```rust
    #[test]
    fn build_connector_card_full_params_is_canonical() {
        let params = json!({
            "identity": "coder@h.test",
            "capabilities": { "streaming": true, "subThreads": true, "contentTypes": ["text", "code"] },
            "card": {
                "name": "Coder",
                "description": "Writes and reviews code.",
                "version": "2.1.0",
                "skills": [{ "name": "Review", "description": "Reviews diffs." }],
                "tools": [{ "name": "run_tests", "description": "Runs tests.", "parameters": { "type": "object" } }]
            }
        });
        let card = build_connector_card("coder@h.test", &params);

        assert_eq!(card["name"], "Coder");
        assert_eq!(card["description"], "Writes and reviews code.");
        assert_eq!(card["url"], "https://h.test");
        assert_eq!(card["version"], "2.1.0");
        assert_eq!(card["protocolVersion"], "0.1.0");
        assert_eq!(card["capabilities"]["streaming"], true);
        assert_eq!(card["capabilities"]["pushNotifications"], false);
        // skill: id slugged from name
        assert_eq!(card["skills"][0]["id"], "review");
        assert_eq!(card["skills"][0]["name"], "Review");
        let ainm = &card["ainm"];
        assert_eq!(ainm["address"], "coder@h.test");
        assert_eq!(ainm["type"], "agent");
        assert_eq!(ainm["contentTypes"], json!(["text", "code"]));
        assert_eq!(ainm["streaming"]["supported"], true);
        assert_eq!(ainm["threading"]["subThreads"], true);
        assert!(ainm["threading"]["maxConcurrentThreads"].is_null());
        assert_eq!(ainm["memory"]["archivalSupported"], false);
        // tool: parameters -> inputSchema
        assert_eq!(ainm["tools"][0]["name"], "run_tests");
        assert_eq!(ainm["tools"][0]["inputSchema"], json!({ "type": "object" }));

        let s = serde_json::to_string(&card).unwrap();
        for snake in ["push_notifications", "protocol_version", "content_types", "input_schema", "sub_threads"] {
            assert!(!s.contains(snake), "snake_case key `{snake}` leaked: {s}");
        }
    }

    #[test]
    fn build_connector_card_drops_malformed_tool() {
        let params = json!({
            "capabilities": { "contentTypes": ["text"] },
            "card": {
                "name": "X",
                "tools": [
                    { "name": "ok", "description": "good", "parameters": { "type": "object" } },
                    { "name": "bad", "description": "no params" },
                    { "name": "bad2", "description": "string params", "parameters": "nope" }
                ]
            }
        });
        let card = build_connector_card("x@h.test", &params);
        let tools = card["ainm"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1, "only the well-formed tool survives");
        assert_eq!(tools[0]["name"], "ok");
    }

    #[test]
    fn build_connector_card_defaults_without_card_object() {
        // No `card` key at all: name from address local-part, description defaulted, tools empty.
        let params = json!({ "capabilities": { "contentTypes": ["text"] } });
        let card = build_connector_card("solver@h.test", &params);
        assert_eq!(card["name"], "solver");
        assert_eq!(card["description"], "solver agent");
        assert_eq!(card["version"], "0.1.0");
        assert_eq!(card["ainm"]["tools"], json!([]));
        assert_eq!(card["skills"], json!([]));
        assert_eq!(card["ainm"]["contentTypes"], json!(["text"]));
    }
```

- [ ] **Step 3: Run the tests to verify they pass**

Run: `cd server && cargo test build_connector_card`
Expected: PASS — all three new tests pass.

- [ ] **Step 4: Commit**

```bash
rtk proxy git add server/src/agent_card.rs
rtk proxy git commit -m "feat(server): build_connector_card canonical card from handshake params"
```

---

## Task 3: Server — register the real card at `handle_agent_connection`

Replace the minimal `{identity, capabilities, connected_at}` blob with the canonical connector card.

**Files:**
- Modify: `server/src/server.rs` (`handle_agent_connection`, ~lines 1671-1684)

- [ ] **Step 1: Replace the blob with `build_connector_card`**

In `server/src/server.rs`, inside `handle_agent_connection`, replace this block:

```rust
    // Build a minimal agent card from the init request capabilities
    let capabilities = init_req
        .get("params")
        .and_then(|p| p.get("capabilities"))
        .cloned()
        .unwrap_or(serde_json::json!({}));
    let agent_card = serde_json::json!({
        "identity": from_addr,
        "capabilities": capabilities,
        "connected_at": protocol::now_rfc3339(),
    });

    // Register this agent in the AgentRegistry
    let _from_registry_rx = state.agent_registry.register(&from_addr, &agent_card).await;
```

with:

```rust
    // Build the canonical Agent Card from the init params (card object + capabilities) and register
    // it. The registry is ephemeral: `cleanup` (on disconnect) removes the card so the well-known
    // route 404s once the agent is gone.
    let empty_params = serde_json::json!({});
    let init_params = init_req.get("params").unwrap_or(&empty_params);
    let agent_card = crate::agent_card::build_connector_card(&from_addr, init_params);

    let _from_registry_rx = state.agent_registry.register(&from_addr, &agent_card).await;
```

- [ ] **Step 2: Verify it compiles**

Run: `cd server && cargo build --bin ainm-server`
Expected: clean compile, no new warnings.

- [ ] **Step 3: Commit**

```bash
rtk proxy git add server/src/server.rs
rtk proxy git commit -m "feat(server): register canonical connector card on agent connect"
```

---

## Task 4: Server — `published_card_for` registry branch (async)

Add a third branch to `published_card_for` that serves a connected agent's card from the registry; this requires making the function `async`.

**Files:**
- Modify: `server/src/server.rs` (`published_card_for` ~line 995, `agent_card_handler` ~line 1032, `AgentRegistry::get_agent_card` ~line 192)

- [ ] **Step 1: Make `published_card_for` async + add the registry branch**

In `server/src/server.rs`, change the signature `fn published_card_for(...)` to `async fn published_card_for(...)`, and add branch (3) just before the final `None`. The function ends with the static-card block returning `Some(card.clone()) / None`; replace that trailing `else { None }` path so the registry is consulted before giving up. Concretely, the static-card section becomes:

```rust
    // (2) Static card: serve only under its own declared identity.
    let card = &state.agent_card;
    if let Some(addr) = card
        .get("address")
        .and_then(|v| v.as_str())
        .or_else(|| {
            card.get("ainm")
                .and_then(|a| a.get("address"))
                .and_then(|v| v.as_str())
        })
    {
        let local = addr.split('@').next().unwrap_or("");
        if !local.is_empty() && local == agent_name {
            return Some(card.clone());
        }
    }

    // (3) Live connector agent: serve its registered (ephemeral) card while connected.
    let full_addr = format!("{agent_name}@{}", state.server_domain);
    if let Some(card) = state.agent_registry.get_agent_card(&full_addr).await {
        return Some(card);
    }

    None
```

(Keep branch (1), the hosted-assistant block, exactly as-is at the top of the function. Note the static-card section is rewritten from `let addr = ...?;` early-return form to an `if let` so that a null/identity-less static card falls through to branch (3) instead of returning early.)

- [ ] **Step 2: Await it in the handler**

In `agent_card_handler` (~line 1032), change:

```rust
    let Some(card) = published_card_for(&state, &agent_name) else {
        return StatusCode::NOT_FOUND.into_response();
    };
```

to:

```rust
    let Some(card) = published_card_for(&state, &agent_name).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
```

- [ ] **Step 3: Drop the dead-code allow on `get_agent_card`**

In `server/src/server.rs` (~line 189-193), remove the `#[allow(dead_code)]` attribute above `pub async fn get_agent_card` and update its doc comment to reflect that it is now used by `published_card_for`:

```rust
    /// Get the cached agent card for a connected agent identity. Consulted by `published_card_for`
    /// (branch 3) to serve a live connector agent's card at the well-known route while it is
    /// connected; returns `None` once the agent disconnects (the card is dropped by `cleanup`).
    pub async fn get_agent_card(&self, identity: &str) -> Option<serde_json::Value> {
        let cards = self.agent_cards.lock().await;
        cards.get(identity).cloned()
    }
```

- [ ] **Step 4: Build + run the existing route regression test**

Run: `cd server && cargo build --bin ainm-server && cargo test --test agent_card_route`
Expected: clean compile (no new warnings); both existing tests still pass (`canonical_agent_card_route_serves_hosted_card`, and `assistant_card_route_serves_hosted_card` which skips without a DB). Branch (1)/(2) behavior is preserved; branch (3) is inert when no agent is connected.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add server/src/server.rs
rtk proxy git commit -m "feat(server): serve live connector agent card from published_card_for (async)"
```

---

## Task 5: Server — connector card route integration test (DB-or-skip)

Black-box test: connect a WS client as an agent (signed handshake, `participant_type: "agent"`, with a `card`), assert its well-known route serves the card with its tool while connected, then disconnect and assert the route 404s.

**Files:**
- Create: `server/tests/connector_card_route.rs`

- [ ] **Step 1: Write the integration test**

Create `server/tests/connector_card_route.rs`:

```rust
//! Integration test: a live connector agent publishes a real card at the canonical well-known
//! route while connected, and the route 404s after it disconnects (ephemeral AgentRegistry).
//! Black-boxes the compiled binary (mirrors ws_conversation_e2e.rs); DB-or-skip.

use ainm_server::{identity, protocol, signing};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use std::process::Stdio;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

async fn pool_and_url_or_skip() -> Option<(sqlx::PgPool, String)> {
    let url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("AINM_DATABASE__URL"))
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()?;
    let pool = sqlx::PgPool::connect(&url).await.ok()?;
    let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    sqlx::migrate::Migrator::new(migrations_dir).await.ok()?.run(&pool).await.ok()?;
    Some((pool, url))
}

/// Insert an active, email-verified account and bind a freshly generated Ed25519 key (URL-safe
/// base64 no-pad of the public key), returning the in-memory identity for signing.
async fn make_bound_account(pool: &sqlx::PgPool, address: &str, domain: &str) -> identity::Identity {
    let username = address.split('@').next().unwrap();
    let account_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (username, domain, address, email_hash, email_verified, status) \
         VALUES ($1, $2, $3, $4, true, 'active') RETURNING id",
    )
    .bind(username)
    .bind(domain)
    .bind(address)
    .bind(format!("hash-{address}"))
    .fetch_one(pool)
    .await
    .expect("insert account");

    let id = identity::generate_keypair(address);
    let pk_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(id.public_key_bytes());
    sqlx::query(
        "INSERT INTO account_keys (account_id, public_key, label) VALUES ($1, $2, 'test')",
    )
    .bind(account_id)
    .bind(&pk_b64)
    .execute(pool)
    .await
    .expect("bind key");
    id
}

async fn cleanup_domain(pool: &sqlx::PgPool, domain: &str) {
    let _ = sqlx::query("DELETE FROM hosted_agents WHERE address LIKE $1")
        .bind(format!("%@{domain}"))
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM accounts WHERE domain = $1")
        .bind(domain)
        .execute(pool)
        .await;
}

async fn free_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}

async fn next_text(
    rx: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
    timeout: Duration,
) -> Option<String> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.checked_duration_since(tokio::time::Instant::now())?;
        match tokio::time::timeout(remaining, rx.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => return Some(t.to_string()),
            Ok(Some(Ok(_))) => continue,
            _ => return None,
        }
    }
}

#[tokio::test]
async fn connector_card_published_while_connected_then_404_on_disconnect() {
    let Some((pool, db_url)) = pool_and_url_or_skip().await else {
        eprintln!("no DB; skipping connector_card_published_while_connected_then_404_on_disconnect");
        return;
    };

    let domain = "connector-card-e2e.local";
    cleanup_domain(&pool, domain).await;
    let run = format!("{:x}", uuid::Uuid::new_v4().as_u128());
    let agent_local = format!("coder-{run}");
    let agent_addr = format!("{agent_local}@{domain}");
    let agent = make_bound_account(&pool, &agent_addr, domain).await;

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

    let http = reqwest::Client::new();
    let ready_url = format!("http://127.0.0.1:{port}/health/ready");
    let mut ready = false;
    for _ in 0..150 {
        if let Ok(resp) = http.get(&ready_url).send().await {
            if resp.status().is_success() { ready = true; break; }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if !ready {
        let _ = child.kill().await;
        cleanup_domain(&pool, domain).await;
        panic!("server did not become ready in 15s");
    }

    // Connect as the agent and complete the signed handshake with a `card`.
    let ws_url = format!("ws://127.0.0.1:{port}/ws");
    let (ws_stream, _) = tokio::time::timeout(Duration::from_secs(5), tokio_tungstenite::connect_async(&ws_url))
        .await.expect("ws connect timed out").expect("ws connect");
    let (mut tx, mut rx) = ws_stream.split();

    let (init_sig, init_ts) = signing::sign_init(&agent.key_pair, &agent.address).unwrap();
    let init_req = protocol::new_request(
        "ainm.initialize",
        serde_json::json!({
            "identity": agent.address,
            "protocolVersion": "0.1.0",
            "signature": init_sig,
            "timestamp": init_ts,
            "capabilities": {
                "streaming": true,
                "subThreads": true,
                "contentTypes": ["text", "code"],
                "participant_type": "agent",
                "toolVisibility": true
            },
            "card": {
                "name": "Coder",
                "description": "Writes and reviews code.",
                "version": "2.1.0",
                "skills": [{ "name": "Review", "description": "Reviews diffs." }],
                "tools": [{ "name": "run_tests", "description": "Runs tests.", "parameters": { "type": "object" } }]
            }
        }),
        serde_json::json!(2),
    );
    tx.send(Message::Text(serde_json::to_string(&init_req).unwrap().into())).await.unwrap();
    let init_resp = next_text(&mut rx, Duration::from_secs(10)).await.expect("init response");
    let init_resp_json: serde_json::Value = serde_json::from_str(&init_resp).unwrap();
    assert!(init_resp_json.get("result").is_some(), "handshake rejected: {init_resp}");
    let initialized = protocol::new_notification("ainm.initialized", serde_json::json!({}));
    tx.send(Message::Text(serde_json::to_string(&initialized).unwrap().into())).await.unwrap();

    // Give the server a moment to register the card, then GET the well-known route.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let card_url = format!("http://127.0.0.1:{port}/.well-known/ainm/{agent_local}/agent-card.json");
    let resp = http.get(&card_url).send().await.expect("GET connector card");
    assert_eq!(resp.status().as_u16(), 200, "connector card must be 200 while connected");
    let body: serde_json::Value = resp.json().await.expect("card JSON");
    assert_eq!(body["name"], "Coder", "got {body}");
    assert_eq!(body["ainm"]["address"], agent_addr, "got {body}");
    assert_eq!(body["ainm"]["tools"][0]["name"], "run_tests", "tool must be published; got {body}");
    assert_eq!(body["capabilities"]["pushNotifications"], false, "camelCase key; got {body}");

    // Disconnect: drop the WS, then the route must 404 once cleanup runs.
    drop(tx);
    drop(rx);
    let mut got_404 = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Ok(r) = http.get(&card_url).send().await {
            if r.status().as_u16() == 404 { got_404 = true; break; }
        }
    }

    let _ = child.kill().await;
    cleanup_domain(&pool, domain).await;
    assert!(got_404, "route must 404 within ~5s after the agent disconnects");
}
```

- [ ] **Step 2: Run the test**

Run: `cd server && cargo test --test connector_card_route`
Expected: with a DB configured, the test connects, asserts 200 + tool, disconnects, asserts 404. Without a DB it prints the skip line and returns (passes). Confirm it COMPILES and is green either way. Note: `identity::generate_keypair`, `identity::Identity::public_key_bytes`, and `signing::sign_init` mirror `ws_conversation_e2e.rs`; if any helper name differs, align it to that file (it's the canonical reference for this harness).

- [ ] **Step 3: Commit**

```bash
rtk proxy git add server/tests/connector_card_route.rs
rtk proxy git commit -m "test(server): connector card served while connected, 404 after disconnect"
```

---

## Task 6: Full suites green + Definition of Done

Verification only. No code changes unless a clear, trivial fix is needed (commit any such fix with `fix(...)`).

**Files:** none.

- [ ] **Step 1: Connector suite**

Run: `cd packages/connector && npm test`
Expected: all vitest tests pass (handshake incl. the new card test, agent, rpc, streaming, signing, memory, e2e-server).

- [ ] **Step 2: Server suite**

Run: `cd server && cargo test`
Expected: green except the two known pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`). New `build_connector_card` unit tests pass; `connector_card_route` passes or skips (no DB); `agent_card_route` still passes.

- [ ] **Step 3: Server clippy (touched files only)**

Run: `cd server && cargo clippy --bin ainm-server -- -D warnings 2>&1 | tail -30`
Expected: no warnings attributable to `agent_card.rs` or `server.rs` changes. (Pre-existing repo-wide clippy debt in untouched files — compiler.rs, graph.rs, etc. — is out of scope; distinguish clearly if `--all-targets` surfaces it.)

- [ ] **Step 4: iOS decode test unaffected**

Run: `xcodebuild -project ios/AINMMessenger/AINMMessenger.xcodeproj -scheme AINMMessenger -destination 'platform=iOS Simulator,name=iPhone 16' -derivedDataPath /tmp/ainm-dd test -only-testing:AINMMessengerTests/AgentCardDecodeTests 2>&1 | tail -6`
Expected: `** TEST SUCCEEDED **` — the connector card shares the canonical camelCase shape the decoder already handles; no iOS change needed.

- [ ] **Step 5: Confirm DoD (spec §9)**

1. Connector sends a `card` object in `ainm.initialize`; unit-tested. ✅ (Task 1)
2. `build_connector_card` schema-conformant camelCase with best-effort tools; unit-tested. ✅ (Task 2)
3. Connected agent's card served (200); 404 after disconnect; locked by integration test. ✅ (Tasks 3–5)
4. `published_card_for` async + registry branch; hosted/static/404 contract preserved. ✅ (Task 4)
5. Server suite green except the two known failures; connector tests green. ✅ (Steps 1–2)
6. Live render — Task 7 (manual).
7. Documented limitations (fed parity, persisted/offline cards, DB-configurable) remain future — in spec §1.

No commit unless a fix was needed.

---

## Task 7: MANUAL live verification (human-run)

Requires a running DB-backed server, a real connector agent, and the iOS app.

**Runbook (human):**

1. Start a DB-backed server headless (agent mode keeps it up; provisioning is DB-gated):
   `AINM_DATABASE__URL=postgres://ainm:dev_password@localhost:5432/ainm cargo run -p ainm-server -- --mode agent --identity localhost --port 8080 --cert server/certs/server-a.pem --key server/certs/server-a-key.pem --ca-cert server/certs/ca.pem`
2. Run a connector agent (e.g. `packages/connector-claude-code` or `packages/connector-openai`) configured with `name`, `description`, `skills`, registered tools, and `identity = <name>@localhost`, pointed at `ws://localhost:8080`. (The agent's address must be a registered account on that server — onboard it as needed.)
3. With the agent connected, `curl -s http://localhost:8080/.well-known/ainm/<name>/agent-card.json | python3 -m json.tool` → 200 with the real `name`/`description`, camelCase keys, and the agent's tools under `ainm.tools`.
4. In the iOS app (against `ws://localhost:8080`), add the agent by `<name>@localhost`, confirm the preview shows the real card (description, skills, capability badges), save it, start a thread, send a message, confirm a reply.
5. Stop the connector; confirm the curl now 404s and the app reports the agent unreachable (the saved contact falls back to its cached snapshot).

- [ ] **Step 1:** Human runs the runbook and confirms. Mark complete when verified.
