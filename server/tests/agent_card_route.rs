//! Regression test locking the client/server agent-card contract: the canonical route
//! `/.well-known/ainm/{agent_name}/agent-card.json` (no `agents/` segment) serves the
//! hosted agent card. Mirrors the RC-5.1 envelope-decode regression style. Black-boxes the
//! compiled binary because `ServerState`/`agent_card_handler` are binary-only (B-8 split).
//!
//! Uses `--mode agent` (not `--mode human`) because human-mode boots with `agent_card = null`
//! (no DB-provisioned agent), so the static fallback has no `name` field to assert on.
//! Agent-mode sets `agent_card` to a real JSON object with a `name` field, enabling the
//! full contract check without requiring Postgres.

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

    // Use --mode agent: boots with a static agent card (name="AINM Federation Helper"),
    // no DB required. /health/ready returns 200 in no-DB mode ("still healthy").
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_ainm-server"))
        .args([
            "--mode", "agent",
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
    if !ready {
        let _ = child.kill().await;
        let mut stderr = String::new();
        if let Some(mut e) = child.stderr.take() {
            use tokio::io::AsyncReadExt;
            let _ = e.read_to_string(&mut stderr).await;
        }
        panic!("server did not become ready in 15s. stderr tail:\n{}", stderr);
    }

    // Canonical route — NO `agents/` segment.
    let card_url = format!("http://127.0.0.1:{port}/.well-known/ainm/helper/agent-card.json");
    let resp = http.get(&card_url).send().await.expect("GET agent-card");
    assert_eq!(resp.status().as_u16(), 200, "canonical agent-card route must return 200");

    let body: serde_json::Value = resp.json().await.expect("agent-card JSON body");
    assert!(
        body.get("name").and_then(|v| v.as_str()).is_some(),
        "agent card must carry a `name` field; got {body}"
    );

    // A bogus `agents/`-prefixed path must NOT serve the card (locks the no-`agents/` contract).
    let bogus = format!("http://127.0.0.1:{port}/.well-known/ainm/agents/helper/agent-card.json");
    let bogus_resp = http.get(&bogus).send().await.expect("GET bogus");
    assert_ne!(
        bogus_resp.status().as_u16(),
        200,
        "the legacy `agents/`-prefixed path must not resolve"
    );

    child.start_kill().ok();
}
