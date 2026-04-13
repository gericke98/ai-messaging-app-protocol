//! End-to-end test: WebSocket → message store → compilation → graph query.
//!
//! Requires the server running at localhost:8080 with PostgreSQL.
//! Run: cargo test --test e2e_compound_memory -- --ignored

use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};

async fn ws_connect(identity: &str) -> (
    futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>,
    futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>,
) {
    let (ws, _) = connect_async("ws://localhost:8080/ws")
        .await
        .expect("Failed to connect WebSocket");
    let (mut sink, mut stream) = ws.split();

    // Handshake: ainm.initialize
    let init = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "ainm.initialize",
        "params": {
            "protocol_version": "0.1.0",
            "identity": identity,
            "signature": "test-sig",
            "capabilities": {
                "streaming": false,
                "sub_threads": false,
                "content_types": ["text"]
            }
        }
    });
    sink.send(Message::Text(serde_json::to_string(&init).unwrap().into())).await.unwrap();

    // Read initialize response
    if let Some(Ok(Message::Text(resp))) = stream.next().await {
        let parsed: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert!(parsed.get("result").is_some(), "Handshake failed: {resp}");
    }

    // Send ainm.initialized notification
    let initialized = json!({
        "jsonrpc": "2.0",
        "method": "ainm.initialized",
        "params": {}
    });
    sink.send(Message::Text(serde_json::to_string(&initialized).unwrap().into())).await.unwrap();

    (sink, stream)
}

fn rpc_request(id: i64, method: &str, params: serde_json::Value) -> String {
    serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params
    })).unwrap()
}

async fn send_and_recv(
    sink: &mut futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>,
    stream: &mut futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>,
    msg: &str,
    expected_id: i64,
) -> serde_json::Value {
    sink.send(Message::Text(msg.into())).await.unwrap();

    // Read responses until we find the one matching our request ID
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
    loop {
        let remaining = deadline - tokio::time::Instant::now();
        match tokio::time::timeout(remaining, stream.next()).await {
            Ok(Some(Ok(Message::Text(resp)))) => {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&resp) {
                    if parsed.get("id").and_then(|i| i.as_i64()) == Some(expected_id) {
                        return parsed;
                    }
                    // Not our response — notification or different ID, skip
                }
            }
            _ => panic!("Timeout waiting for response to request {expected_id}"),
        }
    }
}

#[tokio::test]
#[ignore] // Requires running server
async fn test_e2e_message_store_and_graph_query() {
    let (mut sink, mut stream) = ws_connect("e2e-tester@server-a.local").await;

    // 1. Send a human message
    let msg = rpc_request(10, "ainm.message.send", json!({
        "from": "e2e-tester@server-a.local",
        "thread_id": "e2e-test-thread-001",
        "timestamp": "2026-04-12T00:00:00.000Z",
        "type": "message",
        "content": [{
            "type": "text",
            "version": "1.0",
            "source": "user",
            "text": "I am a software engineer who loves Rust and TypeScript",
            "fallback": "I am a software engineer who loves Rust and TypeScript"
        }],
        "signature": "test-sig"
    }));
    let resp = send_and_recv(&mut sink, &mut stream, &msg, 10).await;
    assert_eq!(resp["result"]["accepted"], true, "Message not accepted: {resp}");
    println!("[E2E] Message sent and accepted");

    // 2. Query graph (may be empty since mock LLM is configured)
    let query = rpc_request(20, "ainm.memory.graph.query", json!({
        "include_edges": true,
        "limit": 50
    }));
    let graph_resp = send_and_recv(&mut sink, &mut stream, &query, 20).await;
    let result = &graph_resp["result"];
    println!("[E2E] Graph query: {} nodes, {} edges",
        result["total_count"], result["edges"].as_array().map(|a| a.len()).unwrap_or(0));

    // 3. Contribute a node directly
    let contribute = rpc_request(30, "ainm.memory.graph.contribute", json!({
        "thread_id": "e2e-test-thread-001",
        "node": {
            "type": "semantic",
            "category": "skill",
            "label": "Expert in Rust programming",
            "confidence": 0.85,
            "properties": {"language": "rust", "level": "expert"},
            "tags": ["programming", "rust"]
        }
    }));
    let contrib_resp = send_and_recv(&mut sink, &mut stream, &contribute, 30).await;
    assert_eq!(contrib_resp["result"]["contributed"], true, "Contribution failed: {contrib_resp}");
    let node_id = contrib_resp["result"]["node_id"].as_str().unwrap().to_string();
    println!("[E2E] Contributed node: {node_id}");

    // 4. Contribute a second node + edge
    let contribute2 = rpc_request(31, "ainm.memory.graph.contribute", json!({
        "thread_id": "e2e-test-thread-001",
        "node": {
            "type": "semantic",
            "category": "tool",
            "label": "Uses Axum web framework",
            "confidence": 0.8,
            "properties": {"framework": "axum"},
            "tags": ["rust", "web"]
        },
        "edges": [{
            "target_node": node_id,
            "relationship": "used_with",
            "confidence": 0.85
        }]
    }));
    let contrib2_resp = send_and_recv(&mut sink, &mut stream, &contribute2, 31).await;
    assert_eq!(contrib2_resp["result"]["contributed"], true);
    let edge_ids = contrib2_resp["result"]["edge_ids"].as_array().unwrap();
    println!("[E2E] Contributed second node + {} edge(s)", edge_ids.len());

    // 5. Query graph again — should have our nodes
    let query2 = rpc_request(40, "ainm.memory.graph.query", json!({
        "query": "Rust",
        "include_edges": true,
        "limit": 50
    }));
    let graph2 = send_and_recv(&mut sink, &mut stream, &query2, 40).await;
    let nodes = graph2["result"]["nodes"].as_array().unwrap();
    assert!(!nodes.is_empty(), "Expected nodes for 'Rust' query, got none");
    println!("[E2E] Graph query 'Rust': {} nodes found", nodes.len());

    // 6. Traverse from the first node
    let traverse = rpc_request(50, "ainm.memory.graph.traverse", json!({
        "start_node": node_id,
        "max_depth": 2,
        "direction": "both"
    }));
    let trav_resp = send_and_recv(&mut sink, &mut stream, &traverse, 50).await;
    let subgraph = &trav_resp["result"]["subgraph"];
    let trav_nodes = subgraph["nodes"].as_array().unwrap();
    let trav_edges = subgraph["edges"].as_array().unwrap();
    println!("[E2E] Traversal from {}: {} nodes, {} edges",
        &node_id[..8], trav_nodes.len(), trav_edges.len());
    assert!(trav_nodes.len() >= 1, "Traversal should find connected nodes");

    // 7. Run lint
    let lint = rpc_request(60, "ainm.memory.lint", json!({}));
    let lint_resp = send_and_recv(&mut sink, &mut stream, &lint, 60).await;
    let summary = &lint_resp["result"]["summary"];
    println!("[E2E] Lint: {} nodes, {} edges, {} contradictions, {} orphans",
        summary["total_nodes"], summary["total_edges"],
        summary["contradictions"], summary["orphan_nodes"]);

    // 8. Export memory
    let export = rpc_request(70, "ainm.memory.export", json!({"include_superseded": false}));
    let export_resp = send_and_recv(&mut sink, &mut stream, &export, 70).await;
    assert_eq!(export_resp["result"]["format"], "ainm-memory-v1");
    let meta = &export_resp["result"]["metadata"];
    println!("[E2E] Export: ainm-memory-v1, {} nodes, {} edges",
        meta["node_count"], meta["edge_count"]);

    // 9. Grant + briefing
    let grant = rpc_request(80, "ainm.memory.grant", json!({
        "agent_identity": "test-agent@server-b.local",
        "scopes": ["identity", "preferences", "expertise"],
        "consent_level": "standard"
    }));
    let grant_resp = send_and_recv(&mut sink, &mut stream, &grant, 80).await;
    assert_eq!(grant_resp["result"]["granted"], true);
    println!("[E2E] Granted scopes to test-agent");

    let briefing = rpc_request(90, "ainm.memory.briefing", json!({
        "thread_id": "e2e-test-thread-001",
        "user_identity": "e2e-tester@server-a.local"
    }));
    let briefing_resp = send_and_recv(&mut sink, &mut stream, &briefing, 90).await;
    assert!(briefing_resp["result"]["briefing_id"].is_string());
    let sections = &briefing_resp["result"]["sections"];
    println!("[E2E] Briefing generated: scope={}, sections={:?}",
        briefing_resp["result"]["scope"],
        sections.as_object().map(|o| o.keys().collect::<Vec<_>>()));

    println!("\n[E2E] ALL CHECKS PASSED");
}
