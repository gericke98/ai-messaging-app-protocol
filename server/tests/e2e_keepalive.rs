//! E2E: ainm.ping / ainm.pong keepalive (transport.md Section 4.4).
//!
//! Required for iOS background WebSocket correctness (M4): the client pings
//! every 30s idle and treats a missing reply as a dead connection.
//!
//! Requires a server running at localhost:8080 (e.g. `make uat`).
//! Run: cargo test --test e2e_keepalive -- --ignored

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::Message};

const WS_URL: &str = "ws://localhost:8080/ws";

async fn handshake(
    identity: &str,
    participant_type: &str,
) -> (
    futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
) {
    let (ws, _) = connect_async(WS_URL).await.expect("connect");
    let (mut sink, mut stream) = ws.split();

    let init = json!({
        "jsonrpc": "2.0", "id": 1, "method": "ainm.initialize",
        "params": {
            "protocolVersion": "0.1.0",
            "identity": identity,
            "capabilities": { "participant_type": participant_type, "contentTypes": ["text"] }
        }
    });
    sink.send(Message::Text(init.to_string().into()))
        .await
        .unwrap();
    // Read the initialize response.
    while let Some(Ok(msg)) = stream.next().await {
        if let Message::Text(t) = msg {
            let v: serde_json::Value = serde_json::from_str(&t).unwrap();
            if v.get("id") == Some(&json!(1)) {
                break;
            }
        }
    }
    let initialized = json!({"jsonrpc": "2.0", "method": "ainm.initialized", "params": {}});
    sink.send(Message::Text(initialized.to_string().into()))
        .await
        .unwrap();
    (sink, stream)
}

async fn ping_round_trip(identity: &str, participant_type: &str) {
    let (mut sink, mut stream) = handshake(identity, participant_type).await;

    let ping = json!({"jsonrpc": "2.0", "id": 99, "method": "ainm.ping", "params": {}});
    sink.send(Message::Text(ping.to_string().into()))
        .await
        .unwrap();

    let reply = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while let Some(Ok(msg)) = stream.next().await {
            if let Message::Text(t) = msg {
                let v: serde_json::Value = serde_json::from_str(&t).unwrap();
                if v.get("id") == Some(&json!(99)) {
                    return v;
                }
            }
        }
        panic!("stream closed without a ping reply");
    })
    .await
    .expect("no ainm.ping reply within 5s — keepalive not wired");

    assert!(reply.get("error").is_none(), "ping must not error: {reply}");
    let ts = reply
        .pointer("/result/timestamp")
        .and_then(|t| t.as_str())
        .expect("result.timestamp present");
    assert!(
        chrono::DateTime::parse_from_rfc3339(ts).is_ok(),
        "timestamp must be RFC3339: {ts}"
    );
}

#[tokio::test]
#[ignore = "requires a running server at localhost:8080 (make uat)"]
async fn ping_pong_round_trips_for_agent_connection() {
    ping_round_trip("keepalive-agent@test.local", "agent").await;
}

#[tokio::test]
#[ignore = "requires a running server at localhost:8080 (make uat)"]
async fn ping_pong_round_trips_for_human_connection() {
    ping_round_trip("keepalive-human@test.local", "human").await;
}

/// Schema follow-up (M1 ground-truth doc): `direction` must be one of
/// outgoing|incoming|both — garbage used to silently traverse "both".
#[tokio::test]
#[ignore = "requires a running server at localhost:8080 (make uat)"]
async fn traverse_rejects_non_enum_direction() {
    let (mut sink, mut stream) = handshake("dircheck@test.local", "human").await;

    let req = json!({
        "jsonrpc": "2.0", "id": 7, "method": "ainm.memory.graph.traverse",
        "params": {
            "start_node": "00000000-0000-0000-0000-000000000000",
            "direction": "outbound"
        }
    });
    sink.send(Message::Text(req.to_string().into()))
        .await
        .unwrap();

    let reply = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while let Some(Ok(msg)) = stream.next().await {
            if let Message::Text(t) = msg {
                let v: serde_json::Value = serde_json::from_str(&t).unwrap();
                if v.get("id") == Some(&json!(7)) {
                    return v;
                }
            }
        }
        panic!("stream closed without a reply");
    })
    .await
    .expect("no reply within 5s");

    assert_eq!(
        reply.pointer("/error/code").and_then(|c| c.as_i64()),
        Some(-32602),
        "invalid direction must be Invalid params, got: {reply}"
    );
    let msg = reply
        .pointer("/error/message")
        .and_then(|m| m.as_str())
        .unwrap_or("");
    assert!(msg.contains("direction"), "message names the field: {msg}");
}
