use crate::identity::IdentityStore;
use crate::protocol;
use crate::signing;
use anyhow::Result;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
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
    };

    let app = Router::new()
        .route("/ws", get(ws_upgrade_handler))
        .route(
            "/.well-known/ainm/agent-card/helper",
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

/// Agent Card HTTP endpoint.
async fn agent_card_handler(State(state): State<ServerState>) -> Json<serde_json::Value> {
    Json(state.agent_card.clone())
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
            "protocol_version": "0.1.0",
            "server_identity": "server@demo.ainm.local",
            "capabilities": {
                "streaming": true,
                "interactive": true,
                "tool_visibility": true
            }
        }),
    );
    {
        let resp_text = serde_json::to_string(&init_response).unwrap();
        let mut sender = ws_sender.lock().await;
        if sender.send(Message::Text(resp_text.into())).await.is_err() {
            return;
        }
    }

    tracing::info!("Client connected: {}", from_addr);

    // Determine if this is the agent or the human based on address
    let is_agent = from_addr.starts_with("helper");

    if is_agent {
        handle_agent_connection(ws_sender, ws_receiver, state).await;
    } else {
        handle_human_connection(ws_sender, ws_receiver, state).await;
    }
}

/// Handle the agent's WebSocket connection.
async fn handle_agent_connection(
    ws_sender: Arc<Mutex<futures_util::stream::SplitSink<WebSocket, Message>>>,
    mut ws_receiver: futures_util::stream::SplitStream<WebSocket>,
    state: ServerState,
) {
    let mut from_human_rx = state.to_agent_tx.subscribe();
    let to_human_tx = state.to_human_tx.clone();
    let identity_store = state.identity_store.clone();
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
                    "ainm.stream.start"
                    | "ainm.stream.delta"
                    | "ainm.stream.end"
                    | "ainm.tool.call.started"
                    | "ainm.tool.call.progress"
                    | "ainm.tool.call.completed" => {
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
