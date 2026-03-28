use crate::identity::{Identity, IdentityStore};
use crate::protocol;
use crate::signing;
use crate::streaming;
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio_tungstenite::tungstenite::Message;

/// The concrete WebSocket sender type used throughout the agent.
type WsSender = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    Message,
>;

/// Run the mock agent: connect to server, respond to messages with scripted scenarios.
/// Per D-03: fully scripted, no API key, works offline.
pub async fn run(
    server_addr: SocketAddr,
    identity: Identity,
    _identity_store: IdentityStore,
) -> Result<()> {
    let url = format!("ws://{}/ws", server_addr);
    let (ws_stream, _) = tokio_tungstenite::connect_async(&url).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Perform ainm.initialize handshake
    let init_req = protocol::new_request(
        "ainm.initialize",
        serde_json::json!({
            "identity": identity.address,
            "protocol_version": "0.1.0",
            "capabilities": {
                "streaming": true,
                "interactive": true,
                "tool_visibility": true
            }
        }),
        serde_json::json!(1),
    );
    let init_text = serde_json::to_string(&init_req)?;
    ws_sender.send(Message::Text(init_text.into())).await?;

    // Wait for initialized response
    if let Some(Ok(Message::Text(_))) = ws_receiver.next().await {
        tracing::info!("Agent connected and initialized");
    }

    // Listen for incoming messages from the server
    let mut first_message = true;
    while let Some(Ok(msg)) = ws_receiver.next().await {
        if let Message::Text(text) = msg {
            let text_str = text.to_string();
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text_str) {
                let method = parsed
                    .get("method")
                    .and_then(|m| m.as_str())
                    .unwrap_or("");

                if method == "ainm.message.deliver" {
                    if let Some(message) = parsed.get("params").and_then(|p| p.get("message")) {
                        let user_text = extract_text(message);
                        let thread_id = message
                            .get("thread_id")
                            .and_then(|t| t.as_str())
                            .unwrap_or("")
                            .to_string();

                        let lower = user_text.to_lowercase();

                        if lower.contains("hello") || first_message {
                            first_message = false;
                            handle_hello(&mut ws_sender, &identity, &thread_id).await?;
                        } else if lower.contains("form") {
                            handle_form(&mut ws_sender, &identity, &thread_id).await?;
                        } else if lower.contains("approve") || lower.contains("approval") {
                            handle_approval(&mut ws_sender, &identity, &thread_id).await?;
                        } else if lower.contains("tool") {
                            handle_tool(&mut ws_sender, &identity, &thread_id).await?;
                        } else if lower.contains("button") {
                            handle_button(&mut ws_sender, &identity, &thread_id).await?;
                        } else {
                            first_message = false;
                            handle_default(&mut ws_sender, &identity, &thread_id, &user_text)
                                .await?;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Extract the text content from a message envelope.
fn extract_text(message: &serde_json::Value) -> String {
    message
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|blocks| {
            blocks.iter().find_map(|b| {
                if b.get("type").and_then(|t| t.as_str()) == Some("text") {
                    b.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                } else {
                    None
                }
            })
        })
        .unwrap_or_default()
}

/// Scenario 1: Streaming text response for "hello" or first message.
async fn handle_hello(
    ws_sender: &mut WsSender,
    identity: &Identity,
    thread_id: &str,
) -> Result<()> {
    let text = "Welcome to AINM! I'm helper, a protocol demo agent. \
        I can demonstrate streaming, forms, buttons, approvals, and tool calls. \
        Try typing: 'form', 'approve', 'tool', 'button', or just chat with me.";
    streaming::send_streamed_response(ws_sender, identity, thread_id, text).await
}

/// Scenario 2: FormBlock demo.
async fn handle_form(
    ws_sender: &mut WsSender,
    identity: &Identity,
    thread_id: &str,
) -> Result<()> {
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
                "source": "agent",
                "text": "Here's a FormBlock demo -- the protocol supports structured data input:",
                "fallback": "Here's a FormBlock demo -- the protocol supports structured data input:"
            },
            {
                "type": "form",
                "version": "1.0",
                "source": "agent",
                "action_id": "demo-form",
                "title": "Protocol Feedback",
                "submit_label": "Submit Feedback",
                "fields": [
                    {
                        "name": "name",
                        "label": "Your Name",
                        "field_type": "text",
                        "required": true
                    },
                    {
                        "name": "rating",
                        "label": "Protocol Rating",
                        "field_type": "select",
                        "options": [
                            {"value": "5", "label": "Excellent"},
                            {"value": "4", "label": "Good"},
                            {"value": "3", "label": "Average"}
                        ]
                    },
                    {
                        "name": "comments",
                        "label": "Comments",
                        "field_type": "textarea"
                    }
                ],
                "fallback": "[Form: Protocol Feedback - 3 fields]"
            }
        ]
    });

    signing::sign_envelope(&identity.key_pair, &mut envelope)?;
    send_message(ws_sender, &envelope).await
}

/// Scenario 3: ApprovalBlock demo.
async fn handle_approval(
    ws_sender: &mut WsSender,
    identity: &Identity,
    thread_id: &str,
) -> Result<()> {
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
                "source": "agent",
                "text": "Here's an ApprovalBlock -- agents can request human approval before taking action:",
                "fallback": "Here's an ApprovalBlock -- agents can request human approval before taking action:"
            },
            {
                "type": "approval",
                "version": "1.0",
                "source": "agent",
                "action_id": "demo-approval",
                "prompt": "Deploy v0.1.0 to production?",
                "options": [
                    {"option_id": "approve", "label": "Approve", "style": "primary"},
                    {"option_id": "reject", "label": "Reject", "style": "danger"}
                ],
                "status": "pending",
                "fallback": "[Approval: Deploy v0.1.0 to production?]"
            }
        ]
    });

    signing::sign_envelope(&identity.key_pair, &mut envelope)?;
    send_message(ws_sender, &envelope).await
}

/// Scenario 4: Tool visibility demo.
async fn handle_tool(
    ws_sender: &mut WsSender,
    identity: &Identity,
    thread_id: &str,
) -> Result<()> {
    let call_id = format!("tc-{}", protocol::new_message_id());

    // ToolCallStarted
    let mut started = serde_json::json!({
        "id": protocol::new_message_id(),
        "from": identity.address,
        "thread_id": thread_id,
        "timestamp": protocol::now_rfc3339(),
        "type": "tool.call.started",
        "content": [
            {
                "type": "text",
                "version": "1.0",
                "source": "agent",
                "text": "Fetching current weather data...",
                "fallback": "Fetching current weather data..."
            }
        ],
        "tool": {
            "call_id": call_id,
            "name": "weather_lookup",
            "description": "Fetching current weather data",
            "input": { "city": "San Francisco" }
        }
    });
    signing::sign_envelope(&identity.key_pair, &mut started)?;
    send_message(ws_sender, &started).await?;

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // ToolCallProgress
    let mut progress = serde_json::json!({
        "id": protocol::new_message_id(),
        "from": identity.address,
        "thread_id": thread_id,
        "timestamp": protocol::now_rfc3339(),
        "type": "tool.call.progress",
        "content": [
            {
                "type": "text",
                "version": "1.0",
                "source": "agent",
                "text": "Querying weather API...",
                "fallback": "Querying weather API..."
            }
        ],
        "tool": {
            "call_id": call_id,
            "progress_text": "Querying weather API...",
            "percent_complete": 50
        }
    });
    signing::sign_envelope(&identity.key_pair, &mut progress)?;
    send_message(ws_sender, &progress).await?;

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // ToolCallCompleted
    let mut completed = serde_json::json!({
        "id": protocol::new_message_id(),
        "from": identity.address,
        "thread_id": thread_id,
        "timestamp": protocol::now_rfc3339(),
        "type": "tool.call.completed",
        "content": [
            {
                "type": "text",
                "version": "1.0",
                "source": "agent",
                "text": "Weather lookup complete.",
                "fallback": "Weather lookup complete."
            }
        ],
        "tool": {
            "call_id": call_id,
            "status": "success",
            "result_summary": "San Francisco: 18C, partly cloudy",
            "duration_ms": 1023
        }
    });
    signing::sign_envelope(&identity.key_pair, &mut completed)?;
    send_message(ws_sender, &completed).await?;

    // Send follow-up message with results
    let mut result_msg = serde_json::json!({
        "id": protocol::new_message_id(),
        "from": identity.address,
        "thread_id": thread_id,
        "timestamp": protocol::now_rfc3339(),
        "type": "message",
        "content": [
            {
                "type": "text",
                "version": "1.0",
                "source": "agent",
                "text": "The weather in San Francisco is 18C and partly cloudy. (This was a tool call demo -- the tool events above show the ToolCallStarted -> Progress -> Completed lifecycle.)",
                "fallback": "The weather in San Francisco is 18C and partly cloudy."
            }
        ]
    });
    signing::sign_envelope(&identity.key_pair, &mut result_msg)?;
    send_message(ws_sender, &result_msg).await
}

/// Scenario 5: ButtonBlock demo.
async fn handle_button(
    ws_sender: &mut WsSender,
    identity: &Identity,
    thread_id: &str,
) -> Result<()> {
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
                "source": "agent",
                "text": "Here are ButtonBlocks -- the three semantic styles (primary, secondary, danger):",
                "fallback": "Here are ButtonBlocks:"
            },
            {
                "type": "button",
                "version": "1.0",
                "source": "agent",
                "label": "Primary Action",
                "action_id": "btn-1",
                "style": "primary",
                "fallback": "[Button: Primary Action]"
            },
            {
                "type": "button",
                "version": "1.0",
                "source": "agent",
                "label": "Secondary",
                "action_id": "btn-2",
                "style": "secondary",
                "fallback": "[Button: Secondary]"
            },
            {
                "type": "button",
                "version": "1.0",
                "source": "agent",
                "label": "Delete (Danger)",
                "action_id": "btn-3",
                "style": "danger",
                "fallback": "[Button: Delete (Danger)]"
            }
        ]
    });

    signing::sign_envelope(&identity.key_pair, &mut envelope)?;
    send_message(ws_sender, &envelope).await
}

/// Default scenario: echo back with streaming.
async fn handle_default(
    ws_sender: &mut WsSender,
    identity: &Identity,
    thread_id: &str,
    user_text: &str,
) -> Result<()> {
    let text = format!(
        "You said: '{}'. The protocol signed, delivered, and verified this message end-to-end.",
        user_text
    );
    streaming::send_streamed_response(ws_sender, identity, thread_id, &text).await
}

/// Helper: send a signed message envelope as ainm.message.send request.
async fn send_message(ws_sender: &mut WsSender, envelope: &serde_json::Value) -> Result<()> {
    let req = protocol::new_request(
        "ainm.message.send",
        envelope.clone(),
        serde_json::json!(protocol::new_message_id()),
    );
    let text = serde_json::to_string(&req)?;
    ws_sender
        .send(Message::Text(text.into()))
        .await
        .map_err(|e| anyhow::anyhow!("send message: {}", e))?;
    Ok(())
}
