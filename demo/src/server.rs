use crate::identity::IdentityStore;
use crate::protocol;
use crate::signing;
use anyhow::Result;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

/// Shared server state accessible by all handlers.
#[derive(Clone)]
pub struct ServerState {
    pub identity_store: IdentityStore,
    pub agent_card: serde_json::Value,
    /// Sender for messages going to the agent.
    pub to_agent_tx: broadcast::Sender<String>,
    /// Sender for messages going to the human (from agent).
    pub to_human_tx: broadcast::Sender<String>,
    /// Active threads (thread_id list).
    pub threads: Arc<Mutex<Vec<String>>>,
    /// In-memory archival fact store keyed by fact key.
    pub archival_store: Arc<Mutex<HashMap<String, serde_json::Value>>>,
}

/// Start the Axum WebSocket server on a random available port.
/// Returns the bound address and the server task handle.
pub async fn start(
    identity_store: IdentityStore,
    agent_card: serde_json::Value,
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let (to_agent_tx, _) = broadcast::channel::<String>(256);
    let (to_human_tx, _) = broadcast::channel::<String>(256);

    let state = ServerState {
        identity_store,
        agent_card,
        to_agent_tx,
        to_human_tx,
        threads: Arc::new(Mutex::new(Vec::new())),
        archival_store: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/ws", get(ws_upgrade_handler))
        .route(
            "/.well-known/ainm/helper/agent-card.json",
            get(agent_card_handler),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    Ok((addr, handle))
}

/// Agent Card HTTP endpoint with ETag caching (per DISC-05).
async fn agent_card_handler(
    headers: HeaderMap,
    State(state): State<ServerState>,
) -> Response {
    let body = serde_json::to_vec(&state.agent_card).unwrap();
    let hash = ring::digest::digest(&ring::digest::SHA256, &body);
    let etag_value = format!(
        "\"{}\"",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash.as_ref())
    );

    // Check If-None-Match
    if let Some(inm) = headers.get(header::IF_NONE_MATCH) {
        if let Ok(inm_str) = inm.to_str() {
            if inm_str == etag_value {
                return (
                    StatusCode::NOT_MODIFIED,
                    [(header::ETAG, etag_value)],
                )
                    .into_response();
            }
        }
    }

    // Return 200 with body and ETag
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json".to_string()),
            (header::ETAG, etag_value),
        ],
        body,
    )
        .into_response()
}

/// WebSocket upgrade handler.
async fn ws_upgrade_handler(
    ws: WebSocketUpgrade,
    State(state): State<ServerState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_connection(socket, state))
}

/// Handle a single WebSocket connection.
async fn handle_ws_connection(socket: WebSocket, state: ServerState) {
    let (ws_sender, mut ws_receiver) = socket.split();
    let ws_sender = Arc::new(Mutex::new(ws_sender));

    // Wait for ainm.initialize request
    let init_text = match ws_receiver.next().await {
        Some(Ok(Message::Text(text))) => text.to_string(),
        _ => return,
    };

    let init_req: serde_json::Value = match serde_json::from_str(&init_text) {
        Ok(v) => v,
        Err(_) => return,
    };

    let req_id = init_req.get("id").cloned().unwrap_or(serde_json::json!(1));
    let from_addr = init_req
        .get("params")
        .and_then(|p| p.get("identity"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    // Send ainm.initialized response
    let init_response = protocol::new_response(
        req_id,
        serde_json::json!({
            "protocolVersion": "0.1.0",
            "serverInfo": {
                "name": "demo.ainm.local",
                "version": "0.1.0"
            },
            "capabilities": {
                "streaming": true,
                "subThreads": true,
                "storeAndForward": false
            },
            "nonce": protocol::new_message_id()
        }),
    );
    {
        let resp_text = serde_json::to_string(&init_response).unwrap();
        let mut sender = ws_sender.lock().await;
        if sender.send(Message::Text(resp_text.into())).await.is_err() {
            return;
        }
    }

    // Wait for ainm.initialized notification before allowing any methods (transport.md Section 4.2)
    loop {
        match ws_receiver.next().await {
            Some(Ok(Message::Text(text))) => {
                let text_str = text.to_string();
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text_str) {
                    let method = parsed.get("method").and_then(|m| m.as_str()).unwrap_or("");
                    if method == "ainm.initialized" {
                        tracing::info!("Handshake complete for: {}", from_addr);
                        break;
                    }
                    // Any other method before initialized -> -32021 (transport.md Section 7)
                    if let Some(req_id) = parsed.get("id").cloned() {
                        let err = protocol::new_error(req_id, -32021, "Not initialized");
                        let mut sender = ws_sender.lock().await;
                        let _ = sender.send(Message::Text(
                            serde_json::to_string(&err).unwrap().into(),
                        )).await;
                    }
                    // Notifications without id: silently drop per JSON-RPC spec
                }
            }
            _ => return, // Connection closed or binary frame
        }
    }

    // Determine if this is the agent or the human based on address
    let is_agent = from_addr.starts_with("helper");

    if is_agent {
        handle_agent_connection(ws_sender, ws_receiver, state, from_addr).await;
    } else {
        handle_human_connection(ws_sender, ws_receiver, state).await;
    }
}

/// Handle the agent's WebSocket connection.
async fn handle_agent_connection(
    ws_sender: Arc<Mutex<futures_util::stream::SplitSink<WebSocket, Message>>>,
    mut ws_receiver: futures_util::stream::SplitStream<WebSocket>,
    state: ServerState,
    from_addr: String,
) {
    let mut from_human_rx = state.to_agent_tx.subscribe();
    let to_human_tx = state.to_human_tx.clone();
    let identity_store = state.identity_store.clone();
    let archival_store = state.archival_store.clone();
    let ws_sender_clone = ws_sender.clone();

    // Task: forward human messages to agent WebSocket
    let forward_handle = tokio::spawn(async move {
        while let Ok(msg) = from_human_rx.recv().await {
            let mut sender = ws_sender_clone.lock().await;
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Main loop: receive agent messages and forward to human
    while let Some(Ok(msg)) = ws_receiver.next().await {
        if let Message::Text(text) = msg {
            let text_str = text.to_string();
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text_str) {
                let method = parsed
                    .get("method")
                    .and_then(|m| m.as_str())
                    .unwrap_or("");
                let has_id = parsed.get("id").is_some();

                match method {
                    "ainm.message.send" => {
                        if let Some(params) = parsed.get("params") {
                            let from = params
                                .get("from")
                                .and_then(|f| f.as_str())
                                .unwrap_or("");
                            let verified = if let Some(pk) =
                                identity_store.get_public_key_bytes(from)
                            {
                                signing::verify_signature(&pk, params).unwrap_or(false)
                            } else {
                                false
                            };

                            let deliver = protocol::new_notification(
                                "ainm.message.deliver",
                                serde_json::json!({
                                    "message": params,
                                    "verified": verified,
                                }),
                            );
                            let deliver_text = serde_json::to_string(&deliver).unwrap();
                            let _ = to_human_tx.send(deliver_text);

                            if has_id {
                                let resp = protocol::new_response(
                                    parsed.get("id").cloned().unwrap(),
                                    serde_json::json!({
                                        "accepted": true,
                                        "server_timestamp": protocol::now_rfc3339()
                                    }),
                                );
                                let mut sender = ws_sender.lock().await;
                                let _ = sender
                                    .send(Message::Text(
                                        serde_json::to_string(&resp).unwrap().into(),
                                    ))
                                    .await;
                            }
                        }
                    }
                    "ainm.memory.store" => {
                        if let Some(params) = parsed.get("params") {
                            let tier = params
                                .get("tier")
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            let thread_id = params
                                .get("thread_id")
                                .and_then(|t| t.as_str())
                                .unwrap_or("")
                                .to_string();

                            if tier != "archival" {
                                // Error -32051: unsupported tier
                                if has_id {
                                    let err = protocol::new_error(
                                        parsed.get("id").cloned().unwrap(),
                                        -32051,
                                        "Only archival tier is supported",
                                    );
                                    let mut sender = ws_sender.lock().await;
                                    let _ = sender
                                        .send(Message::Text(
                                            serde_json::to_string(&err).unwrap().into(),
                                        ))
                                        .await;
                                }
                            } else if let Some(fact) = params.get("fact") {
                                let key = fact
                                    .get("key")
                                    .and_then(|k| k.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                let value = fact.get("value").cloned().unwrap_or(serde_json::json!(""));
                                let tags = fact
                                    .get("tags")
                                    .cloned()
                                    .unwrap_or(serde_json::json!([]));
                                let fact_id = protocol::new_message_id();
                                let stored_at = protocol::now_rfc3339();
                                let from_identity = from_addr.clone();

                                let archival_fact = serde_json::json!({
                                    "fact_id": fact_id,
                                    "key": key,
                                    "value": value,
                                    "tags": tags,
                                    "stored_by": from_identity,
                                    "stored_at": stored_at
                                });

                                // Store in archival_store
                                archival_store.lock().await.insert(key.clone(), archival_fact.clone());

                                // Send response
                                if has_id {
                                    let resp = protocol::new_response(
                                        parsed.get("id").cloned().unwrap(),
                                        archival_fact,
                                    );
                                    let mut sender = ws_sender.lock().await;
                                    let _ = sender
                                        .send(Message::Text(
                                            serde_json::to_string(&resp).unwrap().into(),
                                        ))
                                        .await;
                                }

                                // Forward the store result as a notification to human for rendering
                                let store_notify = protocol::new_notification(
                                    "ainm.memory.stored",
                                    serde_json::json!({
                                        "thread_id": thread_id,
                                        "key": key,
                                        "status": "stored"
                                    }),
                                );
                                let _ = to_human_tx.send(serde_json::to_string(&store_notify).unwrap());
                            }
                        }
                    }
                    "ainm.memory.query" => {
                        if let Some(params) = parsed.get("params") {
                            let thread_id = params
                                .get("thread_id")
                                .and_then(|t| t.as_str())
                                .unwrap_or("")
                                .to_string();
                            let key_prefix = params
                                .get("key")
                                .and_then(|k| k.as_str())
                                .unwrap_or("")
                                .to_string();
                            let limit = params
                                .get("limit")
                                .and_then(|l| l.as_u64())
                                .unwrap_or(10) as usize;

                            let store = archival_store.lock().await;
                            let results: Vec<serde_json::Value> = store
                                .iter()
                                .filter(|(k, _)| key_prefix.is_empty() || k.starts_with(&key_prefix))
                                .take(limit)
                                .map(|(_, v)| v.clone())
                                .collect();

                            let query_result = serde_json::json!({
                                "results": results,
                                "truncated": false
                            });

                            // Send query response
                            if has_id {
                                let resp = protocol::new_response(
                                    parsed.get("id").cloned().unwrap(),
                                    query_result.clone(),
                                );
                                let mut sender = ws_sender.lock().await;
                                let _ = sender
                                    .send(Message::Text(
                                        serde_json::to_string(&resp).unwrap().into(),
                                    ))
                                    .await;
                            }

                            // Forward query results to human for rendering
                            let query_notify = protocol::new_notification(
                                "ainm.memory.query.result",
                                query_result,
                            );
                            let _ = to_human_tx.send(serde_json::to_string(&query_notify).unwrap());

                            // Build and send ainm.context.frame notification
                            let archival_facts: Vec<serde_json::Value> = store.values().cloned().collect();
                            let now = protocol::now_rfc3339();
                            let msg_id = protocol::new_message_id();

                            let context_frame = protocol::new_notification(
                                "ainm.context.frame",
                                serde_json::json!({
                                    "thread_id": thread_id,
                                    "frame": {
                                        "active_context": {
                                            "recent_messages": [
                                                {
                                                    "id": msg_id,
                                                    "from": "alice@demo.ainm.local",
                                                    "timestamp": now,
                                                    "summary": "User requested memory demo"
                                                }
                                            ],
                                            "window_start": now,
                                            "message_count": 1
                                        },
                                        "conversation_summary": "Demo thread exercising memory operations.",
                                        "archival_facts": archival_facts,
                                        "participants": [
                                            { "identity": "alice@demo.ainm.local", "type": "human", "presence": "online" },
                                            { "identity": "helper@demo.ainm.local", "type": "agent", "presence": "available" }
                                        ]
                                    }
                                }),
                            );
                            let _ = to_human_tx.send(serde_json::to_string(&context_frame).unwrap());
                        }
                    }
                    "ainm.memory.evict" => {
                        if let Some(params) = parsed.get("params") {
                            let tier = params
                                .get("tier")
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            let thread_id = params
                                .get("thread_id")
                                .and_then(|t| t.as_str())
                                .unwrap_or("")
                                .to_string();

                            if tier != "archival" {
                                if has_id {
                                    let err = protocol::new_error(
                                        parsed.get("id").cloned().unwrap(),
                                        -32051,
                                        "Only archival tier supports eviction",
                                    );
                                    let mut sender = ws_sender.lock().await;
                                    let _ = sender
                                        .send(Message::Text(
                                            serde_json::to_string(&err).unwrap().into(),
                                        ))
                                        .await;
                                }
                            } else {
                                let key = params
                                    .get("key")
                                    .and_then(|k| k.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                if key.is_empty() {
                                    if has_id {
                                        let err = protocol::new_error(
                                            parsed.get("id").cloned().unwrap(),
                                            -32053,
                                            "Missing required parameter: key",
                                        );
                                        let mut sender = ws_sender.lock().await;
                                        let _ = sender
                                            .send(Message::Text(
                                                serde_json::to_string(&err).unwrap().into(),
                                            ))
                                            .await;
                                    }
                                } else {
                                    let mut store = archival_store.lock().await;
                                    let evicted = store.remove(&key).is_some();

                                    if evicted {
                                        if has_id {
                                            let resp = protocol::new_response(
                                                parsed.get("id").cloned().unwrap(),
                                                serde_json::json!({ "evicted": true }),
                                            );
                                            let mut sender = ws_sender.lock().await;
                                            let _ = sender
                                                .send(Message::Text(
                                                    serde_json::to_string(&resp).unwrap().into(),
                                                ))
                                                .await;
                                        }
                                        let evict_notify = protocol::new_notification(
                                            "ainm.memory.evicted",
                                            serde_json::json!({
                                                "thread_id": thread_id,
                                                "key": key,
                                                "status": "evicted"
                                            }),
                                        );
                                        let _ = to_human_tx
                                            .send(serde_json::to_string(&evict_notify).unwrap());
                                    } else if has_id {
                                        let err = protocol::new_error(
                                            parsed.get("id").cloned().unwrap(),
                                            -32054,
                                            "Fact not found",
                                        );
                                        let mut sender = ws_sender.lock().await;
                                        let _ = sender
                                            .send(Message::Text(
                                                serde_json::to_string(&err).unwrap().into(),
                                            ))
                                            .await;
                                    }
                                }
                            }
                        }
                    }
                    "ainm.stream.start"
                    | "ainm.stream.delta"
                    | "ainm.stream.end"
                    | "ainm.tool.call.started"
                    | "ainm.tool.call.progress"
                    | "ainm.tool.call.completed"
                    | "ainm.presence.update" => {
                        let _ = to_human_tx.send(text_str);
                    }
                    _ => {
                        tracing::debug!("Agent sent unknown method: {}", method);
                    }
                }
            }
        }
    }

    forward_handle.abort();
}

/// Handle the human's WebSocket connection.
async fn handle_human_connection(
    ws_sender: Arc<Mutex<futures_util::stream::SplitSink<WebSocket, Message>>>,
    mut ws_receiver: futures_util::stream::SplitStream<WebSocket>,
    state: ServerState,
) {
    let mut from_agent_rx = state.to_human_tx.subscribe();
    let to_agent_tx = state.to_agent_tx.clone();
    let identity_store = state.identity_store.clone();
    let threads = state.threads.clone();
    let ws_sender_clone = ws_sender.clone();

    // Task: forward agent messages/notifications to human WebSocket
    let forward_handle = tokio::spawn(async move {
        while let Ok(msg) = from_agent_rx.recv().await {
            let mut sender = ws_sender_clone.lock().await;
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Main loop: receive human messages
    while let Some(Ok(msg)) = ws_receiver.next().await {
        if let Message::Text(text) = msg {
            let text_str = text.to_string();
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text_str) {
                let method = parsed
                    .get("method")
                    .and_then(|m| m.as_str())
                    .unwrap_or("");
                let req_id = parsed.get("id").cloned().unwrap_or(serde_json::json!(null));

                match method {
                    "ainm.thread.create" => {
                        let thread_id = parsed
                            .get("params")
                            .and_then(|p| p.get("thread_id"))
                            .and_then(|t| t.as_str())
                            .unwrap_or("")
                            .to_string();
                        threads.lock().await.push(thread_id.clone());
                        let resp = protocol::new_response(
                            req_id,
                            serde_json::json!({
                                "thread_id": thread_id,
                                "created": true
                            }),
                        );
                        let mut sender = ws_sender.lock().await;
                        let _ = sender
                            .send(Message::Text(
                                serde_json::to_string(&resp).unwrap().into(),
                            ))
                            .await;
                    }
                    "ainm.message.send" => {
                        if let Some(params) = parsed.get("params") {
                            let from = params
                                .get("from")
                                .and_then(|f| f.as_str())
                                .unwrap_or("");
                            let verified = if let Some(pk) =
                                identity_store.get_public_key_bytes(from)
                            {
                                signing::verify_signature(&pk, params).unwrap_or(false)
                            } else {
                                false
                            };

                            let deliver = protocol::new_notification(
                                "ainm.message.deliver",
                                serde_json::json!({
                                    "message": params,
                                    "verified": verified,
                                }),
                            );
                            let deliver_text = serde_json::to_string(&deliver).unwrap();
                            let _ = to_agent_tx.send(deliver_text);

                            let resp = protocol::new_response(
                                req_id,
                                serde_json::json!({
                                    "accepted": true,
                                    "server_timestamp": protocol::now_rfc3339()
                                }),
                            );
                            let mut sender = ws_sender.lock().await;
                            let _ = sender
                                .send(Message::Text(
                                    serde_json::to_string(&resp).unwrap().into(),
                                ))
                                .await;
                        }
                    }
                    "ainm.stream.cancel" => {
                        let _ = to_agent_tx.send(text_str);
                    }
                    _ => {
                        let err = protocol::new_error(req_id, -32601, "Method not found");
                        let mut sender = ws_sender.lock().await;
                        let _ = sender
                            .send(Message::Text(
                                serde_json::to_string(&err).unwrap().into(),
                            ))
                            .await;
                    }
                }
            }
        }
    }

    forward_handle.abort();
}
