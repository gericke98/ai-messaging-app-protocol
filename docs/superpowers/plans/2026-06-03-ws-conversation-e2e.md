# WebSocket Conversation E2E Test — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a server-side E2E test that drives the full hosted-agent conversation over a real WebSocket against the compiled `ainm-server` binary and asserts the agent's reply is delivered back — validating the entire server half of IOS-11 automatically.

**Architecture:** A small production affordance (`ConstantMockLlmClient` + `AINM_LLM__MOCK` env) gives the hosted agent a deterministic, key-free reply. A black-box integration test (`server/tests/ws_conversation_e2e.rs`) inserts an account + bound key directly via sqlx, spawns the binary headless with the mock env, polls `/health/ready`, does the signed `ainm.initialize` handshake, sends a JCS-signed `ainm.message.send` addressed to `assistant@<domain>`, and asserts the signed `ainm.message.deliver` from the agent contains the canned text. Gated on a reachable Postgres (skips otherwise), same as the other integration suites.

**Tech Stack:** Rust, tokio, tokio-tungstenite 0.26, reqwest 0.13, sqlx 0.8 (Postgres), ring (Ed25519), serde_json. The test links the `ainm_server` lib (`signing`, `identity`, `protocol`) and black-boxes the binary (`ServerState`/`handle_turn` are binary-only — the B-8 lib/bin split).

**Spec:** `docs/superpowers/specs/2026-06-03-ws-conversation-e2e-design.md`

---

## File Structure

- **`server/src/llm.rs`** — add `pub struct ConstantMockLlmClient { pub reply: String }` implementing `LlmClient` (constant `Text` turn, never errors), plus a unit test. Responsibility: a deterministic, non-exhausting LLM client for the hosted agent.
- **`server/src/main.rs`** — extend the hosted-agent LLM selection (lines 511–528) so `AINM_LLM__MOCK` truthy selects `ConstantMockLlmClient`, taking precedence over the API-key branch. Responsibility: wiring the affordance into the real binary.
- **`server/tests/ws_conversation_e2e.rs`** (new) — the black-box WS E2E test + local `pool_or_skip` helper + WS framing helpers (mirroring `repl.rs`). Responsibility: drive and assert the full conversation loop.
- **`docs/runbooks/2026-06-02-ios11-close-the-loop.md`** — one note documenting `AINM_LLM__MOCK=1` for a key-free reply. Responsibility: operator guidance.

These are independent layers: Task 1 ships the server affordance (compiles + unit-tests on all platforms, no DB); Task 2 ships the test that depends on Task 1's `AINM_LLM__MOCK`; Task 3 runs the whole thing live and finalizes.

---

## Task 1: `ConstantMockLlmClient` + `AINM_LLM__MOCK` wiring

**Files:**
- Modify: `server/src/llm.rs` (add struct + impl after the `MockLlmClient` impl, ~line 478; add unit test in the existing `#[cfg(test)] mod tests` at line 599)
- Modify: `server/src/main.rs:511-528` (hosted-agent LLM selection)

**Context:** `MockLlmClient::chat_with_tools` (llm.rs:463) *errors* when its turn queue is exhausted (a B-4 invariant), so it is unsuitable for a server that may call the LLM any number of times. We need a constant client. The `LlmClient` trait (llm.rs:151) has `extract` (required) and `chat_with_tools` (default-errors; we override it). `ChatResponse { turn: AssistantTurn, usage: Option<TokenUsage> }`. The hosted-agent LLM is currently `None` when no API key is set (main.rs:516-523) — and `handle_turn` early-returns on `None`, so a key-free run produces no reply. This task makes a key-free run deterministically reply.

- [ ] **Step 1: Write the failing unit test**

Add to the existing `#[cfg(test)] mod tests` block in `server/src/llm.rs` (after the existing tests, before the closing `}` of the module):

```rust
    #[tokio::test]
    async fn constant_mock_returns_same_text_and_never_exhausts() {
        let client = ConstantMockLlmClient { reply: "canned reply".into() };
        // Call several times — unlike MockLlmClient, it must never error or change.
        for _ in 0..5 {
            let resp = client.chat_with_tools(&[], &[]).await.unwrap();
            match resp.turn {
                AssistantTurn::Text(t) => assert_eq!(t, "canned reply"),
                AssistantTurn::ToolCalls(_) => panic!("constant mock must never request tool calls"),
            }
        }
        // extract is not exercised on the agent path; it must return an empty result, not error.
        let extracted = client.extract(&[]).await.unwrap();
        assert!(extracted.nodes.is_empty() && extracted.edges.is_empty());
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p ainm-server --lib constant_mock_returns_same_text_and_never_exhausts`
Expected: FAIL — `cannot find struct, variant or union type 'ConstantMockLlmClient' in this scope`.

- [ ] **Step 3: Implement `ConstantMockLlmClient`**

Add to `server/src/llm.rs` immediately after the `impl LlmClient for MockLlmClient { ... }` block (which ends at line 478, just before the `// ── Prompt ──` comment):

```rust
/// A constant mock LLM client: every `chat_with_tools` call returns the same final
/// text turn, and it never exhausts or errors. Unlike `MockLlmClient` (a FIFO queue
/// that errors when drained), this is safe for a long-lived server that may call the
/// LLM any number of times. Selected by the `AINM_LLM__MOCK` env in `main.rs` to give
/// the hosted agent a deterministic, key-free reply (E2E tests + key-free dogfood).
pub struct ConstantMockLlmClient {
    pub reply: String,
}

#[async_trait::async_trait]
impl LlmClient for ConstantMockLlmClient {
    async fn extract(
        &self,
        _messages: &[ConversationMessage],
    ) -> Result<ExtractionResult, AppError> {
        Ok(ExtractionResult { nodes: vec![], edges: vec![] })
    }

    async fn chat_with_tools(
        &self,
        _messages: &[ChatMessage],
        _tools: &[ToolSchema],
    ) -> Result<ChatResponse, AppError> {
        Ok(ChatResponse {
            turn: AssistantTurn::Text(self.reply.clone()),
            usage: None,
        })
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p ainm-server --lib constant_mock_returns_same_text_and_never_exhausts`
Expected: PASS (1 passed).

- [ ] **Step 5: Define the canned reply constant + wire `AINM_LLM__MOCK` in `main.rs`**

The canned string must be a single source of truth the test asserts a substring of. Expose it as a public const on the lib so the test imports it. Add to `server/src/llm.rs` near the top of the `ConstantMockLlmClient` definition (just above the `pub struct ConstantMockLlmClient`):

```rust
/// The fixed reply used when `AINM_LLM__MOCK` selects the constant mock client.
/// Single source of truth: the WS E2E test asserts the delivered reply contains this.
pub const MOCK_LLM_REPLY: &str = "[mock-llm] Hello from the AINM assistant. (no LLM key configured)";
```

Then in `server/src/main.rs`, replace the `let llm: Option<...> = match std::env::var("AINM_LLM__API_KEY") { ... };` block (lines 516-523) with mock-precedence selection:

```rust
        let llm: Option<std::sync::Arc<dyn llm::LlmClient>> = if matches!(
            std::env::var("AINM_LLM__MOCK").as_deref(),
            Ok("1") | Ok("true")
        ) {
            tracing::warn!(
                "[llm] AINM_LLM__MOCK set — hosted agent using constant mock reply (dev/test only)"
            );
            Some(std::sync::Arc::new(llm::ConstantMockLlmClient {
                reply: llm::MOCK_LLM_REPLY.to_string(),
            }))
        } else {
            match std::env::var("AINM_LLM__API_KEY") {
                Ok(key) => Some(std::sync::Arc::new(llm::OpenAiClient::new(
                    key,
                    std::env::var("AINM_LLM__MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string()),
                    std::env::var("AINM_LLM__BASE_URL").ok(),
                ))),
                Err(_) => None,
            }
        };
```

- [ ] **Step 6: Verify it compiles and clippy is clean on touched files**

Run: `cargo build -p ainm-server && cargo clippy -p ainm-server --all-targets -- -D warnings 2>&1 | tail -20`
Expected: builds; no clippy warnings introduced in `llm.rs` / `main.rs`.

- [ ] **Step 7: Commit**

```bash
rtk proxy git add server/src/llm.rs server/src/main.rs
rtk proxy git commit -m "feat(server): ConstantMockLlmClient + AINM_LLM__MOCK for deterministic key-free agent replies"
```

---

## Task 2: Black-box WS conversation E2E test

**Files:**
- Create: `server/tests/ws_conversation_e2e.rs`

**Context:** Integration tests link the `ainm_server` lib but CANNOT construct the server (`ServerState`/`handle_turn` are binary-only). So the test spawns the compiled binary via `env!("CARGO_BIN_EXE_ainm-server")`. The binary in `--mode human` requires cert flags (dev certs live at `server/certs/server-a.pem`, `server-a-key.pem`, `ca.pem`) and, with `AINM_DATABASE__URL`, auto-provisions `assistant@<--identity>`. The headless fix (merged) means human mode serves without a TTY. Account binding: the WS handshake (server.rs:1065-1083) looks up the account by the handshake `identity` address, fetches its active key (URL-safe-base64-no-pad of the public key bytes), and runs `verify_init`. Sending a message with a non-empty `thread_id` auto-joins the human to that thread (server.rs:2337), so the agent's `send_to_thread` delivery (runtime_server.rs:239) reaches the same WS connection. The agent trigger fires on `params.to == agent.address` (server.rs:2456). The delivered reply is a notification `method == "ainm.message.deliver"` with `params.message.from == assistant@<domain>` and `params.message.content[0].text` (runtime_server.rs:220-238).

Mirror the helpers from `server/tests/auth_sessions_test.rs` (`pool_or_skip`, the `INSERT INTO accounts ...` + `INSERT INTO account_keys ...` patterns) and the handshake/envelope framing from `server/src/repl.rs:20-48` and `:317-344`.

- [ ] **Step 1: Write the full test file (it will fail to pass until run against a DB; first verify it compiles)**

Create `server/tests/ws_conversation_e2e.rs`:

```rust
//! End-to-end WebSocket conversation test against the compiled `ainm-server` binary.
//!
//! Validates the entire server half of IOS-11: signed `ainm.initialize` handshake against
//! a bound account key, the agent trigger on `params.to == assistant@<domain>`, the runtime
//! loop, and the signed `ainm.message.deliver` round-trip. Black-boxes the binary because
//! `ServerState`/`handle_turn` are binary-only (the B-8 lib/bin split). Skips with no DB.

use ainm_server::{identity, llm, protocol, signing};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use std::process::Stdio;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

/// Connect to Postgres (TEST_DATABASE_URL → AINM_DATABASE__URL → DATABASE_URL) + run migrations,
/// returning both the pool and the URL (the spawned binary needs the URL). None → skip.
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

/// Insert an active, email-verified account and bind a freshly generated Ed25519 key
/// (stored as URL-safe-base64-no-pad of the public key, matching the bind-key endpoint).
/// Returns the in-memory identity (address + key pair) for signing.
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

    let id = identity::generate_keypair(address).expect("keypair");
    let pk_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&id.public_key_bytes);
    sqlx::query("INSERT INTO account_keys (account_id, public_key) VALUES ($1, $2)")
        .bind(account_id)
        .bind(&pk_b64)
        .execute(pool)
        .await
        .expect("insert account_key");
    id
}

/// Bind 127.0.0.1:0, read the assigned port, drop the listener so the server can take it.
async fn free_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}

#[tokio::test]
async fn ws_conversation_delivers_agent_reply() {
    let Some((pool, db_url)) = pool_and_url_or_skip().await else {
        eprintln!("no DB; skipping ws_conversation_delivers_agent_reply");
        return;
    };

    // Unique per-run domain avoids collisions in a shared DB.
    let domain = format!("e2e-{:x}.local", uuid::Uuid::new_v4().as_u128());
    let alice_addr = format!("alice@{domain}");
    let agent_addr = format!("assistant@{domain}");
    let alice = make_bound_account(&pool, &alice_addr, &domain).await;

    let port = free_port().await;
    let certs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("certs");

    // Spawn the compiled binary headless with the mock LLM affordance.
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_ainm-server"))
        .args([
            "--mode", "human",
            "--port", &port.to_string(),
            "--identity", &domain,
            "--cert", certs.join("server-a.pem").to_str().unwrap(),
            "--key", certs.join("server-a-key.pem").to_str().unwrap(),
            "--ca-cert", certs.join("ca.pem").to_str().unwrap(),
        ])
        .env("AINM_DATABASE__URL", &db_url)
        .env("AINM_LLM__MOCK", "1")
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

    // Open the WebSocket and do the signed handshake (mirror repl.rs:20-48).
    let ws_url = format!("ws://127.0.0.1:{port}/ws");
    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("ws connect");
    let (mut tx, mut rx) = ws_stream.split();

    let (init_sig, init_ts) = signing::sign_init(&alice.key_pair, &alice.address).unwrap();
    let init_req = protocol::new_request(
        "ainm.initialize",
        serde_json::json!({
            "identity": alice.address,
            "protocolVersion": "0.1.0",
            "signature": init_sig,
            "timestamp": init_ts,
            "capabilities": {
                "streaming": true,
                "contentTypes": ["text"],
                "toolVisibility": true
            }
        }),
        serde_json::json!(2),
    );
    tx.send(Message::Text(serde_json::to_string(&init_req).unwrap().into()))
        .await
        .unwrap();

    // Await the `initialized` response (id 2 result), then send the `ainm.initialized` notification.
    let init_resp = next_text(&mut rx, Duration::from_secs(10)).await.expect("init response");
    let init_resp_json: serde_json::Value = serde_json::from_str(&init_resp).unwrap();
    assert!(
        init_resp_json.get("result").is_some(),
        "handshake rejected: {init_resp}"
    );
    let initialized = protocol::new_notification("ainm.initialized", serde_json::json!({}));
    tx.send(Message::Text(serde_json::to_string(&initialized).unwrap().into()))
        .await
        .unwrap();

    // Build + JCS-sign a message addressed to the agent (mirror repl.rs:317-344).
    let thread_id = protocol::new_message_id();
    let mut envelope = serde_json::json!({
        "id": protocol::new_message_id(),
        "from": alice.address,
        "thread_id": thread_id,
        "timestamp": protocol::now_rfc3339(),
        "type": "message",
        "content": [{
            "type": "text",
            "version": "1.0",
            "source": "user",
            "text": "hello assistant",
            "fallback": "hello assistant"
        }],
        "to": agent_addr,
    });
    signing::sign_envelope(&alice.key_pair, &mut envelope).expect("sign envelope");
    let send_req = protocol::new_request(
        "ainm.message.send",
        envelope,
        serde_json::json!(protocol::new_message_id()),
    );
    tx.send(Message::Text(serde_json::to_string(&send_req).unwrap().into()))
        .await
        .unwrap();

    // Read frames until the terminal `ainm.message.deliver` from the agent (~20s budget).
    // Tolerate the send ack (id result), stream deltas, and any other notifications.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let mut delivered_text: Option<String> = None;
    while tokio::time::Instant::now() < deadline {
        let remaining = deadline - tokio::time::Instant::now();
        let Some(frame) = next_text(&mut rx, remaining).await else { break };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&frame) else { continue };
        if v.get("method").and_then(|m| m.as_str()) != Some("ainm.message.deliver") {
            continue;
        }
        let msg = v.get("params").and_then(|p| p.get("message"));
        let from = msg.and_then(|m| m.get("from")).and_then(|f| f.as_str()).unwrap_or("");
        if from != agent_addr {
            continue;
        }
        let text = msg
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|b| b.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        delivered_text = Some(text);
        break;
    }

    let _ = child.kill().await;

    let text = delivered_text.expect("agent never delivered a reply within 20s");
    assert!(
        text.contains(llm::MOCK_LLM_REPLY),
        "delivered reply did not contain the canned mock text.\n  got: {text}\n  want substring: {}",
        llm::MOCK_LLM_REPLY
    );
}

/// Read the next text frame within `timeout`, skipping ping/pong/binary. None on timeout/close.
async fn next_text(
    rx: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
    timeout: Duration,
) -> Option<String> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.checked_duration_since(tokio::time::Instant::now())?;
        match tokio::time::timeout(remaining, rx.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => return Some(t.to_string()),
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => return None,
            Ok(Some(Ok(_))) => continue, // ping/pong/binary — keep waiting
            Ok(Some(Err(_))) => return None,
            Err(_) => return None, // timed out
        }
    }
}
```

- [ ] **Step 2: Verify the test compiles**

Run: `cargo test -p ainm-server --test ws_conversation_e2e --no-run 2>&1 | tail -20`
Expected: compiles (test binary built). If `next_text`'s generic bound fails to infer, the fix is to monomorphize against the concrete split stream type — but try the generic form first.

- [ ] **Step 3: Run the test (skips cleanly without a DB; passes with one)**

Run (no DB): `env -u TEST_DATABASE_URL -u AINM_DATABASE__URL -u DATABASE_URL cargo test -p ainm-server --test ws_conversation_e2e -- --nocapture`
Expected: prints `no DB; skipping ws_conversation_delivers_agent_reply`, test PASSES (skip path).

Run (live, against local Postgres):
`AINM_DATABASE__URL="postgres://santiagogerickeparga@localhost:5432/ainm" cargo test -p ainm-server --test ws_conversation_e2e -- --nocapture`
Expected: PASS — the delivered reply contains the canned mock text.

If it fails at the readiness poll, inspect the captured stderr tail in the panic; if it fails at the handshake (`handshake rejected`), the account/key binding insert or domain mismatch is the cause (the `--identity` domain must equal the account `domain` and the address's domain part).

- [ ] **Step 4: Clippy clean on the new test**

Run: `cargo clippy -p ainm-server --test ws_conversation_e2e -- -D warnings 2>&1 | tail -20`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add server/tests/ws_conversation_e2e.rs
rtk proxy git commit -m "test(server): black-box WebSocket conversation E2E (handshake → agent → signed deliver)"
```

---

## Task 3: Live-run verification + runbook note + full suite + DoD

**Files:**
- Modify: `docs/runbooks/2026-06-02-ios11-close-the-loop.md` (add the `AINM_LLM__MOCK=1` note)

**Context:** The bonus of the affordance: `AINM_LLM__MOCK=1 make dogfood` lets the human IOS-11 run produce a streamed canned reply with no OpenAI key. Document it. Then confirm the whole server suite is green except the two known pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence` — unrelated compound-memory graph tests).

- [ ] **Step 1: Add the runbook note**

In `docs/runbooks/2026-06-02-ios11-close-the-loop.md`, locate the section that documents the `make dogfood` / `AINM_LLM__API_KEY` launch (the run-config / "start the server" step) and add a note immediately after it:

```markdown
> **No OpenAI key?** Launch with `AINM_LLM__MOCK=1 make dogfood` (or set `AINM_LLM__MOCK=1`
> in the environment before any dogfood command). The hosted agent then replies with a fixed
> canned message instead of calling a real LLM — enough to see the full handshake → agent →
> streamed delivery loop render in the app. `AINM_LLM__MOCK` takes precedence over
> `AINM_LLM__API_KEY`; unset it to use the real model. Dev/test only.
```

(If the runbook's exact phrasing/anchor differs, place the note adjacent to wherever the server launch + LLM key is described — the requirement is that an operator reading the launch step learns about the key-free option.)

- [ ] **Step 2: Run the live WS E2E test and confirm it passes**

Run: `AINM_DATABASE__URL="postgres://santiagogerickeparga@localhost:5432/ainm" cargo test -p ainm-server --test ws_conversation_e2e -- --nocapture`
Expected: `test ws_conversation_delivers_agent_reply ... ok`.

- [ ] **Step 3: Run the full server test suite live**

Run: `AINM_DATABASE__URL="postgres://santiagogerickeparga@localhost:5432/ainm" cargo test -p ainm-server 2>&1 | tail -40`
Expected: all green EXCEPT the two known pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`). The new `ws_conversation_e2e` and the `constant_mock_*` unit test pass. If any *other* test regressed, stop and fix it.

- [ ] **Step 4: Workspace clippy clean**

Run: `cargo clippy -p ainm-server --all-targets -- -D warnings 2>&1 | tail -20`
Expected: no warnings.

- [ ] **Step 5: Confirm the DoD (all 6 items from the spec §7)**

Verify and note each: (1) `ConstantMockLlmClient` exists + unit-tested; (2) `AINM_LLM__MOCK` truthy selects it with precedence + logs; (3) the E2E drives the round-trip and asserts the canned text live; (4) it skips cleanly with no DB; (5) full suite green except the two known failures + clippy clean; (6) the runbook documents `AINM_LLM__MOCK=1`.

- [ ] **Step 6: Commit**

```bash
rtk proxy git add docs/runbooks/2026-06-02-ios11-close-the-loop.md
rtk proxy git commit -m "docs(wse2e): document AINM_LLM__MOCK=1 for key-free IOS-11 dogfood reply"
```

---

## Self-Review Notes

- **Spec coverage:** §3 affordance → Task 1; §4 test flow → Task 2; §7 DoD items 1–2 → Task 1, 3–4 → Task 2, 5–6 → Task 3. All covered.
- **Type consistency:** `MOCK_LLM_REPLY` (defined Task 1) is referenced in Task 2's assertion and Task 1's wiring — same const. `ConstantMockLlmClient { reply }` field name consistent. `next_text` helper used in Task 2 handshake + reply loop.
- **Implementer adaptation points (flagged in spec):** the exact `next_text` generic bound (Task 2 Step 2 fallback), and the runbook anchor (Task 3 Step 1 note). Both have explicit fallback guidance.
- **No placeholders:** every code step shows complete code; every run step shows the command + expected output.
