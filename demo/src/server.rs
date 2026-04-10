use crate::federation;
use crate::identity::IdentityStore;
use crate::protocol;
use crate::relay;
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

/// Federation-aware routing context.
///
/// When present, the server routes messages through federation relay
/// for remote participants. When absent (legacy mode), all routing is local.
#[derive(Clone)]
pub struct FederationContext {
    pub server_id: String,
    pub thread_registry: federation::ThreadRegistry,
    pub relay_sender: Arc<relay::RelaySender>,
    pub local_delivery_tx: broadcast::Sender<String>,
    pub remote_cache: crate::identity::RemoteServerCache,
}

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
    /// In-memory archival fact store keyed by fact key (fallback when no DB).
    pub archival_store: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    /// Optional federation context for cross-server routing.
    pub federation: Option<FederationContext>,
    /// Optional PostgreSQL pool for production persistence.
    pub db: Option<sqlx::PgPool>,
}

/// Start the Axum WebSocket server on a random available port (legacy mode).
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
        federation: None,
        db: None,
    };

    let app = Router::new()
        .route("/ws", get(ws_upgrade_handler))
        .route(
            // Demo: single agent -- production would look up by agent_name
            "/.well-known/ainm/{agent_name}/agent-card.json",
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

/// Start the Axum WebSocket server with federation support on a specified port.
/// Returns the bound address and the server task handle.
pub async fn start_federation(
    identity_store: IdentityStore,
    agent_card: serde_json::Value,
    federation: Option<FederationContext>,
    port: u16,
    db: Option<sqlx::PgPool>,
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
        federation,
        db,
    };

    let app = Router::new()
        .route("/ws", get(ws_upgrade_handler))
        .route(
            // Demo: single agent -- production would look up by agent_name
            "/.well-known/ainm/{agent_name}/agent-card.json",
            get(agent_card_handler),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
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

    // Track stream_id -> thread_id for federation relay of delta/end events
    let mut stream_thread_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();

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

                            // Federation-aware routing for agent responses
                            if let Some(ref fed) = state.federation {
                                let thread_id = params
                                    .get("thread_id")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("");
                                let origin = federation::parse_origin_domain(thread_id)
                                    .unwrap_or("");

                                if origin != fed.server_id && !origin.is_empty() {
                                    // Thread originated on remote server: relay response back
                                    fed.relay_sender.enqueue(origin, params.clone());
                                } else {
                                    // Local thread: deliver to human locally
                                    let deliver = protocol::new_notification(
                                        "ainm.message.deliver",
                                        serde_json::json!({
                                            "message": params,
                                            "verified": verified,
                                        }),
                                    );
                                    let deliver_text = serde_json::to_string(&deliver).unwrap();
                                    let _ = to_human_tx.send(deliver_text);
                                }
                            } else {
                                // Legacy mode
                                let deliver = protocol::new_notification(
                                    "ainm.message.deliver",
                                    serde_json::json!({
                                        "message": params,
                                        "verified": verified,
                                    }),
                                );
                                let deliver_text = serde_json::to_string(&deliver).unwrap();
                                let _ = to_human_tx.send(deliver_text);
                            }

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

                                // Store: PostgreSQL if available, else in-memory fallback
                                if let Some(ref db) = state.db {
                                    if let Err(e) = crate::repository::store_fact(
                                        db,
                                        &fact_id,
                                        &from_identity,
                                        Some(&thread_id),
                                        &key,
                                        &value,
                                        &tags,
                                        &from_identity,
                                    ).await {
                                        tracing::error!("[memory] PostgreSQL store failed: {e}, falling back to in-memory");
                                        archival_store.lock().await.insert(key.clone(), archival_fact.clone());
                                    }
                                } else {
                                    archival_store.lock().await.insert(key.clone(), archival_fact.clone());
                                }

                                // Send response
                                if has_id {
                                    let resp = protocol::new_response(
                                        parsed.get("id").cloned().unwrap(),
                                        archival_fact.clone(),
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

                                // After successful memory store in a federated thread,
                                // push context update to remote servers (ainm.context-push)
                                if let Some(ref fed) = state.federation {
                                    let origin = federation::parse_origin_domain(&thread_id).unwrap_or("");
                                    if origin == fed.server_id {
                                        // We are origin -- push the stored fact as a context frame to remotes
                                        let context_frame = serde_json::json!({
                                            "frame_type": "active_context",
                                            "thread_id": thread_id,
                                            "key": key,
                                            "data": archival_fact,
                                            "timestamp": protocol::now_rfc3339()
                                        });
                                        if let Some((_, remotes)) = fed.thread_registry.get_info(&thread_id) {
                                            for remote in &remotes {
                                                if remote != &fed.server_id {
                                                    let push_payload = serde_json::json!({
                                                        "jsonrpc": "2.0",
                                                        "method": "ainm.context-push",
                                                        "id": protocol::new_message_id(),
                                                        "params": {
                                                            "thread_id": thread_id,
                                                            "context_frames": [context_frame]
                                                        }
                                                    });
                                                    fed.relay_sender.enqueue(remote, push_payload);
                                                    tracing::info!("[federation] Pushed context frame to {}", remote);
                                                }
                                            }
                                        }
                                    }
                                }
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

                            // Query: PostgreSQL if available, else in-memory fallback
                            let results: Vec<serde_json::Value> = if let Some(ref db) = state.db {
                                match crate::repository::query_facts(
                                    db,
                                    &from_addr,
                                    if key_prefix.is_empty() { None } else { Some(&key_prefix) },
                                    limit as i64,
                                ).await {
                                    Ok(rows) => rows.iter().map(|r| r.to_protocol_json()).collect(),
                                    Err(e) => {
                                        tracing::error!("[memory] PostgreSQL query failed: {e}, falling back to in-memory");
                                        let store = archival_store.lock().await;
                                        store.iter()
                                            .filter(|(k, _)| key_prefix.is_empty() || k.starts_with(&key_prefix))
                                            .take(limit)
                                            .map(|(_, v)| v.clone())
                                            .collect()
                                    }
                                }
                            } else {
                                let store = archival_store.lock().await;
                                store.iter()
                                    .filter(|(k, _)| key_prefix.is_empty() || k.starts_with(&key_prefix))
                                    .take(limit)
                                    .map(|(_, v)| v.clone())
                                    .collect()
                            };

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
                            let archival_facts: Vec<serde_json::Value> = if let Some(ref db) = state.db {
                                match crate::repository::get_all_facts(db, &from_addr).await {
                                    Ok(rows) => rows.iter().map(|r| r.to_protocol_json()).collect(),
                                    Err(_) => {
                                        let store = archival_store.lock().await;
                                        store.values().cloned().collect()
                                    }
                                }
                            } else {
                                let store = archival_store.lock().await;
                                store.values().cloned().collect()
                            };
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
                                    // Evict: PostgreSQL if available, else in-memory fallback
                                    let evicted = if let Some(ref db) = state.db {
                                        match crate::repository::evict_fact(db, &from_addr, &key).await {
                                            Ok(deleted) => deleted,
                                            Err(e) => {
                                                tracing::error!("[memory] PostgreSQL evict failed: {e}, falling back to in-memory");
                                                archival_store.lock().await.remove(&key).is_some()
                                            }
                                        }
                                    } else {
                                        archival_store.lock().await.remove(&key).is_some()
                                    };

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
                    // ── Compound Memory: Graph Operations ──
                    "ainm.memory.graph.query" => {
                        if let (Some(ref db), Some(params)) = (&state.db, parsed.get("params")) {
                            let text_q = params.get("query").and_then(|q| q.as_str());
                            let min_conf = params.get("min_confidence").and_then(|c| c.as_f64()).unwrap_or(0.0) as f32;
                            let lim = params.get("limit").and_then(|l| l.as_i64()).unwrap_or(20);
                            let temporal = params.get("temporal_filter").and_then(|t| t.as_str()).unwrap_or("current");
                            let include_edges = params.get("include_edges").and_then(|e| e.as_bool()).unwrap_or(false);

                            match crate::graph::query_nodes(db, &from_addr, text_q, None, None, temporal, min_conf, lim).await {
                                Ok(nodes) => {
                                    let node_ids: Vec<uuid::Uuid> = nodes.iter().map(|n| n.id).collect();
                                    let edges = if include_edges && !node_ids.is_empty() {
                                        crate::graph::get_edges_for_nodes(db, &from_addr, &node_ids).await.unwrap_or_default()
                                    } else {
                                        vec![]
                                    };
                                    let total = nodes.len();
                                    let result = serde_json::json!({
                                        "nodes": nodes.iter().map(|n| n.to_protocol_json()).collect::<Vec<_>>(),
                                        "edges": edges.iter().map(|e| e.to_protocol_json()).collect::<Vec<_>>(),
                                        "total_count": total,
                                        "returned_count": total,
                                    });
                                    if has_id {
                                        let resp = protocol::new_response(parsed.get("id").cloned().unwrap(), result);
                                        let mut sender = ws_sender.lock().await;
                                        let _ = sender.send(Message::Text(serde_json::to_string(&resp).unwrap().into())).await;
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("[graph] query failed: {e}");
                                    if has_id {
                                        let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32050, &format!("Graph query failed: {e}"));
                                        let mut sender = ws_sender.lock().await;
                                        let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                                    }
                                }
                            }
                        } else if has_id {
                            let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32062, "Compound memory not supported");
                            let mut sender = ws_sender.lock().await;
                            let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                        }
                    }
                    "ainm.memory.graph.traverse" => {
                        if let (Some(ref db), Some(params)) = (&state.db, parsed.get("params")) {
                            let start_str = params.get("start_node").and_then(|s| s.as_str()).unwrap_or("");
                            let max_depth = params.get("max_depth").and_then(|d| d.as_i64()).unwrap_or(2) as i32;
                            let direction = params.get("direction").and_then(|d| d.as_str()).unwrap_or("both");

                            if let Ok(start_uuid) = uuid::Uuid::parse_str(start_str) {
                                match crate::graph::traverse(db, &from_addr, start_uuid, max_depth, direction, None).await {
                                    Ok((nodes, edges)) => {
                                        let result = serde_json::json!({
                                            "subgraph": {
                                                "nodes": nodes.iter().map(|n| n.to_protocol_json()).collect::<Vec<_>>(),
                                                "edges": edges.iter().map(|e| e.to_protocol_json()).collect::<Vec<_>>(),
                                            },
                                            "traversal_depth_reached": max_depth,
                                        });
                                        if has_id {
                                            let resp = protocol::new_response(parsed.get("id").cloned().unwrap(), result);
                                            let mut sender = ws_sender.lock().await;
                                            let _ = sender.send(Message::Text(serde_json::to_string(&resp).unwrap().into())).await;
                                        }
                                    }
                                    Err(e) => {
                                        if has_id {
                                            let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32050, &format!("Traversal failed: {e}"));
                                            let mut sender = ws_sender.lock().await;
                                            let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                                        }
                                    }
                                }
                            } else if has_id {
                                let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32066, "Invalid start_node UUID");
                                let mut sender = ws_sender.lock().await;
                                let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                            }
                        } else if has_id {
                            let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32062, "Compound memory not supported");
                            let mut sender = ws_sender.lock().await;
                            let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                        }
                    }
                    "ainm.memory.graph.contribute" => {
                        if let (Some(ref db), Some(params)) = (&state.db, parsed.get("params")) {
                            let thread_id = params.get("thread_id").and_then(|t| t.as_str()).unwrap_or("").to_string();
                            if let Some(node_data) = params.get("node") {
                                let node_type = node_data.get("type").and_then(|t| t.as_str()).unwrap_or("semantic");
                                let category = node_data.get("category").and_then(|c| c.as_str()).unwrap_or("fact");
                                let label = node_data.get("label").and_then(|l| l.as_str()).unwrap_or("");
                                let confidence = node_data.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.85) as f32;
                                // Cap agent contributions at 0.85
                                let confidence = confidence.min(0.85);
                                let properties = node_data.get("properties").cloned().unwrap_or(serde_json::json!({}));
                                let tags = node_data.get("tags").cloned().unwrap_or(serde_json::json!([]));
                                let provenance = serde_json::json!({
                                    "thread_id": thread_id,
                                    "contributed_by": from_addr,
                                    "extracted_at": protocol::now_rfc3339(),
                                });

                                // Determine user_addr: for agent contributions, we need the user in the thread
                                // For now, use from_addr as user_addr (the contributor)
                                // In production, this would resolve to the thread owner
                                let user_addr = &from_addr;

                                match crate::graph::insert_node(db, user_addr, node_type, category, label, &properties, confidence, &provenance, &tags).await {
                                    Ok(node_id) => {
                                        // Process optional edges
                                        let mut edge_ids = Vec::new();
                                        if let Some(edges) = params.get("edges").and_then(|e| e.as_array()) {
                                            for edge_data in edges {
                                                let rel = edge_data.get("relationship").and_then(|r| r.as_str()).unwrap_or("related_to");
                                                let edge_conf = edge_data.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.85) as f32;
                                                if let Some(target_id_str) = edge_data.get("target_node").and_then(|t| t.as_str()) {
                                                    if let Ok(target_uuid) = uuid::Uuid::parse_str(target_id_str) {
                                                        if let Ok(eid) = crate::graph::insert_edge(db, user_addr, node_id, target_uuid, rel, 1.0, edge_conf, &provenance, &serde_json::json!({})).await {
                                                            edge_ids.push(eid.to_string());
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        if has_id {
                                            let result = serde_json::json!({
                                                "contributed": true,
                                                "node_id": node_id.to_string(),
                                                "edge_ids": edge_ids,
                                            });
                                            let resp = protocol::new_response(parsed.get("id").cloned().unwrap(), result);
                                            let mut sender = ws_sender.lock().await;
                                            let _ = sender.send(Message::Text(serde_json::to_string(&resp).unwrap().into())).await;
                                        }
                                    }
                                    Err(e) => {
                                        if has_id {
                                            let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32067, &format!("Contribution rejected: {e}"));
                                            let mut sender = ws_sender.lock().await;
                                            let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                                        }
                                    }
                                }
                            }
                        } else if has_id {
                            let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32062, "Compound memory not supported");
                            let mut sender = ws_sender.lock().await;
                            let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                        }
                    }
                    "ainm.memory.lint" => {
                        if let Some(ref db) = state.db {
                            let contradictions = crate::graph::find_contradictions(db, &from_addr).await.unwrap_or_default();
                            let orphans = crate::graph::find_orphans(db, &from_addr).await.unwrap_or_default();
                            let stale = crate::graph::find_stale(db, &from_addr, 180).await.unwrap_or_default();
                            let (total_nodes, total_edges) = crate::graph::graph_stats(db, &from_addr).await.unwrap_or((0, 0));

                            let mut findings = Vec::new();
                            for (s, t, _) in &contradictions {
                                findings.push(serde_json::json!({
                                    "type": "contradiction",
                                    "severity": "warning",
                                    "description": "Two active nodes may assert conflicting facts",
                                    "node_ids": [s.to_string(), t.to_string()],
                                }));
                            }
                            for oid in &orphans {
                                findings.push(serde_json::json!({
                                    "type": "orphan",
                                    "severity": "info",
                                    "description": "Node has no edges connecting it to the rest of the graph",
                                    "node_ids": [oid.to_string()],
                                }));
                            }
                            for sid in &stale {
                                findings.push(serde_json::json!({
                                    "type": "stale",
                                    "severity": "info",
                                    "description": "Node has not been updated in over 6 months",
                                    "node_ids": [sid.to_string()],
                                }));
                            }

                            let result = serde_json::json!({
                                "lint_id": protocol::new_message_id(),
                                "timestamp": protocol::now_rfc3339(),
                                "findings": findings,
                                "summary": {
                                    "total_nodes": total_nodes,
                                    "total_edges": total_edges,
                                    "contradictions": contradictions.len(),
                                    "orphan_nodes": orphans.len(),
                                    "stale_nodes": stale.len(),
                                },
                            });
                            if has_id {
                                let resp = protocol::new_response(parsed.get("id").cloned().unwrap(), result);
                                let mut sender = ws_sender.lock().await;
                                let _ = sender.send(Message::Text(serde_json::to_string(&resp).unwrap().into())).await;
                            }
                        } else if has_id {
                            let err = protocol::new_error(parsed.get("id").cloned().unwrap(), -32062, "Compound memory not supported");
                            let mut sender = ws_sender.lock().await;
                            let _ = sender.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
                        }
                    }
                    "ainm.stream.start"
                    | "ainm.stream.delta"
                    | "ainm.stream.end"
                    | "ainm.presence.update" => {
                        // Always deliver locally
                        let _ = to_human_tx.send(text_str.clone());

                        // Federation relay if thread is federated (D-04: stream events via ainm.relay)
                        if let Some(ref fed) = state.federation {
                            let thread_id_opt = parsed.get("params")
                                .and_then(|p| p.get("thread_id"))
                                .and_then(|t| t.as_str());

                            let effective_thread_id = if let Some(tid) = thread_id_opt {
                                // StreamStart — track the stream_id -> thread_id mapping
                                if method == "ainm.stream.start" {
                                    if let Some(sid) = parsed.get("params").and_then(|p| p.get("stream_id")).and_then(|s| s.as_str()) {
                                        stream_thread_map.insert(sid.to_string(), tid.to_string());
                                    }
                                }
                                Some(tid.to_string())
                            } else {
                                // Delta/End — look up thread_id from stream_id
                                parsed.get("params")
                                    .and_then(|p| p.get("stream_id"))
                                    .and_then(|s| s.as_str())
                                    .and_then(|sid| stream_thread_map.get(sid).cloned())
                            };

                            if let Some(tid) = effective_thread_id {
                                let origin = federation::parse_origin_domain(&tid).unwrap_or("");
                                if !origin.is_empty() && origin != fed.server_id {
                                    // Relay to origin server (which will fan out)
                                    fed.relay_sender.enqueue(origin, parsed.clone());
                                } else if origin == fed.server_id {
                                    // We ARE origin — fan out to all participating remote servers
                                    if let Some((_, remotes)) = fed.thread_registry.get_info(&tid) {
                                        for remote in &remotes {
                                            if remote != &fed.server_id {
                                                fed.relay_sender.enqueue(remote, parsed.clone());
                                            }
                                        }
                                    }
                                }
                            }

                            // Clean up stream_thread_map on StreamEnd
                            if method == "ainm.stream.end" {
                                if let Some(sid) = parsed.get("params").and_then(|p| p.get("stream_id")).and_then(|s| s.as_str()) {
                                    stream_thread_map.remove(sid);
                                }
                            }
                        }
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
    let archival_store = state.archival_store.clone();
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
                        // In federation mode, use ThreadRegistry to create thread
                        // with origin domain embedded per D-12
                        let thread_id = if let Some(ref fed) = state.federation {
                            let tid = fed.thread_registry.create_thread(&fed.server_id);
                            // Pre-populate thread with all known remote servers
                            // so the first human message gets relayed immediately
                            for remote_id in fed.remote_cache.keys() {
                                if remote_id != fed.server_id {
                                    fed.thread_registry.add_server(&tid, &remote_id);
                                }
                            }
                            tid
                        } else {
                            parsed
                                .get("params")
                                .and_then(|p| p.get("thread_id"))
                                .and_then(|t| t.as_str())
                                .unwrap_or("")
                                .to_string()
                        };
                        threads.lock().await.push(thread_id.clone());

                        // After thread creation with federation, push existing context
                        // frames to remote servers so they start with knowledge
                        if let Some(ref fed) = state.federation {
                            let store = archival_store.lock().await;
                            if !store.is_empty() {
                                let context_frames: Vec<serde_json::Value> = store
                                    .iter()
                                    .map(|(key, value)| {
                                        serde_json::json!({
                                            "frame_type": "active_context",
                                            "thread_id": thread_id,
                                            "key": key,
                                            "data": value,
                                            "timestamp": protocol::now_rfc3339()
                                        })
                                    })
                                    .collect();

                                // Push to all remote participating servers
                                if let Some((_, remotes)) = fed.thread_registry.get_info(&thread_id) {
                                    for remote in &remotes {
                                        if remote != &fed.server_id {
                                            let push_payload = serde_json::json!({
                                                "jsonrpc": "2.0",
                                                "method": "ainm.context-push",
                                                "id": protocol::new_message_id(),
                                                "params": {
                                                    "thread_id": thread_id,
                                                    "context_frames": context_frames
                                                }
                                            });
                                            fed.relay_sender.enqueue(remote, push_payload);
                                            tracing::info!(
                                                "[federation] Pushed {} context frames to {}",
                                                context_frames.len(),
                                                remote
                                            );
                                        }
                                    }
                                }
                            }
                        }

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

                            // Federation-aware routing
                            if let Some(ref fed) = state.federation {
                                let thread_id = params
                                    .get("thread_id")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("");
                                let origin = federation::parse_origin_domain(thread_id)
                                    .unwrap_or("");

                                if origin == fed.server_id || origin.is_empty() {
                                    // We are origin: deliver locally AND relay to remote servers
                                    let deliver = protocol::new_notification(
                                        "ainm.message.deliver",
                                        serde_json::json!({
                                            "message": params,
                                            "verified": verified,
                                        }),
                                    );
                                    let deliver_text = serde_json::to_string(&deliver).unwrap();
                                    // Deliver locally (to agent if connected via WS)
                                    let _ = to_agent_tx.send(deliver_text.clone());
                                    // Also deliver via federation local_delivery_tx
                                    let _ = fed.local_delivery_tx.send(deliver_text);

                                    // Relay to all remote participating servers
                                    if let Some((_, servers)) = fed.thread_registry.get_info(thread_id) {
                                        for server in servers {
                                            fed.relay_sender.enqueue(&server, params.clone());
                                        }
                                    }

                                    // Relay to known remotes not yet in the thread registry
                                    let remote_domains: Vec<String> = {
                                        let registered: std::collections::HashSet<String> = fed
                                            .thread_registry
                                            .get_info(thread_id)
                                            .map(|(_, servers)| servers)
                                            .unwrap_or_default();
                                        fed.remote_cache
                                            .keys()
                                            .into_iter()
                                            .filter(|id| id != &fed.server_id && !registered.contains(id))
                                            .collect()
                                    };
                                    for domain in remote_domains {
                                        fed.thread_registry.add_server(thread_id, &domain);
                                        fed.relay_sender.enqueue(&domain, params.clone());
                                    }
                                } else {
                                    // Thread originated on remote server: relay to origin
                                    fed.relay_sender.enqueue(origin, params.clone());
                                }
                            } else {
                                // Legacy mode: deliver locally
                                let deliver = protocol::new_notification(
                                    "ainm.message.deliver",
                                    serde_json::json!({
                                        "message": params,
                                        "verified": verified,
                                    }),
                                );
                                let deliver_text = serde_json::to_string(&deliver).unwrap();
                                let _ = to_agent_tx.send(deliver_text);
                            }

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
