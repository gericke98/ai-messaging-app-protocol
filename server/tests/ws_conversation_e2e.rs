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

/// Remove every row for `domain` so the test leaves the shared DB as it found it. The
/// auto-provisioned `assistant@<domain>` account otherwise collides with
/// `hosted_agent_test::bootstrap_is_idempotent` via the GLOBAL `accounts_username_lower_idx`
/// on `lower(username)`. `hosted_agents` has no FK to `accounts`, so delete it explicitly;
/// `account_keys` and `key_escrow` cascade from `accounts` on delete. Best-effort (ignore errors).
async fn cleanup_domain(pool: &sqlx::PgPool, domain: &str) {
    // hosted_agents has no FK to accounts — delete explicitly. accounts cascades to keys/escrow.
    let _ = sqlx::query("DELETE FROM hosted_agents WHERE address LIKE $1")
        .bind(format!("%@{domain}"))
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM accounts WHERE domain = $1")
        .bind(domain)
        .execute(pool)
        .await;
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

    // Fixed domain so the auto-provisioned `assistant@<domain>` survives across runs:
    // `accounts.username` is GLOBALLY unique (`accounts_username_lower_idx` on lower(username)),
    // so a per-run domain would let only the first run ever provision the `assistant` username —
    // every later run would silently fail provisioning and the agent would never reply. A stable
    // domain hits the server's idempotent `ON CONFLICT (address) DO NOTHING` path instead.
    // Only the sender (`alice`) needs a per-run-unique username to avoid the global index.
    let run = format!("{:x}", uuid::Uuid::new_v4().as_u128());
    let domain = "ws-e2e.local".to_string();
    let alice_addr = format!("alice-{run}@{domain}");
    let agent_addr = format!("assistant@{domain}");

    // Clean slate: drop any `ws-e2e.local` rows a prior aborted run may have left behind, so
    // stale state can't interfere with provisioning or assertions below.
    cleanup_domain(&pool, &domain).await;

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
    let (ws_stream, _) = tokio::time::timeout(
        Duration::from_secs(5),
        tokio_tungstenite::connect_async(&ws_url),
    )
    .await
    .expect("ws connect timed out")
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
    let mut delivered_message: Option<serde_json::Value> = None;
    while tokio::time::Instant::now() < deadline {
        let remaining = deadline
            .checked_duration_since(tokio::time::Instant::now())
            .unwrap_or(Duration::ZERO);
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
        delivered_message = msg.cloned();
        break;
    }

    // Fetch the agent's bound public key (the active, non-revoked key for `assistant@<domain>`).
    // Mirrors `accounts::find_active_key` semantics (WHERE revoked_at IS NULL); `account_keys` has
    // no `created_at` column (it uses `enrolled_at`), and the partial unique index
    // `account_keys_one_active_idx` guarantees at most one active key per account.
    let agent_pk_b64: String = sqlx::query_scalar(
        "SELECT ak.public_key FROM account_keys ak \
         JOIN accounts a ON a.id = ak.account_id \
         WHERE a.address = $1 AND ak.revoked_at IS NULL \
         ORDER BY ak.enrolled_at DESC LIMIT 1",
    )
    .bind(&agent_addr)
    .fetch_one(&pool)
    .await
    .expect("agent key provisioned");
    let agent_pk = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(agent_pk_b64.as_bytes())
        .expect("decode agent pk");

    let _ = child.kill().await;

    // Best-effort cleanup BEFORE assertions so a failing assert can't leak rows that would poison
    // hosted_agent_test::bootstrap_is_idempotent (global username index). The agent public key was
    // already fetched above, so deleting the account here does not affect signature verification.
    cleanup_domain(&pool, &domain).await;

    let text = delivered_text.expect("agent never delivered a reply within 20s");
    let delivered_message = delivered_message.expect("agent never delivered a reply within 20s");
    assert!(
        text.contains(llm::MOCK_LLM_REPLY),
        "delivered reply did not contain the canned mock text.\n  got: {text}\n  want substring: {}",
        llm::MOCK_LLM_REPLY
    );
    let sig_ok = signing::verify_signature(&agent_pk, &delivered_message).unwrap_or(false);
    assert!(
        sig_ok,
        "agent reply signature did not verify against the provisioned agent key"
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
