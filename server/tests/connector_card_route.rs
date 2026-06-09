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
    sqlx::migrate::Migrator::new(migrations_dir)
        .await
        .ok()?
        .run(&pool)
        .await
        .ok()?;
    Some((pool, url))
}

/// Insert an active, email-verified account and bind a freshly generated Ed25519 key.
/// Mirrors ws_conversation_e2e.rs::make_bound_account exactly.
async fn make_bound_account(
    pool: &sqlx::PgPool,
    address: &str,
    domain: &str,
) -> identity::Identity {
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
        eprintln!(
            "no DB; skipping connector_card_published_while_connected_then_404_on_disconnect"
        );
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
            "--mode",
            "human",
            "--port",
            &port.to_string(),
            "--identity",
            domain,
            "--cert",
            certs.join("server-a.pem").to_str().unwrap(),
            "--key",
            certs.join("server-a-key.pem").to_str().unwrap(),
            "--ca-cert",
            certs.join("ca.pem").to_str().unwrap(),
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
            if resp.status().is_success() {
                ready = true;
                break;
            }
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
    let (ws_stream, _) = tokio::time::timeout(
        Duration::from_secs(5),
        tokio_tungstenite::connect_async(&ws_url),
    )
    .await
    .expect("ws connect timed out")
    .expect("ws connect");
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
    tx.send(Message::Text(
        serde_json::to_string(&init_req).unwrap().into(),
    ))
    .await
    .unwrap();
    let init_resp = next_text(&mut rx, Duration::from_secs(10))
        .await
        .expect("init response");
    let init_resp_json: serde_json::Value = serde_json::from_str(&init_resp).unwrap();
    assert!(
        init_resp_json.get("result").is_some(),
        "handshake rejected: {init_resp}"
    );
    let initialized = protocol::new_notification("ainm.initialized", serde_json::json!({}));
    tx.send(Message::Text(
        serde_json::to_string(&initialized).unwrap().into(),
    ))
    .await
    .unwrap();

    // Give the server a moment to register the card, then GET the well-known route.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let card_url =
        format!("http://127.0.0.1:{port}/.well-known/ainm/{agent_local}/agent-card.json");
    let resp = http
        .get(&card_url)
        .send()
        .await
        .expect("GET connector card");
    assert_eq!(
        resp.status().as_u16(),
        200,
        "connector card must be 200 while connected"
    );
    let body: serde_json::Value = resp.json().await.expect("card JSON");
    assert_eq!(body["name"], "Coder", "got {body}");
    assert_eq!(body["ainm"]["address"], agent_addr, "got {body}");
    assert_eq!(
        body["ainm"]["tools"][0]["name"], "run_tests",
        "tool must be published; got {body}"
    );
    assert_eq!(
        body["capabilities"]["pushNotifications"], false,
        "camelCase key; got {body}"
    );

    // Disconnect: drop the WS, then the route must 404 once cleanup runs.
    drop(tx);
    drop(rx);
    let mut got_404 = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Ok(r) = http.get(&card_url).send().await {
            if r.status().as_u16() == 404 {
                got_404 = true;
                break;
            }
        }
    }

    let _ = child.kill().await;
    cleanup_domain(&pool, domain).await;
    assert!(
        got_404,
        "route must 404 within ~5s after the agent disconnects"
    );
}
