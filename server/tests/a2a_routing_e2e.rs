//! Runtime A2A (P1) black-box test: two agents in one thread address each other,
//! the hop cap trips after N consecutive agent→agent hops, a system notice is
//! delivered to the human, and a human message resets the circuit. DB-or-skip.

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

async fn make_bound_account(pool: &sqlx::PgPool, address: &str, domain: &str) -> identity::Identity {
    let username = address.split('@').next().unwrap();
    let account_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (username, domain, address, email_hash, email_verified, status) \
         VALUES ($1, $2, $3, $4, true, 'active') RETURNING id",
    )
    .bind(username).bind(domain).bind(address).bind(format!("hash-{address}"))
    .fetch_one(pool).await.expect("insert account");
    let id = identity::generate_keypair(address).expect("keypair");
    let pk_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&id.public_key_bytes);
    sqlx::query("INSERT INTO account_keys (account_id, public_key) VALUES ($1, $2)")
        .bind(account_id).bind(&pk_b64).execute(pool).await.expect("insert key");
    id
}

async fn cleanup_domain(pool: &sqlx::PgPool, domain: &str) {
    let _ = sqlx::query("DELETE FROM hosted_agents WHERE address LIKE $1")
        .bind(format!("%@{domain}")).execute(pool).await;
    let _ = sqlx::query("DELETE FROM accounts WHERE domain = $1").bind(domain).execute(pool).await;
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
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => return None,
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(_))) => return None,
            Err(_) => return None,
        }
    }
}

type WsSink = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;
type WsStream = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
>;

async fn connect_and_handshake(
    ws_url: &str,
    id: &identity::Identity,
    participant_type: &str,
) -> (WsSink, WsStream) {
    let (ws_stream, _) = tokio::time::timeout(Duration::from_secs(5),
        tokio_tungstenite::connect_async(ws_url))
        .await.expect("ws connect timeout").expect("ws connect");
    let (mut tx, mut rx) = ws_stream.split();
    let (sig, ts) = signing::sign_init(&id.key_pair, &id.address).unwrap();
    let init_req = protocol::new_request(
        "ainm.initialize",
        serde_json::json!({
            "identity": id.address,
            "protocolVersion": "0.1.0",
            "signature": sig,
            "timestamp": ts,
            "capabilities": {
                "streaming": true,
                "contentTypes": ["text"],
                "participant_type": participant_type,
            }
        }),
        serde_json::json!(2),
    );
    tx.send(Message::Text(serde_json::to_string(&init_req).unwrap().into())).await.unwrap();
    let resp = next_text(&mut rx, Duration::from_secs(10)).await.expect("init response");
    let resp_json: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert!(resp_json.get("result").is_some(), "handshake rejected: {resp}");
    let initialized = protocol::new_notification("ainm.initialized", serde_json::json!({}));
    tx.send(Message::Text(serde_json::to_string(&initialized).unwrap().into())).await.unwrap();
    (tx, rx)
}

async fn send_addressed(tx: &mut WsSink, sender: &identity::Identity, thread_id: &str, to: &str, text: &str) {
    let mut envelope = serde_json::json!({
        "id": protocol::new_message_id(),
        "from": sender.address,
        "thread_id": thread_id,
        "timestamp": protocol::now_rfc3339(),
        "type": "message",
        "content": [{
            "type": "text", "version": "1.0", "source": "agent",
            "text": text, "fallback": text,
        }],
        "to": to,
    });
    signing::sign_envelope(&sender.key_pair, &mut envelope).expect("sign");
    let req = protocol::new_request("ainm.message.send", envelope, serde_json::json!(protocol::new_message_id()));
    tx.send(Message::Text(serde_json::to_string(&req).unwrap().into())).await.unwrap();
}

async fn await_deliver_from(rx: &mut WsStream, from: &str, timeout: Duration) -> Option<(String, String)> {
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        let remaining = deadline.checked_duration_since(tokio::time::Instant::now()).unwrap_or(Duration::ZERO);
        let frame = next_text(rx, remaining).await?;
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&frame) else { continue };
        if v.get("method").and_then(|m| m.as_str()) != Some("ainm.message.deliver") { continue; }
        let params = v.get("params");
        let msg = params.and_then(|p| p.get("message"));
        let msg_from = msg.and_then(|m| m.get("from")).and_then(|f| f.as_str()).unwrap_or("");
        if msg_from != from { continue; }
        let sender_type = params.and_then(|p| p.get("sender_type")).and_then(|s| s.as_str()).unwrap_or("").to_string();
        let text = msg.and_then(|m| m.get("content")).and_then(|c| c.as_array())
            .and_then(|a| a.first()).and_then(|b| b.get("text")).and_then(|t| t.as_str())
            .unwrap_or("").to_string();
        return Some((text, sender_type));
    }
    None
}

#[tokio::test]
async fn agent_to_agent_routes_and_hop_cap_trips() {
    let Some((pool, db_url)) = pool_and_url_or_skip().await else {
        eprintln!("no DB; skipping agent_to_agent_routes_and_hop_cap_trips");
        return;
    };

    let run = format!("{:x}", uuid::Uuid::new_v4().as_u128());
    let domain = format!("a2a-{run}.local");
    let alice = make_bound_account(&pool, &format!("alice@{domain}"), &domain).await;
    let agent_a = make_bound_account(&pool, &format!("alpha@{domain}"), &domain).await;
    let agent_b = make_bound_account(&pool, &format!("beta@{domain}"), &domain).await;

    let port = free_port().await;
    let certs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("certs");
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_ainm-server"))
        .args([
            "--mode", "human", "--port", &port.to_string(), "--identity", &domain,
            "--cert", certs.join("server-a.pem").to_str().unwrap(),
            "--key", certs.join("server-a-key.pem").to_str().unwrap(),
            "--ca-cert", certs.join("ca.pem").to_str().unwrap(),
        ])
        .env("AINM_DATABASE__URL", &db_url)
        .env("AINM_A2A__HOP_CAP", "2")
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null()).stderr(Stdio::piped()).kill_on_drop(true)
        .spawn().expect("spawn ainm-server");

    let http = reqwest::Client::new();
    let ready_url = format!("http://127.0.0.1:{port}/health/ready");
    let mut ready = false;
    for _ in 0..150 {
        if let Ok(r) = http.get(&ready_url).send().await { if r.status().is_success() { ready = true; break; } }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ready, "server did not become ready");

    let ws_url = format!("ws://127.0.0.1:{port}/ws");
    let (mut human_tx, mut _human_rx) = connect_and_handshake(&ws_url, &alice, "human").await;
    let (mut a_tx, mut a_rx) = connect_and_handshake(&ws_url, &agent_a, "agent").await;
    let (mut _b_tx, mut b_rx) = connect_and_handshake(&ws_url, &agent_b, "agent").await;

    let thread_id = protocol::new_message_id();

    send_addressed(&mut human_tx, &alice, &thread_id, &agent_a.address, "kick off please @alpha").await;
    let got = await_deliver_from(&mut a_rx, &alice.address, Duration::from_secs(10)).await
        .expect("agent A should receive the human kickoff");
    assert_eq!(got.1, "human", "kickoff sender_type should be human");

    send_addressed(&mut a_tx, &agent_a, &thread_id, &agent_b.address, "hi @beta, hop 1").await;
    let b_got = await_deliver_from(&mut b_rx, &agent_a.address, Duration::from_secs(10)).await
        .expect("agent B should receive A's first A2A message");
    assert_eq!(b_got.1, "agent", "A→B sender_type should be agent");
    assert!(b_got.0.contains("hop 1"));

    send_addressed(&mut _b_tx, &agent_b, &thread_id, &agent_a.address, "reply @alpha, hop 2").await;
    let a_got = await_deliver_from(&mut a_rx, &agent_b.address, Duration::from_secs(10)).await
        .expect("agent A should receive B's reply (hop 2)");
    assert!(a_got.0.contains("hop 2"));

    send_addressed(&mut a_tx, &agent_a, &thread_id, &agent_b.address, "hop 3 should be blocked @beta").await;

    let blocked = await_deliver_from(&mut b_rx, &agent_a.address, Duration::from_secs(3)).await;
    let hop3_delivered = blocked.as_ref().is_some_and(|(t, _)| t.contains("hop 3"));
    assert!(
        !hop3_delivered,
        "agent B should NOT receive the over-cap hop-3 message, got: {blocked:?}"
    );

    let notice = await_deliver_from(&mut _human_rx, "system@local", Duration::from_secs(5)).await;
    assert!(notice.is_some(), "human should receive the a2a-paused system notice");
    assert_eq!(notice.as_ref().unwrap().1, "system");
    assert!(notice.unwrap().0.to_lowercase().contains("paused"));

    let _ = child.kill().await;
    cleanup_domain(&pool, &domain).await;
}
