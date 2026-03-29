use crate::identity::{Identity, IdentityStore};
use crate::protocol;
use crate::render;
use crate::signing;
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio_tungstenite::tungstenite::Message;

/// Run the terminal REPL for human input.
pub async fn run(
    server_addr: SocketAddr,
    identity: Identity,
    identity_store: IdentityStore,
) -> Result<()> {
    let url = format!("ws://{}/ws", server_addr);
    let (ws_stream, _) = tokio_tungstenite::connect_async(&url).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Perform ainm.initialize handshake with Ed25519 signature
    let init_signature = signing::sign_init(&identity.key_pair, &identity.address)?;
    let init_req = protocol::new_request(
        "ainm.initialize",
        serde_json::json!({
            "identity": identity.address,
            "protocolVersion": "0.1.0",
            "signature": init_signature,
            "capabilities": {
                "streaming": true,
                "contentTypes": ["text", "code", "button", "form", "table", "approval", "action"],
                "toolVisibility": true
            }
        }),
        serde_json::json!(2),
    );
    let init_text = serde_json::to_string(&init_req)?;
    ws_sender.send(Message::Text(init_text.into())).await?;

    // Wait for initialized response
    if let Some(Ok(Message::Text(_))) = ws_receiver.next().await {
        tracing::info!("REPL connected and initialized");
    }

    // Send ainm.initialized confirmation notification (transport.md Section 4.3)
    let initialized_notif = protocol::new_notification("ainm.initialized", serde_json::json!({}));
    let initialized_text = serde_json::to_string(&initialized_notif)?;
    ws_sender.send(Message::Text(initialized_text.into())).await?;

    // Create a thread
    let thread_id = protocol::new_message_id();
    let create_req = protocol::new_request(
        "ainm.thread.create",
        serde_json::json!({
            "thread_id": thread_id,
            "title": "Demo Conversation"
        }),
        serde_json::json!(3),
    );
    let create_text = serde_json::to_string(&create_req)?;
    ws_sender.send(Message::Text(create_text.into())).await?;

    // Wait for thread creation response and extract server-assigned federation thread_id
    let thread_id = if let Some(Ok(Message::Text(resp_text))) = ws_receiver.next().await {
        let resp_str = resp_text.to_string();
        if let Ok(resp) = serde_json::from_str::<serde_json::Value>(&resp_str) {
            resp.get("result")
                .and_then(|r| r.get("thread_id"))
                .and_then(|t| t.as_str())
                .map(|t| t.to_string())
                .unwrap_or_else(|| {
                    tracing::warn!("Thread create response missing thread_id, using local ID");
                    thread_id
                })
        } else {
            tracing::warn!("Failed to parse thread create response");
            thread_id
        }
    } else {
        tracing::warn!("No thread create response received");
        thread_id
    };
    tracing::info!("Thread created: {}", thread_id);

    // Split: receiver task handles incoming messages, sender is used by REPL
    let ws_sender = std::sync::Arc::new(tokio::sync::Mutex::new(ws_sender));

    // Spawn task to listen for incoming agent messages and render them
    let id_store = identity_store.clone();
    let receiver_handle = tokio::spawn(async move {
        handle_incoming(ws_receiver, id_store).await;
    });

    // Run rustyline REPL on a blocking thread
    let ws_sender_clone = ws_sender.clone();
    let identity_clone = identity.clone();
    let thread_id_clone = thread_id.clone();
    let server_addr_clone = server_addr;

    let repl_result = tokio::task::spawn_blocking(move || {
        run_repl_loop(
            ws_sender_clone,
            identity_clone,
            &thread_id_clone,
            server_addr_clone,
        )
    })
    .await?;

    receiver_handle.abort();
    repl_result
}

/// Handle incoming WebSocket messages from server (agent responses, streams, tool events).
async fn handle_incoming(
    mut ws_receiver: futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    identity_store: IdentityStore,
) {
    while let Some(Ok(msg)) = ws_receiver.next().await {
        if let Message::Text(text) = msg {
            let text_str = text.to_string();
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text_str) {
                let method = parsed
                    .get("method")
                    .and_then(|m| m.as_str())
                    .unwrap_or("");

                match method {
                    "ainm.message.deliver" => {
                        if let Some(params) = parsed.get("params") {
                            let verified = params
                                .get("verified")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            if let Some(message) = params.get("message") {
                                let msg_type = message
                                    .get("type")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("");

                                match msg_type {
                                    "tool.call.started" => {
                                        render::render_tool_event("started", message);
                                    }
                                    "tool.call.progress" => {
                                        render::render_tool_event("progress", message);
                                    }
                                    "tool.call.completed" => {
                                        render::render_tool_event("completed", message);
                                    }
                                    _ => {
                                        render::render_message(message, verified);
                                    }
                                }
                            }
                        }
                    }
                    "ainm.stream.start" => {
                        if let Some(params) = parsed.get("params") {
                            let stream_id = params
                                .get("stream_id")
                                .and_then(|s| s.as_str())
                                .unwrap_or("");
                            let from = params
                                .get("from")
                                .and_then(|f| f.as_str())
                                .unwrap_or("agent");
                            println!(
                                "\n  [{}] {}",
                                colored::Colorize::cyan(from),
                                colored::Colorize::dimmed("[streaming...]")
                            );
                            render::render_stream_start(stream_id);
                        }
                    }
                    "ainm.stream.delta" => {
                        if let Some(params) = parsed.get("params") {
                            let delta = params
                                .get("delta")
                                .and_then(|d| d.as_str())
                                .unwrap_or("");
                            render::render_stream_delta(delta);
                        }
                    }
                    "ainm.stream.end" => {
                        if let Some(params) = parsed.get("params") {
                            // Verify the collapsed message signature
                            let verified = if let Some(collapsed) =
                                params.get("collapsed_message")
                            {
                                let from = collapsed
                                    .get("from")
                                    .and_then(|f| f.as_str())
                                    .unwrap_or("");
                                if let Some(pk) = identity_store.get_public_key_bytes(from) {
                                    signing::verify_signature(&pk, collapsed).unwrap_or(false)
                                } else {
                                    false
                                }
                            } else {
                                false
                            };
                            render::render_stream_end(verified);
                        }
                    }
                    "ainm.memory.stored" => {
                        if let Some(params) = parsed.get("params") {
                            render::render_memory_event("stored", params);
                        }
                    }
                    "ainm.memory.query.result" => {
                        if let Some(params) = parsed.get("params") {
                            render::render_memory_event("query_result", params);
                        }
                    }
                    "ainm.context.frame" => {
                        if let Some(params) = parsed.get("params") {
                            render::render_memory_event("context_frame", params);
                        }
                    }
                    "ainm.memory.evicted" => {
                        if let Some(params) = parsed.get("params") {
                            render::render_memory_event("evicted", params);
                        }
                    }
                    "ainm.presence.update" => {
                        if let Some(params) = parsed.get("params") {
                            let detail = params
                                .get("detail")
                                .and_then(|d| d.as_str())
                                .unwrap_or("");
                            let status = params
                                .get("status")
                                .and_then(|s| s.as_str())
                                .unwrap_or("");
                            if status == "processing" {
                                render::render_bridge_event("a2a_working", params);
                            } else {
                                println!("  Presence: {} - {}", status, detail);
                            }
                        }
                    }
                    _ => {
                        // Ignore responses and other messages
                    }
                }
            }
        }
    }
}

/// Run the rustyline REPL loop on a blocking thread.
fn run_repl_loop(
    ws_sender: std::sync::Arc<tokio::sync::Mutex<futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >>>,
    identity: Identity,
    thread_id: &str,
    server_addr: SocketAddr,
) -> Result<()> {
    let mut rl = rustyline::DefaultEditor::new()?;

    loop {
        match rl.readline("alice> ") {
            Ok(line) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }

                rl.add_history_entry(&line).ok();

                match line.as_str() {
                    "/quit" | "/exit" => {
                        break;
                    }
                    "/help" => {
                        println!();
                        println!("  Commands:");
                        println!("    hello    - Streaming text response");
                        println!("    form     - FormBlock demo");
                        println!("    approve  - ApprovalBlock demo");
                        println!("    tool     - Tool visibility demo");
                        println!("    button   - ButtonBlock demo");
                        println!("    /card    - View Agent Card");
                        println!("    /discover agent@domain - Discover remote agent capabilities");
                        println!("    /memory  - Demo memory store, query, and context frame operations");
                        println!("    /evict   - Demo memory eviction (run /memory first)");
                        println!("    /bridge  - Demo MCP and A2A bridge compatibility mappings");
                        println!("    /help    - Show this help");
                        println!("    /quit    - Exit demo");
                        println!("    (any text) - Echo with streaming");
                        println!();
                    }
                    "/card" => {
                        // Fetch Agent Card via HTTP
                        let url = format!(
                            "http://{}/.well-known/ainm/helper/agent-card.json",
                            server_addr
                        );
                        let rt = tokio::runtime::Handle::current();
                        match rt.block_on(fetch_agent_card(&url)) {
                            Ok(card) => render::render_agent_card(&card),
                            Err(e) => println!("  Failed to fetch Agent Card: {}", e),
                        }
                    }
                    "/memory" => {
                        println!("  Starting memory operations demo...");
                        // Send /memory as a message to trigger agent handler
                        let mut envelope = serde_json::json!({
                            "id": protocol::new_message_id(),
                            "from": identity.address,
                            "thread_id": thread_id,
                            "timestamp": protocol::now_rfc3339(),
                            "type": "message",
                            "content": [
                                {
                                    "type": "text",
                                    "version": "1.0",
                                    "source": "user",
                                    "text": "/memory",
                                    "fallback": "/memory"
                                }
                            ]
                        });

                        if let Err(e) = signing::sign_envelope(&identity.key_pair, &mut envelope) {
                            println!("  Signing error: {}", e);
                            continue;
                        }

                        let req = protocol::new_request(
                            "ainm.message.send",
                            envelope,
                            serde_json::json!(protocol::new_message_id()),
                        );
                        let text = serde_json::to_string(&req).unwrap();

                        let rt = tokio::runtime::Handle::current();
                        let sender = ws_sender.clone();
                        if let Err(e) = rt.block_on(async {
                            let mut s = sender.lock().await;
                            s.send(Message::Text(text.into())).await
                        }) {
                            println!("  Send error: {}", e);
                        }
                    }
                    "/bridge" => {
                        println!("  Starting bridge compatibility demo...");
                        // Send /bridge as a message to trigger agent handler
                        let mut envelope = serde_json::json!({
                            "id": protocol::new_message_id(),
                            "from": identity.address,
                            "thread_id": thread_id,
                            "timestamp": protocol::now_rfc3339(),
                            "type": "message",
                            "content": [
                                {
                                    "type": "text",
                                    "version": "1.0",
                                    "source": "user",
                                    "text": "/bridge",
                                    "fallback": "/bridge"
                                }
                            ]
                        });

                        if let Err(e) = signing::sign_envelope(&identity.key_pair, &mut envelope) {
                            println!("  Signing error: {}", e);
                            continue;
                        }

                        let req = protocol::new_request(
                            "ainm.message.send",
                            envelope,
                            serde_json::json!(protocol::new_message_id()),
                        );
                        let text = serde_json::to_string(&req).unwrap();

                        let rt = tokio::runtime::Handle::current();
                        let sender = ws_sender.clone();
                        if let Err(e) = rt.block_on(async {
                            let mut s = sender.lock().await;
                            s.send(Message::Text(text.into())).await
                        }) {
                            println!("  Send error: {}", e);
                        }
                    }
                    "/evict" => {
                        println!("  Starting memory evict demo...");
                        let mut envelope = serde_json::json!({
                            "id": protocol::new_message_id(),
                            "from": identity.address,
                            "thread_id": thread_id,
                            "timestamp": protocol::now_rfc3339(),
                            "type": "message",
                            "content": [
                                {
                                    "type": "text",
                                    "version": "1.0",
                                    "source": "user",
                                    "text": "/evict",
                                    "fallback": "/evict"
                                }
                            ]
                        });

                        if let Err(e) = signing::sign_envelope(&identity.key_pair, &mut envelope) {
                            println!("  Signing error: {}", e);
                            continue;
                        }

                        let req = protocol::new_request(
                            "ainm.message.send",
                            envelope,
                            serde_json::json!(protocol::new_message_id()),
                        );
                        let text = serde_json::to_string(&req).unwrap();

                        let rt = tokio::runtime::Handle::current();
                        let sender = ws_sender.clone();
                        if let Err(e) = rt.block_on(async {
                            let mut s = sender.lock().await;
                            s.send(Message::Text(text.into())).await
                        }) {
                            println!("  Send error: {}", e);
                        }
                    }
                    line if line.starts_with("/discover ") => {
                        let target = line.trim_start_matches("/discover ").trim();
                        if target.is_empty() {
                            println!("  Usage: /discover agent@server-domain");
                            continue;
                        }
                        let parts: Vec<&str> = target.splitn(2, '@').collect();
                        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
                            println!("  Invalid format. Use: /discover agent@server-domain");
                            continue;
                        }
                        let agent_name = parts[0];
                        let domain = parts[1];

                        let url = format!(
                            "https://{}/.well-known/ainm/{}/agent-card.json",
                            domain, agent_name
                        );
                        println!("  Discovering {}...", target);
                        let rt = tokio::runtime::Handle::current();
                        match rt.block_on(fetch_remote_agent_card(&url)) {
                            Ok(card) => {
                                println!("  Agent Card resolved for {}:", target);
                                render::render_agent_card(&card);
                            }
                            Err(e) => println!("  Failed to discover {}: {}", target, e),
                        }
                    }
                    _ => {
                        // Build and sign message envelope
                        let mut envelope = serde_json::json!({
                            "id": protocol::new_message_id(),
                            "from": identity.address,
                            "thread_id": thread_id,
                            "timestamp": protocol::now_rfc3339(),
                            "type": "message",
                            "content": [
                                {
                                    "type": "text",
                                    "version": "1.0",
                                    "source": "user",
                                    "text": line,
                                    "fallback": line
                                }
                            ]
                        });

                        if let Err(e) = signing::sign_envelope(&identity.key_pair, &mut envelope) {
                            println!("  Signing error: {}", e);
                            continue;
                        }

                        // Send as JSON-RPC request
                        let req = protocol::new_request(
                            "ainm.message.send",
                            envelope,
                            serde_json::json!(protocol::new_message_id()),
                        );
                        let text = serde_json::to_string(&req).unwrap();

                        let rt = tokio::runtime::Handle::current();
                        let sender = ws_sender.clone();
                        if let Err(e) = rt.block_on(async {
                            let mut s = sender.lock().await;
                            s.send(Message::Text(text.into())).await
                        }) {
                            println!("  Send error: {}", e);
                        }
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("  Ctrl-C: use /quit to exit");
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                break;
            }
            Err(e) => {
                println!("  REPL error: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Fetch a remote agent card via HTTPS, accepting self-signed certs for the demo.
async fn fetch_remote_agent_card(url: &str) -> Result<serde_json::Value> {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?;
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }
    let card: serde_json::Value = resp.json().await?;
    Ok(card)
}

/// Fetch Agent Card via HTTP GET.
async fn fetch_agent_card(url: &str) -> Result<serde_json::Value> {
    // Use a simple TCP connection + HTTP request since we don't have reqwest
    let addr: SocketAddr = url
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap()
        .parse()?;

    let path = url
        .trim_start_matches("http://")
        .find('/')
        .map(|i| &url.trim_start_matches("http://")[i..])
        .unwrap_or("/");

    let stream = tokio::net::TcpStream::connect(addr).await?;
    let (mut reader, mut writer) = stream.into_split();

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
        path, addr
    );
    writer.write_all(request.as_bytes()).await?;

    let mut response = Vec::new();
    reader.read_to_end(&mut response).await?;

    let response_str = String::from_utf8_lossy(&response);
    // Find the JSON body after headers
    if let Some(body_start) = response_str.find("\r\n\r\n") {
        let body = &response_str[body_start + 4..];
        let card: serde_json::Value = serde_json::from_str(body)?;
        Ok(card)
    } else {
        anyhow::bail!("Invalid HTTP response")
    }
}
