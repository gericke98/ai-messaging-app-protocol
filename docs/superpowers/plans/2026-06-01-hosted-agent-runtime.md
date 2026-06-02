# Hosted Agent Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `ainm-server` run a genuine, multi-step, tool-calling agent loop in-process for one built-in first-party agent (`assistant@<server-domain>`), so every memory read and tool call passes through our consent gate before it happens.

**Architecture:** A new `runtime.rs` runs a bounded LLM↔tool loop. Tools are exposed through a `ToolProvider` trait with two impls: in-process memory tools (`tools.rs`) and a curated-MCP bridge (`mcp.rs`, `rmcp` 1.7 client). The agent has a server-held Ed25519 keypair + auto-provisioned account (`hosted_agent.rs` + a `hosted_agents` table). The loop is invoked from the existing message-delivery path; replies stream back through the existing `ainm.stream.*` path signed by the agent key. Both consent gates — memory visibility and the offered tool set — are keyed on the requester's resolved consent scopes (reused from sub-project A).

**Tech Stack:** Rust 2024 / Axum / sqlx / PostgreSQL / `ring` (Ed25519) / `reqwest` (LLM HTTP) / `rmcp` 1.7 (MCP client) / `serde_json`. Tests: `cargo test` + `MockLlmClient`/`MockToolProvider`, DB-backed integration tests under `server/tests/`.

**Source spec:** `docs/superpowers/specs/2026-06-01-hosted-agent-runtime-design.md`

---

## Conventions for the implementer

- **Working directory** is the repo root: `/Users/santiagogerickeparga/Proyectos/ai-native-messaging`. The server crate is at `server/` (crate name `ainm_server`).
- **Run the server test suite** with `cargo test -p ainm-server`. Integration tests that need a DB read `DATABASE_URL` / `AINM_DATABASE__URL`; they are `#[ignore]`-gated or skip cleanly when no DB is present (see existing `server/tests/`). Pure unit tests run without a DB.
- **Two known pre-existing failures** are acceptable per the spec DoD: `test_graph_traversal` and `test_compilation_dedup_updates_confidence`. Do not try to fix them.
- **Branch:** create `feat/hosted-agent-runtime` off `main` before Task B-1 (the controller handles this via the worktree skill). Never start on `main`.
- **Module visibility:** new modules must be added to `server/src/lib.rs` with `pub mod` (not only `main.rs`) so integration tests under `server/tests/` can reference `ainm_server::runtime`, `ainm_server::tools`, etc. This bit us in sub-project A.
- **Never log** the agent's private key bytes.
- **Commit** after each task's tests pass, using the message in that task's final step.

### Reference signatures discovered in the codebase (do not re-derive — these are ground truth)

```rust
// server/src/identity.rs
pub struct Identity {
    pub address: String,
    pub key_pair: std::sync::Arc<ring::signature::Ed25519KeyPair>,
    pub public_key_bytes: Vec<u8>,
}
impl Identity { pub fn public_key_b64(&self) -> String; }
pub fn generate_keypair(address: &str) -> anyhow::Result<Identity>; // uses Ed25519KeyPair::generate_pkcs8

// server/src/signing.rs
pub fn sign_envelope(key_pair: &ring::signature::Ed25519KeyPair, envelope: &mut serde_json::Value) -> anyhow::Result<String>;

// server/src/accounts.rs
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AccountRow { pub id: uuid::Uuid, pub username: String, pub domain: String,
    pub address: String, pub email_hash: String, pub email_verified: bool, pub status: String }
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AccountKeyRow { pub id: uuid::Uuid, pub account_id: uuid::Uuid,
    pub public_key: String, pub device_label: Option<String> }
pub async fn find_account_by_address(pool: &sqlx::PgPool, address: &str) -> Result<Option<AccountRow>, StorageError>;
pub async fn find_active_key(pool: &sqlx::PgPool, account_id: uuid::Uuid) -> Result<Option<AccountKeyRow>, StorageError>;
pub async fn insert_account_key(pool: &sqlx::PgPool, account_id: uuid::Uuid, public_key: &str, device_label: Option<&str>) -> Result<(), StorageError>;

// server/src/briefing.rs
pub enum ConsentLevel { Minimal, Standard, Full }   // Copy
pub async fn resolve_scopes(db: &sqlx::PgPool, user_addr: &str, agent_addr: &str, default_consent: ConsentLevel)
    -> Result<(Vec<String>, ConsentLevel), AppError>;      // returns (granted_scopes, effective_level)
pub async fn generate_briefing(db: &sqlx::PgPool, user_addr: &str, agent_addr: &str, thread_id: &str, default_consent: ConsentLevel)
    -> Result<serde_json::Value, AppError>;
// scope_to_categories(scope: &str) -> Vec<&'static str>  -- currently PRIVATE; Task B-6 makes it pub.

// server/src/graph.rs
pub async fn query_nodes(pool: &sqlx::PgPool, user_addr: &str, text_query: Option<&str>,
    type_filter: Option<&[&str]>, category_filter: Option<&[&str]>, temporal_filter: &str,
    min_confidence: f32, limit: i64) -> Result<Vec<NodeRow>, StorageError>;
pub async fn insert_node(pool: &sqlx::PgPool, user_addr: &str, node_type: &str, category: &str,
    label: &str, properties: &serde_json::Value, confidence: f32, provenance: &serde_json::Value,
    tags: &serde_json::Value) -> Result<uuid::Uuid, StorageError>;

// server/src/llm.rs
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn extract(&self, messages: &[ConversationMessage]) -> Result<ExtractionResult, AppError>;
}
pub struct OpenAiClient { /* http, api_key, model, base_url */ }
impl OpenAiClient { pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self; }
pub struct MockLlmClient { pub results: std::sync::Mutex<Vec<ExtractionResult>> }

// server/src/error.rs
pub enum AppError { Storage(StorageError), Protocol { code: i32, message: String }, Unauthorized(String), NoDB }

// server/src/server.rs
#[derive(Clone)] pub struct ServerState { /* identity_store, agent_card, to_agent_tx, human_registry,
    agent_registry, threads, archival_store, federation, db: Option<PgPool>, compile_tx, compile_notify_tx,
    email_sender, token_signer, server_domain: String */ }
// HumanRegistry::send_to_thread(&self, thread_id: &str, msg: &str)   -- delivers stream/message frames to humans in a thread
```

LLM HTTP detail: `OpenAiClient` POSTs to `{base_url}/chat/completions` with header `Authorization: Bearer {api_key}`; non-200 → `AppError::Protocol { code: -32050, message }`. Config env: `AINM_LLM__API_KEY`, `AINM_LLM__MODEL` (default `gpt-4o-mini`), `AINM_LLM__BASE_URL` (default `https://api.openai.com/v1`).

Migrations: `server/migrations/NNN_description.sql`, run by `sqlx::migrate::Migrator::new(Path::new("migrations"))` in `server/src/db.rs::init_pool`. Latest is `003_accounts.sql`; the next is `004_hosted_agents.sql`.

---

## Wave 1 — Agent identity, table, and idempotent provisioning

### Task B-1: `hosted_agents` table + repository

**Files:**
- Create: `server/migrations/004_hosted_agents.sql`
- Create: `server/src/hosted_agent.rs`
- Modify: `server/src/lib.rs` (add `pub mod hosted_agent;`)
- Test: unit tests inline in `server/src/hosted_agent.rs` (pure row mapping) + DB behaviour covered by integration Task B-11.

- [ ] **Step 1: Write the migration**

Create `server/migrations/004_hosted_agents.sql`:

```sql
-- Hosted first-party agent: server-held identity + config.
-- v1 holds exactly one row (assistant@<server-domain>).
CREATE TABLE hosted_agents (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    address       TEXT NOT NULL UNIQUE,
    public_key    TEXT NOT NULL,              -- base64url(no-pad) Ed25519 public key
    private_key   BYTEA NOT NULL,             -- PKCS#8 v2 document bytes (server-held secret)
    system_prompt TEXT NOT NULL,
    enabled       BOOLEAN NOT NULL DEFAULT true,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

- [ ] **Step 2: Write the failing test for row mapping**

In `server/src/hosted_agent.rs`, add at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosted_agent_row_exposes_fields() {
        let row = HostedAgentRow {
            id: uuid::Uuid::nil(),
            address: "assistant@ainm.local".to_string(),
            public_key: "pk".to_string(),
            private_key: vec![1, 2, 3],
            system_prompt: "be helpful".to_string(),
            enabled: true,
        };
        assert_eq!(row.address, "assistant@ainm.local");
        assert!(row.enabled);
        assert_eq!(row.private_key, vec![1, 2, 3]);
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p ainm-server hosted_agent_row_exposes_fields`
Expected: FAIL — module/type does not exist yet.

- [ ] **Step 4: Implement the module**

Create `server/src/hosted_agent.rs` (top portion — the repository; provisioning + signer come in B-2):

```rust
//! Hosted first-party agent: server-held identity, config, and startup provisioning.
//!
//! The agent's Ed25519 private key (a PKCS#8 document) is stored server-side. This is
//! a legitimately server-held key for OUR first-party agent — distinct from the
//! zero-knowledge human key escrow. Never log the private key bytes.

use crate::error::StorageError;
use sqlx::PgPool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct HostedAgentRow {
    pub id: uuid::Uuid,
    pub address: String,
    pub public_key: String,
    pub private_key: Vec<u8>,
    pub system_prompt: String,
    pub enabled: bool,
}

/// Fetch the hosted agent by address, if it exists.
pub async fn find_by_address(
    pool: &PgPool,
    address: &str,
) -> Result<Option<HostedAgentRow>, StorageError> {
    let row = sqlx::query_as::<_, HostedAgentRow>(
        "SELECT id, address, public_key, private_key, system_prompt, enabled \
         FROM hosted_agents WHERE address = $1",
    )
    .bind(address)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Insert a new hosted agent row. Caller ensures idempotency.
pub async fn insert(
    pool: &PgPool,
    address: &str,
    public_key: &str,
    private_key: &[u8],
    system_prompt: &str,
) -> Result<HostedAgentRow, StorageError> {
    let row = sqlx::query_as::<_, HostedAgentRow>(
        "INSERT INTO hosted_agents (address, public_key, private_key, system_prompt) \
         VALUES ($1, $2, $3, $4) \
         RETURNING id, address, public_key, private_key, system_prompt, enabled",
    )
    .bind(address)
    .bind(public_key)
    .bind(private_key)
    .bind(system_prompt)
    .fetch_one(pool)
    .await?;
    Ok(row)
}
```

Add to `server/src/lib.rs` (alphabetical position, after `pub mod graph;` / before `pub mod identity;`):

```rust
pub mod hosted_agent;
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p ainm-server hosted_agent_row_exposes_fields`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add server/migrations/004_hosted_agents.sql server/src/hosted_agent.rs server/src/lib.rs
git commit -m "feat(b): hosted_agents table + repository (B-1)"
```

---

### Task B-2: Idempotent startup provisioning + `HostedAgent` signer

**Files:**
- Modify: `server/src/hosted_agent.rs` (add `HostedAgent` struct + `provision`)
- Test: unit test inline (keypair round-trip); DB idempotency covered in B-11.

The provisioning must, given a `PgPool` and `server_domain`:
1. Ensure an `accounts` row `username='assistant'`, `domain=server_domain`, `status='active'`, `email_verified=true`. There is no `create_active_account` helper, so insert directly (mirroring `003_accounts.sql` columns) with `ON CONFLICT (address) DO NOTHING`, then look it up.
2. Ensure an `account_keys` row binding the agent's Ed25519 public key (so the agent passes the handshake gate). Reuse `accounts::insert_account_key`, guarded by `accounts::find_active_key`.
3. Ensure a `hosted_agents` row holding the PKCS#8 private key bytes + system prompt.
4. Return a `HostedAgent` (address + reconstructed `ring::signature::Ed25519KeyPair` + public key bytes) used to sign outbound envelopes.

The keypair must be stored as the PKCS#8 document so it can be reconstructed across restarts. `generate_keypair` in `identity.rs` discards the PKCS#8 doc, so provisioning generates the keypair here to keep the bytes.

- [ ] **Step 1: Write the failing test (PKCS#8 round-trip + signer)**

Append to `server/src/hosted_agent.rs` tests module:

```rust
    #[test]
    fn keypair_roundtrips_through_pkcs8_and_signs() {
        use ring::rand::SystemRandom;
        use ring::signature::{Ed25519KeyPair, KeyPair};

        let rng = SystemRandom::new();
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let pkcs8_bytes = pkcs8.as_ref().to_vec();

        // Reconstruct (as provisioning does on restart from stored bytes).
        let kp = Ed25519KeyPair::from_pkcs8(&pkcs8_bytes).unwrap();
        let pk_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(kp.public_key().as_ref());

        let agent = HostedAgent {
            address: "assistant@ainm.local".to_string(),
            key_pair: std::sync::Arc::new(kp),
            public_key_bytes: base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(&pk_b64).unwrap(),
            system_prompt: "be helpful".to_string(),
        };

        // Signs an envelope and sets a signature field.
        let mut env = serde_json::json!({ "method": "ainm.message.send", "params": {} });
        let sig = crate::signing::sign_envelope(&agent.key_pair, &mut env).unwrap();
        assert!(!sig.is_empty());
        assert_eq!(env["signature"].as_str().unwrap(), sig);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p ainm-server keypair_roundtrips_through_pkcs8_and_signs`
Expected: FAIL — `HostedAgent` does not exist.

- [ ] **Step 3: Implement `HostedAgent` + `provision`**

Add to the top of `server/src/hosted_agent.rs` imports:

```rust
use base64::Engine;
use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair};
use std::sync::Arc;
```

Add the struct + provisioning function (after `insert`):

```rust
/// The live, in-process hosted agent: identity + signer + prompt.
#[derive(Clone)]
pub struct HostedAgent {
    pub address: String,
    pub key_pair: Arc<Ed25519KeyPair>,
    pub public_key_bytes: Vec<u8>,
    pub system_prompt: String,
}

const DEFAULT_SYSTEM_PROMPT: &str = "You are the AINM assistant, a helpful first-party agent. \
You may use the available tools to recall what you are permitted to know about the person you are \
talking with, and to take actions on their behalf. Only use information returned by your tools. \
Be concise and useful.";

/// Idempotently provision the hosted agent for `server_domain`.
/// Safe to call on every startup; creates rows only when missing.
pub async fn provision(pool: &PgPool, server_domain: &str) -> Result<HostedAgent, StorageError> {
    let address = format!("assistant@{server_domain}");

    // (1) ensure active, verified account row
    sqlx::query(
        "INSERT INTO accounts (username, domain, address, email_hash, email_verified, status) \
         VALUES ('assistant', $1, $2, $3, true, 'active') \
         ON CONFLICT (address) DO NOTHING",
    )
    .bind(server_domain)
    .bind(&address)
    // email_hash is UNIQUE NOT NULL; derive a stable non-email sentinel for the agent.
    .bind(format!("hosted-agent:{address}"))
    .execute(pool)
    .await?;

    let account = crate::accounts::find_account_by_address(pool, &address)
        .await?
        .ok_or_else(|| StorageError::NotFound("hosted agent account".into()))?;

    // (3 first, so we can reuse its key for (2)) ensure hosted_agents row
    let row = match find_by_address(pool, &address).await? {
        Some(existing) => existing,
        None => {
            let rng = SystemRandom::new();
            let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng)
                .map_err(|e| StorageError::Other(format!("keygen failed: {e}")))?;
            let kp = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref())
                .map_err(|e| StorageError::Other(format!("keypair load failed: {e}")))?;
            let pk_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(kp.public_key().as_ref());
            insert(pool, &address, &pk_b64, pkcs8.as_ref(), DEFAULT_SYSTEM_PROMPT).await?
        }
    };

    // (2) ensure an active account_keys row for the agent's public key
    if crate::accounts::find_active_key(pool, account.id).await?.is_none() {
        crate::accounts::insert_account_key(pool, account.id, &row.public_key, Some("hosted-agent"))
            .await?;
    }

    // Build the live signer from stored PKCS#8 bytes.
    let key_pair = Ed25519KeyPair::from_pkcs8(&row.private_key)
        .map_err(|e| StorageError::Other(format!("stored keypair load failed: {e}")))?;
    let public_key_bytes = key_pair.public_key().as_ref().to_vec();

    tracing::info!("[hosted-agent] provisioned {} (enabled={})", address, row.enabled);

    Ok(HostedAgent {
        address,
        key_pair: Arc::new(key_pair),
        public_key_bytes,
        system_prompt: row.system_prompt,
    })
}
```

If `StorageError` has no `NotFound`/`Other` variants, use whichever string-bearing variant exists (check `server/src/error.rs`); the implementer adapts the two `.map_err`/`ok_or_else` lines to the real variants. Do not invent new error plumbing — reuse what's there.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p ainm-server keypair_roundtrips_through_pkcs8_and_signs`
Expected: PASS.

- [ ] **Step 5: Run the full unit suite + clippy**

Run: `cargo test -p ainm-server --lib && cargo clippy -p ainm-server --all-targets`
Expected: builds; no new clippy warnings in `hosted_agent.rs`.

- [ ] **Step 6: Commit**

```bash
git add server/src/hosted_agent.rs
git commit -m "feat(b): idempotent hosted-agent provisioning + signer (B-2)"
```

---

## Wave 2 — LLM tool-use

### Task B-3: `chat_with_tools` types + trait method + `OpenAiClient` impl

**Files:**
- Modify: `server/src/llm.rs`
- Test: unit tests inline in `server/src/llm.rs`.

Design: a single request/response method (no provider-side SSE parsing). The runtime loop calls it repeatedly. It takes the running message list + offered tool schemas and returns one assistant turn that is either final text or a set of tool calls.

- [ ] **Step 1: Write the failing test (serialization shape)**

Add to the `tests` module in `server/src/llm.rs`:

```rust
    #[test]
    fn chat_message_serializes_openai_shape() {
        let m = ChatMessage::user("hello");
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["role"], "user");
        assert_eq!(v["content"], "hello");

        let tool_call = ToolCall {
            id: "call_1".into(),
            name: "query_memory".into(),
            arguments: serde_json::json!({"q": "x"}),
        };
        let assistant = ChatMessage::assistant_tool_calls(vec![tool_call.clone()]);
        let av = serde_json::to_value(&assistant).unwrap();
        assert_eq!(av["role"], "assistant");
        assert_eq!(av["tool_calls"][0]["function"]["name"], "query_memory");

        let tool_result = ChatMessage::tool_result("call_1", "the answer");
        let tv = serde_json::to_value(&tool_result).unwrap();
        assert_eq!(tv["role"], "tool");
        assert_eq!(tv["tool_call_id"], "call_1");
    }

    #[test]
    fn assistant_turn_distinguishes_text_from_tool_calls() {
        let text = AssistantTurn::Text("done".into());
        assert!(matches!(text, AssistantTurn::Text(_)));
        let calls = AssistantTurn::ToolCalls(vec![ToolCall {
            id: "c".into(), name: "n".into(), arguments: serde_json::json!({}),
        }]);
        assert!(matches!(calls, AssistantTurn::ToolCalls(_)));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p ainm-server chat_message_serializes_openai_shape`
Expected: FAIL — types don't exist.

- [ ] **Step 3: Implement the types + trait method + OpenAiClient impl**

Add near the top of `server/src/llm.rs` (after existing imports):

```rust
use serde::ser::SerializeStruct;
```

Add the tool-use types:

```rust
/// A tool the agent may call, advertised to the LLM (OpenAI function shape).
#[derive(Debug, Clone, Serialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    /// JSON Schema object for the tool's arguments.
    pub parameters: serde_json::Value,
}

/// A tool call requested by the model.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// One message in the running chat (system/user/assistant/tool).
#[derive(Debug, Clone)]
pub enum ChatMessage {
    System(String),
    User(String),
    AssistantText(String),
    AssistantToolCalls(Vec<ToolCall>),
    ToolResult { tool_call_id: String, content: String },
}

impl ChatMessage {
    pub fn system(s: impl Into<String>) -> Self { ChatMessage::System(s.into()) }
    pub fn user(s: impl Into<String>) -> Self { ChatMessage::User(s.into()) }
    pub fn assistant_text(s: impl Into<String>) -> Self { ChatMessage::AssistantText(s.into()) }
    pub fn assistant_tool_calls(calls: Vec<ToolCall>) -> Self { ChatMessage::AssistantToolCalls(calls) }
    pub fn tool_result(id: impl Into<String>, content: impl Into<String>) -> Self {
        ChatMessage::ToolResult { tool_call_id: id.into(), content: content.into() }
    }
}

impl Serialize for ChatMessage {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            ChatMessage::System(c) => {
                let mut st = s.serialize_struct("m", 2)?;
                st.serialize_field("role", "system")?;
                st.serialize_field("content", c)?;
                st.end()
            }
            ChatMessage::User(c) => {
                let mut st = s.serialize_struct("m", 2)?;
                st.serialize_field("role", "user")?;
                st.serialize_field("content", c)?;
                st.end()
            }
            ChatMessage::AssistantText(c) => {
                let mut st = s.serialize_struct("m", 2)?;
                st.serialize_field("role", "assistant")?;
                st.serialize_field("content", c)?;
                st.end()
            }
            ChatMessage::AssistantToolCalls(calls) => {
                let tool_calls: Vec<serde_json::Value> = calls.iter().map(|c| {
                    serde_json::json!({
                        "id": c.id,
                        "type": "function",
                        "function": { "name": c.name, "arguments": c.arguments.to_string() }
                    })
                }).collect();
                let mut st = s.serialize_struct("m", 3)?;
                st.serialize_field("role", "assistant")?;
                st.serialize_field("content", &serde_json::Value::Null)?;
                st.serialize_field("tool_calls", &tool_calls)?;
                st.end()
            }
            ChatMessage::ToolResult { tool_call_id, content } => {
                let mut st = s.serialize_struct("m", 3)?;
                st.serialize_field("role", "tool")?;
                st.serialize_field("tool_call_id", tool_call_id)?;
                st.serialize_field("content", content)?;
                st.end()
            }
        }
    }
}

/// The result of one LLM turn.
#[derive(Debug, Clone)]
pub enum AssistantTurn {
    Text(String),
    ToolCalls(Vec<ToolCall>),
}
```

Add the trait method to the `LlmClient` trait (default impl returns an error so existing impls aren't forced to implement it until they do):

```rust
    /// Run one chat turn with tools available. Returns either final assistant text
    /// or the tool calls the model wants executed. Single request/response (the runtime
    /// loop calls this repeatedly).
    async fn chat_with_tools(
        &self,
        _messages: &[ChatMessage],
        _tools: &[ToolSchema],
    ) -> Result<AssistantTurn, AppError> {
        Err(AppError::Protocol { code: -32601, message: "chat_with_tools not supported".into() })
    }
```

Implement it for `OpenAiClient` (inside its `impl LlmClient for OpenAiClient` block):

```rust
    async fn chat_with_tools(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolSchema],
    ) -> Result<AssistantTurn, AppError> {
        let tools_json: Vec<serde_json::Value> = tools.iter().map(|t| {
            serde_json::json!({
                "type": "function",
                "function": { "name": t.name, "description": t.description, "parameters": t.parameters }
            })
        }).collect();

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.1,
        });
        if !tools_json.is_empty() {
            body["tools"] = serde_json::Value::Array(tools_json);
            body["tool_choice"] = serde_json::json!("auto");
        }

        let resp = self.http
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Protocol { code: -32050, message: format!("LLM request failed: {e}") })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Protocol { code: -32050, message: format!("LLM returned {status}: {text}") });
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| AppError::Protocol {
            code: -32050, message: format!("LLM parse failed: {e}"),
        })?;

        let message = &json["choices"][0]["message"];
        if let Some(tcs) = message["tool_calls"].as_array() {
            if !tcs.is_empty() {
                let calls = tcs.iter().filter_map(|tc| {
                    let id = tc["id"].as_str()?.to_string();
                    let name = tc["function"]["name"].as_str()?.to_string();
                    let raw = tc["function"]["arguments"].as_str().unwrap_or("{}");
                    let arguments = serde_json::from_str(raw).unwrap_or(serde_json::json!({}));
                    Some(ToolCall { id, name, arguments })
                }).collect::<Vec<_>>();
                return Ok(AssistantTurn::ToolCalls(calls));
            }
        }
        let text = message["content"].as_str().unwrap_or("").to_string();
        Ok(AssistantTurn::Text(text))
    }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p ainm-server chat_message_serializes_openai_shape assistant_turn_distinguishes_text_from_tool_calls`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/llm.rs
git commit -m "feat(b): chat_with_tools LLM tool-use types + OpenAiClient impl (B-3)"
```

---

### Task B-4: Extend `MockLlmClient` to script multi-turn tool use

**Files:**
- Modify: `server/src/llm.rs`
- Test: unit test inline.

`MockLlmClient` currently scripts `extract` results. Add a separate scripted queue for `chat_with_tools` so a test can say "turn 1 → emit this tool call; turn 2 → emit final text."

- [ ] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
    #[tokio::test]
    async fn mock_scripts_multi_turn_tool_use() {
        let mock = MockLlmClient::with_turns(vec![
            AssistantTurn::ToolCalls(vec![ToolCall {
                id: "call_1".into(), name: "query_memory".into(),
                arguments: serde_json::json!({"q": "prefs"}),
            }]),
            AssistantTurn::Text("Based on your prefs, done.".into()),
        ]);

        let t1 = mock.chat_with_tools(&[ChatMessage::user("hi")], &[]).await.unwrap();
        assert!(matches!(t1, AssistantTurn::ToolCalls(_)));
        let t2 = mock.chat_with_tools(&[ChatMessage::user("hi")], &[]).await.unwrap();
        match t2 { AssistantTurn::Text(s) => assert!(s.contains("done")), _ => panic!("expected text") }
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p ainm-server mock_scripts_multi_turn_tool_use`
Expected: FAIL — `with_turns` / scripted tool turns not present.

- [ ] **Step 3: Implement**

Add a field to `MockLlmClient` and a constructor + the trait method. Update the struct:

```rust
pub struct MockLlmClient {
    /// Pre-configured extraction results returned in order.
    pub results: std::sync::Mutex<Vec<ExtractionResult>>,
    /// Pre-configured chat_with_tools turns returned in order.
    pub turns: std::sync::Mutex<Vec<AssistantTurn>>,
}
```

Update existing constructors (`new`, `empty`) to also init `turns: std::sync::Mutex::new(vec![])`, and add:

```rust
impl MockLlmClient {
    /// Build a mock that scripts chat_with_tools turns (FIFO).
    pub fn with_turns(turns: Vec<AssistantTurn>) -> Self {
        Self {
            results: std::sync::Mutex::new(vec![]),
            turns: std::sync::Mutex::new(turns),
        }
    }
}
```

Add the trait method inside `impl LlmClient for MockLlmClient`:

```rust
    async fn chat_with_tools(
        &self,
        _messages: &[ChatMessage],
        _tools: &[ToolSchema],
    ) -> Result<AssistantTurn, AppError> {
        let mut turns = self.turns.lock().unwrap();
        if turns.is_empty() {
            Ok(AssistantTurn::Text(String::new()))
        } else {
            Ok(turns.remove(0))
        }
    }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p ainm-server mock_scripts_multi_turn_tool_use`
Expected: PASS.

- [ ] **Step 5: Run the full lib test suite**

Run: `cargo test -p ainm-server --lib`
Expected: PASS (minus the two known-failing tests if they are in `--lib`; they are integration — should be green here).

- [ ] **Step 6: Commit**

```bash
git add server/src/llm.rs
git commit -m "feat(b): MockLlmClient scripts multi-turn tool use (B-4)"
```

---

## Wave 3 — Tools + consent gate

### Task B-5: `ToolProvider` trait + `ToolResult` + registry

**Files:**
- Create: `server/src/tools.rs`
- Modify: `server/src/lib.rs` (add `pub mod tools;`)
- Test: unit tests inline.

- [ ] **Step 1: Write the failing test**

Create `server/src/tools.rs` with this test at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct EchoProvider;
    #[async_trait::async_trait]
    impl ToolProvider for EchoProvider {
        async fn list_tools(&self, _scopes: &[String]) -> Vec<crate::llm::ToolSchema> {
            vec![crate::llm::ToolSchema {
                name: "echo".into(),
                description: "echoes input".into(),
                parameters: serde_json::json!({"type":"object","properties":{"msg":{"type":"string"}}}),
            }]
        }
        async fn call(&self, name: &str, args: &serde_json::Value, _scopes: &[String]) -> ToolResult {
            if name == "echo" {
                ToolResult::ok(args["msg"].as_str().unwrap_or("").to_string())
            } else {
                ToolResult::error(format!("unknown tool: {name}"))
            }
        }
    }

    #[tokio::test]
    async fn registry_lists_and_dispatches() {
        let reg = ToolRegistry::new(vec![Box::new(EchoProvider)]);
        let scopes = vec!["identity".to_string()];
        let tools = reg.list_tools(&scopes).await;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");

        let res = reg.call("echo", &serde_json::json!({"msg": "hi"}), &scopes).await;
        assert!(!res.is_error);
        assert_eq!(res.content, "hi");

        let missing = reg.call("nope", &serde_json::json!({}), &scopes).await;
        assert!(missing.is_error);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p ainm-server registry_lists_and_dispatches`
Expected: FAIL — module/types absent.

- [ ] **Step 3: Implement the trait + registry**

Top of `server/src/tools.rs`:

```rust
//! Tool abstraction for the hosted agent runtime.
//!
//! A `ToolProvider` advertises tools (filtered to the requester's consent scopes) and
//! executes calls. The MCP client (mcp.rs) lives behind this same trait so sub-project D
//! can replace it with a sandboxed executor without touching the runtime loop.

use crate::llm::ToolSchema;

/// The result of a tool call, fed back to the model as a `tool` message.
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub content: String,
    pub is_error: bool,
}

impl ToolResult {
    pub fn ok(content: impl Into<String>) -> Self { Self { content: content.into(), is_error: false } }
    pub fn error(content: impl Into<String>) -> Self { Self { content: content.into(), is_error: true } }
}

/// A source of tools. Implementations: memory tools (in-process), MCP bridge.
#[async_trait::async_trait]
pub trait ToolProvider: Send + Sync {
    /// Tools this provider offers GIVEN the requester's granted scopes (already consent-filtered).
    async fn list_tools(&self, scopes: &[String]) -> Vec<ToolSchema>;
    /// Execute a tool. Implementations MUST re-check consent and refuse disallowed calls
    /// with `ToolResult::error` rather than executing.
    async fn call(&self, name: &str, args: &serde_json::Value, scopes: &[String]) -> ToolResult;
}

/// Aggregates multiple providers and dispatches by tool name.
pub struct ToolRegistry {
    providers: Vec<Box<dyn ToolProvider>>,
}

impl ToolRegistry {
    pub fn new(providers: Vec<Box<dyn ToolProvider>>) -> Self { Self { providers } }

    /// The union of all providers' consent-filtered tools.
    pub async fn list_tools(&self, scopes: &[String]) -> Vec<ToolSchema> {
        let mut out = Vec::new();
        for p in &self.providers {
            out.extend(p.list_tools(scopes).await);
        }
        out
    }

    /// Dispatch a call to the provider that offers `name`. If no provider offers it
    /// for these scopes, refuse with a tool-error (never silently execute).
    pub async fn call(&self, name: &str, args: &serde_json::Value, scopes: &[String]) -> ToolResult {
        for p in &self.providers {
            let offered = p.list_tools(scopes).await;
            if offered.iter().any(|t| t.name == name) {
                return p.call(name, args, scopes).await;
            }
        }
        ToolResult::error(format!("tool '{name}' is not available (not offered for current consent scopes)"))
    }
}
```

Add to `server/src/lib.rs`:

```rust
pub mod tools;
```

(alphabetical: after `pub mod signing;` — adjust to keep order; exact order is not enforced by the compiler.)

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p ainm-server registry_lists_and_dispatches`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/tools.rs server/src/lib.rs
git commit -m "feat(b): ToolProvider trait + ToolResult + registry (B-5)"
```

---

### Task B-6: Memory tools + scope filtering

**Files:**
- Modify: `server/src/briefing.rs` (make `scope_to_categories` `pub`)
- Create: `server/src/tools_memory.rs` (the memory `ToolProvider` impl)
- Modify: `server/src/lib.rs` (add `pub mod tools_memory;`)
- Test: unit tests inline (scope→category mapping, tool gating); DB-backed query behaviour covered in B-11.

Memory tools: `retrieve_briefing`, `query_memory`, `store_fact`. Each maps to a required scope; results are scope-filtered. The provider holds the `PgPool`, the requester `user_addr`, the agent `agent_addr`, and the default consent level.

Tool→required-scope map (consent gate, direction 2):
- `retrieve_briefing` → requires any granted scope (offered whenever scopes is non-empty).
- `query_memory` → requires any granted scope; results filtered to categories of granted scopes.
- `store_fact` → requires `preferences` scope (writing a preference/fact on the user's behalf is a higher-trust action), so a `Minimal`-scope requester is never offered `store_fact` (spec test §11).

- [ ] **Step 1: Make `scope_to_categories` public**

In `server/src/briefing.rs`, change:

```rust
fn scope_to_categories(scope: &str) -> Vec<&'static str> {
```

to:

```rust
pub fn scope_to_categories(scope: &str) -> Vec<&'static str> {
```

- [ ] **Step 2: Write the failing test**

Create `server/src/tools_memory.rs` with this test at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_scope_does_not_offer_store_fact() {
        // identity-only requester
        let scopes = vec!["identity".to_string()];
        let names = offered_tool_names(&scopes);
        assert!(names.contains(&"retrieve_briefing".to_string()));
        assert!(names.contains(&"query_memory".to_string()));
        assert!(!names.contains(&"store_fact".to_string()));
    }

    #[test]
    fn preferences_scope_offers_store_fact() {
        let scopes = vec!["identity".to_string(), "preferences".to_string()];
        let names = offered_tool_names(&scopes);
        assert!(names.contains(&"store_fact".to_string()));
    }

    #[test]
    fn empty_scopes_offers_nothing() {
        assert!(offered_tool_names(&[]).is_empty());
    }

    #[test]
    fn granted_categories_unions_scopes() {
        let scopes = vec!["identity".to_string(), "preferences".to_string()];
        let cats = granted_categories(&scopes);
        assert!(cats.contains(&"fact"));        // from identity
        assert!(cats.contains(&"preference"));  // from preferences
        assert!(!cats.contains(&"skill"));      // expertise not granted
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p ainm-server -- tools_memory`
Expected: FAIL — module/functions absent.

- [ ] **Step 4: Implement the memory provider**

`server/src/tools_memory.rs`:

```rust
//! In-process memory tools exposed to the hosted agent, consent-scoped per requester.

use crate::briefing::{self, ConsentLevel};
use crate::llm::ToolSchema;
use crate::tools::{ToolProvider, ToolResult};
use sqlx::PgPool;

/// Map each memory tool to the scope a requester must have been granted to be offered it.
/// `None` means "offered whenever the requester has any granted scope".
fn required_scope(tool: &str) -> Option<&'static str> {
    match tool {
        "retrieve_briefing" => None,
        "query_memory" => None,
        "store_fact" => Some("preferences"),
        _ => Some("__never__"),
    }
}

/// Names of tools offered for the given granted scopes (pure; used by the consent gate + tests).
pub fn offered_tool_names(scopes: &[String]) -> Vec<String> {
    if scopes.is_empty() {
        return vec![];
    }
    ["retrieve_briefing", "query_memory", "store_fact"]
        .into_iter()
        .filter(|t| match required_scope(t) {
            None => true,
            Some(req) => scopes.iter().any(|s| s == req),
        })
        .map(|s| s.to_string())
        .collect()
}

/// The union of memory categories visible for the granted scopes (pure; used to filter results).
pub fn granted_categories(scopes: &[String]) -> Vec<&'static str> {
    let mut cats: Vec<&'static str> = Vec::new();
    for s in scopes {
        for c in briefing::scope_to_categories(s) {
            if !cats.contains(&c) {
                cats.push(c);
            }
        }
    }
    cats
}

fn schema_for(name: &str) -> ToolSchema {
    match name {
        "retrieve_briefing" => ToolSchema {
            name: name.into(),
            description: "Retrieve the consent-scoped memory briefing about the person you are talking with.".into(),
            parameters: serde_json::json!({"type":"object","properties":{}}),
        },
        "query_memory" => ToolSchema {
            name: name.into(),
            description: "Search the person's memory graph for facts you are permitted to see.".into(),
            parameters: serde_json::json!({
                "type":"object",
                "properties":{"query":{"type":"string","description":"free-text search"}},
                "required":["query"]
            }),
        },
        "store_fact" => ToolSchema {
            name: name.into(),
            description: "Store a durable preference/fact about the person (only when they ask you to remember something).".into(),
            parameters: serde_json::json!({
                "type":"object",
                "properties":{
                    "label":{"type":"string"},
                    "value":{"type":"string"}
                },
                "required":["label","value"]
            }),
        },
        _ => ToolSchema { name: name.into(), description: String::new(), parameters: serde_json::json!({"type":"object"}) },
    }
}

pub struct MemoryToolProvider {
    pub db: PgPool,
    pub user_addr: String,
    pub agent_addr: String,
    pub default_consent: ConsentLevel,
}

#[async_trait::async_trait]
impl ToolProvider for MemoryToolProvider {
    async fn list_tools(&self, scopes: &[String]) -> Vec<ToolSchema> {
        offered_tool_names(scopes).iter().map(|n| schema_for(n)).collect()
    }

    async fn call(&self, name: &str, args: &serde_json::Value, scopes: &[String]) -> ToolResult {
        // Consent gate (direction 2): refuse any tool not in the offered set.
        if !offered_tool_names(scopes).iter().any(|t| t == name) {
            return ToolResult::error(format!("tool '{name}' is not permitted by current consent scopes"));
        }
        match name {
            "retrieve_briefing" => {
                match briefing::generate_briefing(
                    &self.db, &self.user_addr, &self.agent_addr, "", self.default_consent,
                ).await {
                    Ok(b) => ToolResult::ok(b.to_string()),
                    Err(e) => ToolResult::error(format!("briefing failed: {e}")),
                }
            }
            "query_memory" => {
                let q = args["query"].as_str().unwrap_or("");
                let cats = granted_categories(scopes);
                let cat_refs: Vec<&str> = cats.clone();
                match crate::graph::query_nodes(
                    &self.db, &self.user_addr, Some(q), None,
                    Some(&cat_refs), "current", 0.0, 10,
                ).await {
                    Ok(nodes) => {
                        // Defense in depth: filter again to granted categories.
                        let visible: Vec<_> = nodes.into_iter()
                            .filter(|n| cats.iter().any(|c| *c == n.category.as_str()))
                            .map(|n| serde_json::json!({"label": n.label, "category": n.category, "confidence": n.confidence}))
                            .collect();
                        ToolResult::ok(serde_json::json!({"results": visible}).to_string())
                    }
                    Err(e) => ToolResult::error(format!("query failed: {e}")),
                }
            }
            "store_fact" => {
                let label = args["label"].as_str().unwrap_or("");
                let value = args["value"].as_str().unwrap_or("");
                if label.is_empty() {
                    return ToolResult::error("label is required".into());
                }
                let props = serde_json::json!({"value": value});
                let prov = serde_json::json!({"source": "hosted-agent", "agent": self.agent_addr});
                match crate::graph::insert_node(
                    &self.db, &self.user_addr, "preference", "preference", label,
                    &props, 0.8, &prov, &serde_json::json!([]),
                ).await {
                    Ok(_) => ToolResult::ok(format!("stored: {label}")),
                    Err(e) => ToolResult::error(format!("store failed: {e}")),
                }
            }
            other => ToolResult::error(format!("unknown memory tool: {other}")),
        }
    }
}
```

`NodeRow` field names (`label`, `category`, `confidence`) must match `server/src/graph.rs`; if a field differs, adapt the `.map(...)`/filter lines to the real `NodeRow`. Do not change `graph.rs`.

Add to `server/src/lib.rs`:

```rust
pub mod tools_memory;
```

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p ainm-server -- tools_memory`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add server/src/briefing.rs server/src/tools_memory.rs server/src/lib.rs
git commit -m "feat(b): consent-scoped memory tools + scope filtering (B-6)"
```

---

## Wave 4 — Runtime loop + delivery wiring

### Task B-7: `runtime.rs` bounded tool-calling loop

**Files:**
- Create: `server/src/runtime.rs`
- Modify: `server/src/lib.rs` (add `pub mod runtime;`)
- Test: unit tests inline using `MockLlmClient` + a `MockToolProvider`.

`handle_turn` assembles the context (system prompt + scope-allowed tool schemas + recent history + inbound message), runs the LLM↔tool loop with a `max_tool_rounds` cap, and returns the final assistant text. **Streaming to the human and signing happen in B-8** (the delivery wiring); this task keeps the loop pure and testable by returning the final text + an event sink callback.

Design the loop to take collaborators as parameters (dependency injection) so it's unit-testable without a server:

- [ ] **Step 1: Write the failing test**

Create `server/src/runtime.rs` with this test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{AssistantTurn, MockLlmClient, ToolCall};
    use crate::tools::{ToolProvider, ToolRegistry, ToolResult};

    struct CannedProvider;
    #[async_trait::async_trait]
    impl ToolProvider for CannedProvider {
        async fn list_tools(&self, _scopes: &[String]) -> Vec<crate::llm::ToolSchema> {
            vec![crate::llm::ToolSchema {
                name: "query_memory".into(), description: "q".into(),
                parameters: serde_json::json!({"type":"object"}),
            }]
        }
        async fn call(&self, name: &str, _args: &serde_json::Value, _scopes: &[String]) -> ToolResult {
            if name == "query_memory" { ToolResult::ok("user likes Rust") }
            else { ToolResult::error("no") }
        }
    }

    #[tokio::test]
    async fn loop_runs_tool_then_finalizes() {
        let llm = MockLlmClient::with_turns(vec![
            AssistantTurn::ToolCalls(vec![ToolCall {
                id: "c1".into(), name: "query_memory".into(), arguments: serde_json::json!({"query":"prefs"}),
            }]),
            AssistantTurn::Text("You like Rust.".into()),
        ]);
        let reg = ToolRegistry::new(vec![Box::new(CannedProvider)]);
        let out = run_loop(
            &llm, &reg,
            "system", &["identity".to_string()],
            vec![], "tell me about myself",
            8,
        ).await;
        assert_eq!(out.final_text, "You like Rust.");
        assert_eq!(out.tool_rounds, 1);
    }

    #[tokio::test]
    async fn loop_stops_at_cap() {
        // LLM always asks for a tool → must stop at max_tool_rounds.
        let turns = (0..20).map(|_| AssistantTurn::ToolCalls(vec![ToolCall {
            id: "c".into(), name: "query_memory".into(), arguments: serde_json::json!({}),
        }])).collect();
        let llm = MockLlmClient::with_turns(turns);
        let reg = ToolRegistry::new(vec![Box::new(CannedProvider)]);
        let out = run_loop(&llm, &reg, "system", &["identity".into()], vec![], "go", 3).await;
        assert_eq!(out.tool_rounds, 3);
        assert!(out.stopped_early);
        assert!(!out.final_text.is_empty()); // a "stopped early" note
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p ainm-server -- runtime`
Expected: FAIL — module/functions absent.

- [ ] **Step 3: Implement the loop**

`server/src/runtime.rs` (top portion — the pure loop; the server entry point comes in B-8):

```rust
//! The hosted agent's bounded, consent-gated, tool-calling loop.

use crate::llm::{AssistantTurn, ChatMessage, LlmClient, ToolSchema};
use crate::tools::ToolRegistry;

/// Outcome of one full agent turn.
pub struct TurnOutcome {
    pub final_text: String,
    pub tool_rounds: usize,
    pub stopped_early: bool,
}

/// Run the LLM↔tool loop. Collaborators are injected so this is unit-testable.
/// `scopes` are the requester's granted consent scopes (gates the offered tool set).
pub async fn run_loop(
    llm: &dyn LlmClient,
    tools: &ToolRegistry,
    system_prompt: &str,
    scopes: &[String],
    history: Vec<ChatMessage>,
    inbound_user_text: &str,
    max_tool_rounds: usize,
) -> TurnOutcome {
    let offered: Vec<ToolSchema> = tools.list_tools(scopes).await;

    let mut messages: Vec<ChatMessage> = Vec::new();
    messages.push(ChatMessage::system(system_prompt));
    messages.extend(history);
    messages.push(ChatMessage::user(inbound_user_text));

    let mut rounds = 0usize;
    loop {
        let turn = match llm.chat_with_tools(&messages, &offered).await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("[runtime] LLM error: {e}");
                return TurnOutcome {
                    final_text: "Sorry — I hit an error contacting my model. Please try again.".into(),
                    tool_rounds: rounds,
                    stopped_early: true,
                };
            }
        };

        match turn {
            AssistantTurn::Text(text) => {
                return TurnOutcome { final_text: text, tool_rounds: rounds, stopped_early: false };
            }
            AssistantTurn::ToolCalls(calls) => {
                if rounds >= max_tool_rounds {
                    return TurnOutcome {
                        final_text: "I gathered some information but stopped early to avoid running too long. \
                                     Could you narrow the request?".into(),
                        tool_rounds: rounds,
                        stopped_early: true,
                    };
                }
                rounds += 1;
                // Record the assistant's tool-call message, then each tool result.
                messages.push(ChatMessage::assistant_tool_calls(calls.clone()));
                for call in calls {
                    let result = tools.call(&call.name, &call.arguments, scopes).await;
                    let payload = if result.is_error {
                        format!("ERROR: {}", result.content)
                    } else {
                        result.content
                    };
                    messages.push(ChatMessage::tool_result(&call.id, payload));
                }
            }
        }
    }
}
```

Add to `server/src/lib.rs`:

```rust
pub mod runtime;
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p ainm-server -- runtime`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/runtime.rs server/src/lib.rs
git commit -m "feat(b): bounded consent-gated tool-calling loop (B-7)"
```

---

### Task B-8: Wire the runtime into delivery — `ServerState` fields, `handle_turn`, streaming + signing

**Files:**
- Modify: `server/src/runtime.rs` (add `handle_turn` server entry point)
- Modify: `server/src/server.rs` (ServerState fields; invoke from delivery path; register agent participant)
- Modify: `server/src/main.rs` (build registry + provision agent + inject into state)
- Test: covered end-to-end by integration Task B-11; this task must keep the full build green.

This is the integration keystone. `handle_turn` ties the loop to the live server: it resolves the requester's scopes, builds the `MemoryToolProvider` for that requester, fetches recent thread history, runs `run_loop`, then **streams the final text to the human** via `ainm.stream.start`/`delta`/`end` and sends a final `ainm.message.send` envelope **signed by the agent key**, all through `HumanRegistry::send_to_thread`.

- [ ] **Step 1: Add `ServerState` fields**

In `server/src/server.rs`, add to the `ServerState` struct (after `server_domain`):

```rust
    /// The provisioned hosted agent (signer + prompt), if a DB-backed agent exists.
    pub hosted_agent: Option<std::sync::Arc<crate::hosted_agent::HostedAgent>>,
    /// LLM client for the hosted agent loop.
    pub llm_client: Option<std::sync::Arc<dyn crate::llm::LlmClient>>,
    /// Curated MCP tool providers (built at startup), shared across turns.
    pub mcp_providers: std::sync::Arc<Vec<std::sync::Arc<dyn crate::tools::ToolProvider>>>,
```

Update ALL THREE `ServerState { ... }` construction sites (`start`, `start_with_port`, `start_federation`) to set the new fields. For `start`/`start_with_port` (no DB / UAT), default them:

```rust
    hosted_agent: None,
    llm_client: None,
    mcp_providers: std::sync::Arc::new(vec![]),
```

For `start_federation`, thread them through as new function parameters (see Step 3).

- [ ] **Step 2: Implement `handle_turn` in `runtime.rs`**

Add to `server/src/runtime.rs`:

```rust
use crate::server::ServerState;

/// Server entry point: invoked from the delivery path when the hosted agent is a
/// participant in `thread_id` and `inbound` is addressed to/mentions it.
/// `requester` is the human whose consent scopes gate everything.
pub async fn handle_turn(
    state: &ServerState,
    thread_id: &str,
    requester: &str,
    inbound_user_text: &str,
) {
    let (Some(agent), Some(llm), Some(db)) =
        (state.hosted_agent.as_ref(), state.llm_client.as_ref(), state.db.as_ref())
    else {
        return; // hosted agent not provisioned → nothing to do
    };

    // Resolve the requester's granted scopes (consent gate spine).
    let (scopes, _level) = match crate::briefing::resolve_scopes(
        db, requester, &agent.address, crate::briefing::ConsentLevel::Standard,
    ).await {
        Ok(s) => s,
        Err(e) => { tracing::warn!("[runtime] resolve_scopes failed: {e}"); (vec![], crate::briefing::ConsentLevel::Minimal) }
    };

    // Build the per-requester tool registry: memory tools + curated MCP providers.
    let mem = crate::tools_memory::MemoryToolProvider {
        db: db.clone(),
        user_addr: requester.to_string(),
        agent_addr: agent.address.clone(),
        default_consent: crate::briefing::ConsentLevel::Standard,
    };
    let mut providers: Vec<Box<dyn crate::tools::ToolProvider>> = vec![Box::new(mem)];
    for p in state.mcp_providers.iter() {
        providers.push(Box::new(SharedProvider(p.clone())));
    }
    let registry = crate::tools::ToolRegistry::new(providers);

    // Recent thread history as chat messages (oldest→newest).
    let history = fetch_history_as_chat(db, thread_id, requester, &agent.address).await;

    let outcome = run_loop(
        llm.as_ref(), &registry, &agent.system_prompt, &scopes, history,
        inbound_user_text, 8,
    ).await;

    // Stream the final reply to the human and send the signed final envelope.
    stream_reply(state, agent, thread_id, &outcome.final_text).await;
}

/// Wrap an Arc<dyn ToolProvider> so it can join the Box<dyn> registry.
struct SharedProvider(std::sync::Arc<dyn crate::tools::ToolProvider>);
#[async_trait::async_trait]
impl crate::tools::ToolProvider for SharedProvider {
    async fn list_tools(&self, scopes: &[String]) -> Vec<ToolSchema> { self.0.list_tools(scopes).await }
    async fn call(&self, name: &str, args: &serde_json::Value, scopes: &[String]) -> crate::tools::ToolResult {
        self.0.call(name, args, scopes).await
    }
}

/// Load recent thread messages and map them to chat messages (the agent's own messages
/// become assistant turns; everyone else becomes user turns, prefixed with the sender).
async fn fetch_history_as_chat(
    db: &sqlx::PgPool, thread_id: &str, _requester: &str, agent_addr: &str,
) -> Vec<ChatMessage> {
    let rows: Vec<(String, serde_json::Value)> = sqlx::query_as(
        "SELECT from_addr, content FROM messages WHERE thread_id = $1 ORDER BY sequence ASC LIMIT 20",
    )
    .bind(thread_id)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    rows.into_iter().map(|(from, content)| {
        let text = content_to_text(&content);
        if from == agent_addr {
            ChatMessage::assistant_text(text)
        } else {
            ChatMessage::user(format!("{from}: {text}"))
        }
    }).collect()
}

/// Flatten a content block array to plain text.
fn content_to_text(content: &serde_json::Value) -> String {
    if let Some(arr) = content.as_array() {
        arr.iter()
            .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        content.as_str().unwrap_or("").to_string()
    }
}

/// Stream the reply to thread participants: start → one delta with full text → end,
/// then a final signed `ainm.message.send`.
async fn stream_reply(state: &ServerState, agent: &crate::hosted_agent::HostedAgent, thread_id: &str, text: &str) {
    let stream_id = uuid::Uuid::now_v7().to_string();

    let start = serde_json::json!({
        "jsonrpc": "2.0", "method": "ainm.stream.start",
        "params": {"stream_id": stream_id, "thread_id": thread_id, "from": agent.address}
    });
    state.human_registry.send_to_thread(thread_id, &start.to_string()).await;

    let delta = serde_json::json!({
        "jsonrpc": "2.0", "method": "ainm.stream.delta",
        "params": {"stream_id": stream_id, "thread_id": thread_id, "delta": text}
    });
    state.human_registry.send_to_thread(thread_id, &delta.to_string()).await;

    let end = serde_json::json!({
        "jsonrpc": "2.0", "method": "ainm.stream.end",
        "params": {"stream_id": stream_id, "thread_id": thread_id}
    });
    state.human_registry.send_to_thread(thread_id, &end.to_string()).await;

    // Final signed message envelope.
    let mut envelope = serde_json::json!({
        "jsonrpc": "2.0", "method": "ainm.message.deliver",
        "params": {
            "from": agent.address,
            "thread_id": thread_id,
            "content": [{"type": "text", "text": text}],
        }
    });
    if let Err(e) = crate::signing::sign_envelope(&agent.key_pair, &mut envelope) {
        tracing::warn!("[runtime] failed to sign agent envelope: {e}");
    }
    state.human_registry.send_to_thread(thread_id, &envelope.to_string()).await;
}
```

The exact `messages` columns (`from_addr`, `content`, `sequence`, `thread_id`) match the history fetch found at `server.rs:936-945`. If the column names differ, adapt the SQL to the real schema in `server/migrations/001_base_tables.sql`.

- [ ] **Step 3: Invoke from the delivery path**

In `server/src/server.rs`, in the human-message handler where a local `ainm.message.send` is processed (around `server.rs:2049`, after `human_registry.join_thread(...)` and local delivery), add an invocation when the hosted agent participates. Insert after the local-delivery block:

```rust
        // Hosted agent: if provisioned and this thread targets it, run the in-process loop.
        if let Some(agent) = state.hosted_agent.clone() {
            let targets_agent = msg_thread_id.contains("assistant@")  // simple v1 heuristic
                || params.get("to").and_then(|t| t.as_str()).map(|t| t == agent.address).unwrap_or(false)
                || content_mentions(&content, &agent.address);
            if targets_agent {
                // Ensure the agent is a tracked participant so it receives stream frames? No —
                // the agent produces frames; humans receive. Just record the human joined (done above).
                let state2 = state.clone();
                let tid = msg_thread_id.to_string();
                let requester = human_addr.to_string();
                let user_text = content_to_plain(&content);
                tokio::spawn(async move {
                    crate::runtime::handle_turn(&state2, &tid, &requester, &user_text).await;
                });
            }
        }
```

Add two small helpers near the top of `server.rs` (or inline as free fns):

```rust
fn content_mentions(content: &serde_json::Value, addr: &str) -> bool {
    content_to_plain(content).contains(&format!("@{}", addr.split('@').next().unwrap_or(addr)))
}
fn content_to_plain(content: &serde_json::Value) -> String {
    if let Some(arr) = content.as_array() {
        arr.iter().filter_map(|b| b.get("text").and_then(|t| t.as_str())).collect::<Vec<_>>().join(" ")
    } else { content.as_str().unwrap_or("").to_string() }
}
```

Use the variable names that actually exist at that point in the handler (`human_addr`, `msg_thread_id`, `content`, `params`); the names above match the exploration of `server.rs:2049-2141`. If a name differs, bind it from `params` as the surrounding code does.

- [ ] **Step 4: Thread new params through `start_federation` + build in `main.rs`**

Add `hosted_agent`, `llm_client`, and `mcp_providers` parameters to `server::start_federation` (and set the new `ServerState` fields from them). In `server/src/main.rs`, after the DB pool is initialized and before `start_federation` is called (around `main.rs:447`), build them:

```rust
    // Provision the hosted agent + build its LLM client and MCP providers (DB-backed only).
    let (hosted_agent, hosted_llm, mcp_providers): (
        Option<std::sync::Arc<ainm_server::hosted_agent::HostedAgent>>,
        Option<std::sync::Arc<dyn ainm_server::llm::LlmClient>>,
        std::sync::Arc<Vec<std::sync::Arc<dyn ainm_server::tools::ToolProvider>>>,
    ) = if let Some(ref pool) = db_pool {
        let agent = ainm_server::hosted_agent::provision(pool, server_id).await
            .map(std::sync::Arc::new)
            .map_err(|e| { tracing::warn!("[hosted-agent] provisioning failed: {e}"); e })
            .ok();
        let llm: Option<std::sync::Arc<dyn ainm_server::llm::LlmClient>> = match std::env::var("AINM_LLM__API_KEY") {
            Ok(key) => Some(std::sync::Arc::new(ainm_server::llm::OpenAiClient::new(
                key,
                std::env::var("AINM_LLM__MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string()),
                std::env::var("AINM_LLM__BASE_URL").ok(),
            ))),
            Err(_) => None,
        };
        let providers = ainm_server::mcp::build_providers_from_config(&app_config).await;
        (agent, llm, std::sync::Arc::new(providers))
    } else {
        (None, None, std::sync::Arc::new(vec![]))
    };
```

(`ainm_server::mcp::build_providers_from_config` is implemented in B-9; until then, B-8 can stub it to return `vec![]`. To keep B-8 self-contained, add a temporary `pub async fn build_providers_from_config(_c: &crate::config::AppConfig) -> Vec<std::sync::Arc<dyn crate::tools::ToolProvider>> { vec![] }` in a new `server/src/mcp.rs` + `pub mod mcp;` — B-9 fills it in.)

Note: in a binary, the crate self-refers as `crate::`, not `ainm_server::`. In `main.rs` (which is the binary root), use `ainm_server::` only if `main.rs` uses the library crate; otherwise use `crate::`. Match the existing style in `main.rs` (it already calls e.g. `config::load()`, `db::init_pool` — so use the same unqualified `mod`/`use` style the file already uses).

- [ ] **Step 5: Build the whole server**

Run: `cargo build -p ainm-server`
Expected: compiles. Fix any signature mismatches (parameter threading) until green.

- [ ] **Step 6: Run unit suite**

Run: `cargo test -p ainm-server --lib`
Expected: PASS (the new `runtime`/`tools`/`tools_memory` unit tests included).

- [ ] **Step 7: Commit**

```bash
git add server/src/runtime.rs server/src/server.rs server/src/main.rs server/src/mcp.rs server/src/lib.rs
git commit -m "feat(b): wire hosted-agent runtime into delivery path with streaming + signing (B-8)"
```

---

## Wave 5 — Curated MCP bridge

### Task B-9: `mcp.rs` — `rmcp` client behind `ToolProvider` + curated config

**Files:**
- Modify: `server/Cargo.toml` (add `rmcp`)
- Modify: `server/src/config.rs` (add curated MCP server config)
- Modify: `server/src/mcp.rs` (replace the B-8 stub with the real bridge)
- Test: unit test for config parsing + capability gating; live MCP verified manually (and via `MockToolProvider` in B-11).

`rmcp` 1.7 facts (researched): features `client`, `transport-child-process`, `transport-streamable-http-client-reqwest`; `().serve(transport).await` → `RunningService`; `.list_all_tools()`; `.call_tool(CallToolRequestParams::new(name).with_arguments(map))`. `arguments` is a `serde_json::Map`, not a bare `Value`.

- [ ] **Step 1: Add the dependency**

In `server/Cargo.toml` `[dependencies]`:

```toml
rmcp = { version = "1.7", default-features = false, features = [
    "client",
    "transport-child-process",
    "transport-streamable-http-client-reqwest",
] }
```

- [ ] **Step 2: Add curated MCP config + write the failing test**

In `server/src/config.rs`, add to the config structs:

```rust
#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct McpServerConfig {
    pub name: String,
    /// "stdio" or "http"
    pub transport: String,
    /// For stdio: the command to spawn (argv[0]); for http: unused.
    #[serde(default)]
    pub command: Option<String>,
    /// For stdio: command args.
    #[serde(default)]
    pub args: Vec<String>,
    /// For http: the server URL.
    #[serde(default)]
    pub url: Option<String>,
    /// The consent capability/scope a requester must hold to be offered this server's tools.
    pub required_capability: String,
}
```

Add a field to `AppConfig`:

```rust
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
```

Write the failing test (add to `config.rs` tests, or `mcp.rs` tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_server_offered_only_when_capability_granted() {
        let cfg = crate::config::McpServerConfig {
            name: "weather".into(), transport: "http".into(), command: None, args: vec![],
            url: Some("http://localhost:9000/mcp".into()),
            required_capability: "patterns".into(),
        };
        assert!(server_is_permitted(&cfg, &["patterns".to_string()]));
        assert!(!server_is_permitted(&cfg, &["identity".to_string()]));
        assert!(!server_is_permitted(&cfg, &[]));
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p ainm-server mcp_server_offered_only_when_capability_granted`
Expected: FAIL.

- [ ] **Step 4: Implement the bridge**

Replace `server/src/mcp.rs` contents:

```rust
//! Curated MCP bridge: connects to trusted MCP servers (config-defined) and exposes
//! their tools through the `ToolProvider` trait. This is the sub-project-D isolation seam —
//! D can replace this impl with a sandboxed executor without touching the runtime loop.

use crate::config::{AppConfig, McpServerConfig};
use crate::llm::ToolSchema;
use crate::tools::{ToolProvider, ToolResult};
use std::sync::Arc;

/// Pure consent check: is this curated server offered for the requester's scopes?
pub fn server_is_permitted(cfg: &McpServerConfig, scopes: &[String]) -> bool {
    scopes.iter().any(|s| s == &cfg.required_capability)
}

/// Build one ToolProvider per configured curated MCP server. Connection failures are
/// logged and that server is skipped (the runtime still works without it).
pub async fn build_providers_from_config(config: &AppConfig) -> Vec<Arc<dyn ToolProvider>> {
    let mut providers: Vec<Arc<dyn ToolProvider>> = Vec::new();
    for cfg in &config.mcp_servers {
        match McpToolProvider::connect(cfg.clone()).await {
            Ok(p) => {
                tracing::info!("[mcp] connected curated server '{}'", cfg.name);
                providers.push(Arc::new(p));
            }
            Err(e) => tracing::warn!("[mcp] failed to connect '{}': {e}", cfg.name),
        }
    }
    providers
}

/// A connected curated MCP server exposed as a ToolProvider.
pub struct McpToolProvider {
    cfg: McpServerConfig,
    /// Cached tool list discovered at connect time: (name, description, params schema).
    tools: Vec<ToolSchema>,
    client: McpClientHandle,
}

impl McpToolProvider {
    pub async fn connect(cfg: McpServerConfig) -> anyhow::Result<Self> {
        let client = McpClientHandle::connect(&cfg).await?;
        let tools = client.list_tools().await?;
        Ok(Self { cfg, tools, client })
    }
}

#[async_trait::async_trait]
impl ToolProvider for McpToolProvider {
    async fn list_tools(&self, scopes: &[String]) -> Vec<ToolSchema> {
        if !server_is_permitted(&self.cfg, scopes) {
            return vec![];
        }
        self.tools.clone()
    }

    async fn call(&self, name: &str, args: &serde_json::Value, scopes: &[String]) -> ToolResult {
        // Consent gate (direction 2): refuse if this server isn't permitted for the scopes.
        if !server_is_permitted(&self.cfg, scopes) {
            return ToolResult::error(format!("MCP server '{}' not permitted by consent scopes", self.cfg.name));
        }
        if !self.tools.iter().any(|t| t.name == name) {
            return ToolResult::error(format!("tool '{name}' not offered by '{}'", self.cfg.name));
        }
        match self.client.call_tool(name, args).await {
            Ok(text) => ToolResult::ok(text),
            Err(e) => ToolResult::error(format!("MCP call failed: {e}")),
        }
    }
}

/// Thin wrapper around the rmcp client, isolating the SDK surface.
struct McpClientHandle {
    inner: rmcp::service::RunningService<rmcp::RoleClient, ()>,
}

impl McpClientHandle {
    async fn connect(cfg: &McpServerConfig) -> anyhow::Result<Self> {
        use rmcp::ServiceExt;
        let inner = match cfg.transport.as_str() {
            "stdio" => {
                use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
                let command = cfg.command.clone()
                    .ok_or_else(|| anyhow::anyhow!("stdio MCP server '{}' missing command", cfg.name))?;
                let args = cfg.args.clone();
                let transport = TokioChildProcess::new(
                    tokio::process::Command::new(command).configure(|c| { for a in &args { c.arg(a); } })
                )?;
                ().serve(transport).await?
            }
            "http" => {
                use rmcp::transport::StreamableHttpClientTransport;
                let url = cfg.url.clone()
                    .ok_or_else(|| anyhow::anyhow!("http MCP server '{}' missing url", cfg.name))?;
                let transport = StreamableHttpClientTransport::from_uri(url);
                ().serve(transport).await?
            }
            other => anyhow::bail!("unknown MCP transport '{other}' for '{}'", cfg.name),
        };
        Ok(Self { inner })
    }

    async fn list_tools(&self) -> anyhow::Result<Vec<ToolSchema>> {
        let tools = self.inner.list_all_tools().await?;
        Ok(tools.into_iter().map(|t| ToolSchema {
            name: t.name.to_string(),
            description: t.description.map(|d| d.to_string()).unwrap_or_default(),
            parameters: serde_json::to_value(&*t.input_schema).unwrap_or(serde_json::json!({"type":"object"})),
        }).collect())
    }

    async fn call_tool(&self, name: &str, args: &serde_json::Value) -> anyhow::Result<String> {
        use rmcp::model::CallToolRequestParam;
        let arguments = args.as_object().cloned();
        let params = CallToolRequestParam { name: name.to_string().into(), arguments };
        let result = self.inner.call_tool(params).await?;
        // Flatten the content blocks to text.
        let text = serde_json::to_string(&result.content).unwrap_or_default();
        Ok(text)
    }
}
```

The exact `rmcp` type/field names (`CallToolRequestParam` vs `CallToolRequestParams`, `RunningService<RoleClient, ()>`, `Tool.input_schema`, content shape) must be reconciled against `docs.rs/rmcp/1.7` at implementation time — the research confirms the capability and rough shapes; the implementer pins the exact API. Keep all `rmcp` types **inside `McpClientHandle`** so the rest of the codebase never imports `rmcp` directly (the isolation seam).

- [ ] **Step 5: Run the config/gating test + build**

Run: `cargo test -p ainm-server mcp_server_offered_only_when_capability_granted && cargo build -p ainm-server`
Expected: PASS + compiles.

- [ ] **Step 6: Add a sample curated server to config (documentation, disabled by default)**

Add a commented example to `server/config/default.toml`:

```toml
# Curated MCP servers the hosted agent may use. Empty by default.
# [[mcp_servers]]
# name = "time"
# transport = "stdio"
# command = "uvx"
# args = ["mcp-server-time"]
# required_capability = "patterns"
```

- [ ] **Step 7: Commit**

```bash
git add server/Cargo.toml server/src/config.rs server/src/mcp.rs server/config/default.toml
git commit -m "feat(b): curated MCP bridge via rmcp behind ToolProvider (B-9)"
```

---

## Wave 6 — Integration tests + final review

### Task B-10: Integration tests (bootstrap, full turn, consent gate, MCP-mock, cap)

**Files:**
- Create: `server/tests/hosted_agent_test.rs`
- Test: this IS the test file.

These are DB-backed where needed; follow the existing pattern in `server/tests/` for acquiring a pool and skipping when no DB (e.g. read `AINM_DATABASE__URL`; if absent, `eprintln!` + `return`). Reuse helpers from existing integration tests (look at the sub-project-A lifecycle test for the pool-setup helper and migration handling).

- [ ] **Step 1: Write the tests**

Create `server/tests/hosted_agent_test.rs`:

```rust
//! Integration tests for the hosted agent runtime (sub-project B).

use ainm_server::hosted_agent;

/// Acquire a test pool or skip. Mirror the helper used by the accounts lifecycle test.
async fn pool_or_skip() -> Option<sqlx::PgPool> {
    let url = std::env::var("AINM_DATABASE__URL").or_else(|_| std::env::var("DATABASE_URL")).ok()?;
    let pool = sqlx::PgPool::connect(&url).await.ok()?;
    sqlx::migrate::Migrator::new(std::path::Path::new("migrations")).await.ok()?
        .run(&pool).await.ok()?;
    Some(pool)
}

#[tokio::test]
async fn bootstrap_is_idempotent() {
    let Some(pool) = pool_or_skip().await else { eprintln!("no DB; skipping"); return; };
    let domain = "ainm.local";

    let a1 = hosted_agent::provision(&pool, domain).await.expect("provision 1");
    let a2 = hosted_agent::provision(&pool, domain).await.expect("provision 2");

    // Same address + same stable public key across calls.
    assert_eq!(a1.address, format!("assistant@{domain}"));
    assert_eq!(a1.public_key_bytes, a2.public_key_bytes);

    // Exactly one account, one active key, one hosted_agents row.
    let acct_count: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts WHERE address = $1")
        .bind(&a1.address).fetch_one(&pool).await.unwrap();
    assert_eq!(acct_count, 1);
    let ha_count: i64 = sqlx::query_scalar("SELECT count(*) FROM hosted_agents WHERE address = $1")
        .bind(&a1.address).fetch_one(&pool).await.unwrap();
    assert_eq!(ha_count, 1);
}

#[tokio::test]
async fn consent_gate_hides_ungranted_tools() {
    // Pure check using the memory-tool gating (no DB needed).
    let minimal = vec!["identity".to_string()];
    let offered = ainm_server::tools_memory::offered_tool_names(&minimal);
    assert!(offered.contains(&"query_memory".to_string()));
    assert!(!offered.contains(&"store_fact".to_string()));

    let none: Vec<String> = vec![];
    assert!(ainm_server::tools_memory::offered_tool_names(&none).is_empty());
}

#[tokio::test]
async fn full_hosted_turn_with_mock_llm_and_mock_tool() {
    use ainm_server::llm::{AssistantTurn, MockLlmClient, ToolCall, ToolSchema};
    use ainm_server::tools::{ToolProvider, ToolRegistry, ToolResult};
    use ainm_server::runtime::run_loop;

    struct MockMcp;
    #[async_trait::async_trait]
    impl ToolProvider for MockMcp {
        async fn list_tools(&self, scopes: &[String]) -> Vec<ToolSchema> {
            if scopes.iter().any(|s| s == "patterns") {
                vec![ToolSchema { name: "get_time".into(), description: "t".into(),
                    parameters: serde_json::json!({"type":"object"}) }]
            } else { vec![] }
        }
        async fn call(&self, name: &str, _a: &serde_json::Value, scopes: &[String]) -> ToolResult {
            if !scopes.iter().any(|s| s == "patterns") { return ToolResult::error("denied"); }
            if name == "get_time" { ToolResult::ok("2026-06-01T12:00:00Z") } else { ToolResult::error("no") }
        }
    }

    let llm = MockLlmClient::with_turns(vec![
        AssistantTurn::ToolCalls(vec![ToolCall { id: "c1".into(), name: "get_time".into(), arguments: serde_json::json!({}) }]),
        AssistantTurn::Text("It is noon UTC.".into()),
    ]);
    let reg = ToolRegistry::new(vec![Box::new(MockMcp)]);

    // With the capability granted: tool runs, final text returned.
    let out = run_loop(&llm, &reg, "sys", &["patterns".to_string()], vec![], "what time is it?", 8).await;
    assert_eq!(out.final_text, "It is noon UTC.");
    assert_eq!(out.tool_rounds, 1);
}

#[tokio::test]
async fn mcp_tool_absent_without_capability() {
    use ainm_server::llm::ToolSchema;
    use ainm_server::tools::{ToolProvider, ToolRegistry};

    struct MockMcp;
    #[async_trait::async_trait]
    impl ToolProvider for MockMcp {
        async fn list_tools(&self, scopes: &[String]) -> Vec<ToolSchema> {
            if scopes.iter().any(|s| s == "patterns") {
                vec![ToolSchema { name: "get_time".into(), description: "t".into(), parameters: serde_json::json!({"type":"object"}) }]
            } else { vec![] }
        }
        async fn call(&self, _n: &str, _a: &serde_json::Value, _s: &[String]) -> ainm_server::tools::ToolResult {
            ainm_server::tools::ToolResult::error("denied")
        }
    }
    let reg = ToolRegistry::new(vec![Box::new(MockMcp)]);
    // No capability → tool not offered, and a forced call is refused.
    assert!(reg.list_tools(&["identity".to_string()]).await.is_empty());
    let refused = reg.call("get_time", &serde_json::json!({}), &["identity".to_string()]).await;
    assert!(refused.is_error);
}

#[tokio::test]
async fn loop_stops_at_cap_integration() {
    use ainm_server::llm::{AssistantTurn, MockLlmClient, ToolCall, ToolSchema};
    use ainm_server::tools::{ToolProvider, ToolRegistry, ToolResult};
    use ainm_server::runtime::run_loop;

    struct AlwaysTool;
    #[async_trait::async_trait]
    impl ToolProvider for AlwaysTool {
        async fn list_tools(&self, _s: &[String]) -> Vec<ToolSchema> {
            vec![ToolSchema { name: "t".into(), description: "t".into(), parameters: serde_json::json!({"type":"object"}) }]
        }
        async fn call(&self, _n: &str, _a: &serde_json::Value, _s: &[String]) -> ToolResult { ToolResult::ok("x") }
    }
    let turns = (0..20).map(|_| AssistantTurn::ToolCalls(vec![ToolCall { id: "c".into(), name: "t".into(), arguments: serde_json::json!({}) }])).collect();
    let llm = MockLlmClient::with_turns(turns);
    let reg = ToolRegistry::new(vec![Box::new(AlwaysTool)]);
    let out = run_loop(&llm, &reg, "sys", &["identity".to_string()], vec![], "go", 3).await;
    assert_eq!(out.tool_rounds, 3);
    assert!(out.stopped_early);
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p ainm-server --test hosted_agent_test`
Expected: the pure/loop tests PASS; the DB-backed `bootstrap_is_idempotent` PASSES if a DB is configured, otherwise prints "no DB; skipping" and passes.

- [ ] **Step 3: Commit**

```bash
git add server/tests/hosted_agent_test.rs
git commit -m "test(b): hosted-agent integration tests — bootstrap, turn, consent gate, cap (B-10)"
```

---

### Task B-11: Full suite green + clippy + final review

**Files:**
- Modify: any file needing a clippy/test fix surfaced here.

- [ ] **Step 1: Run the full server suite**

Run: `cargo test -p ainm-server`
Expected: all green EXCEPT the two known pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`). If any NEW test fails, fix it before proceeding.

- [ ] **Step 2: Run clippy across all targets**

Run: `cargo clippy -p ainm-server --all-targets -- -D warnings`
Expected: no warnings. Fix any introduced by the new modules (unused imports, needless clones, etc.).

- [ ] **Step 3: Verify the agent's private key never appears in logs**

Run: `grep -rn "private_key" server/src/ | grep -i "tracing\|println\|eprintln\|debug!\|info!\|warn!"`
Expected: NO matches. If any, remove the log of the secret.

- [ ] **Step 4: Confirm DoD items by inspection**

Re-read `docs/superpowers/specs/2026-06-01-hosted-agent-runtime-design.md` §14 and confirm each item maps to shipped code:
1. Startup provisioning idempotent → B-2 + `bootstrap_is_idempotent`.
2. `chat_with_tools` + scripted mock → B-3/B-4.
3. Memory tools + both-direction consent gating → B-6 + tests.
4. MCP bridge behind `ToolProvider`, verified via mock → B-9/B-10.
5. `runtime.rs` bounded loop invoked from delivery, signed streamed reply → B-7/B-8.
6. End-to-end human↔agent turn uses memory + (mock) MCP tool, gate provably hides ungranted → B-10.
7. Suite green minus the two known failures → Step 1.

- [ ] **Step 5: Commit any fixes**

```bash
git add -A
git commit -m "chore(b): full suite green + clippy clean + secret-in-logs audit (B-11)"
```

---

## Self-Review (completed by plan author)

**Spec coverage:** §4 identity/provisioning → B-1/B-2. §5 routing → B-8. §6 loop → B-7. §7 tools + both consent gates → B-5/B-6 (memory) + B-9 (MCP). §8 data model/config → B-1 (table) + B-9 (config). §9 module boundaries → all (`runtime.rs`, `tools.rs`+`tools_memory.rs`, `mcp.rs`, `hosted_agent.rs`, `llm.rs` extend, `server.rs`/`main.rs` extend). §10 error handling → B-7 (LLM error, cap), B-6/B-9 (tool/consent errors). §11 testing → B-10. §12 risk (`rmcp`) → resolved by research, isolated in `McpClientHandle`. §14 DoD → B-11 Step 4.

**Note on a spec/plan deviation (intentional):** the spec §6 says "streaming tool-use loop." The plan implements `chat_with_tools` as a single request/response and streams the *final* text to the human via `ainm.stream.*`. This preserves the human-visible streaming UX and the moat (every tool call gated), while keeping the loop robust and token-free testable. Flagged here so the reviewer sees it is deliberate, not a gap.

**Type consistency:** `ToolSchema`/`ToolCall`/`ChatMessage`/`AssistantTurn` defined in `llm.rs` (B-3) are used consistently in `tools.rs` (B-5), `tools_memory.rs` (B-6), `runtime.rs` (B-7/B-8), `mcp.rs` (B-9). `ToolProvider`/`ToolResult`/`ToolRegistry` defined in `tools.rs` (B-5) used everywhere after. `HostedAgent` (B-2) used in `server.rs`/`runtime.rs` (B-8).

**Known adaptation points (called out inline, not placeholders):** exact `StorageError` variant names (B-2), `NodeRow` field names (B-6), `messages` table column names (B-8), and the exact `rmcp` 1.7 type/field names (B-9) are reconciled against the real code/docs at implementation time. Each is flagged at its site with the fallback instruction.
