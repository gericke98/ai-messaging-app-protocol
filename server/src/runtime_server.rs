//! Server-coupled glue for the hosted-agent runtime loop (B-8).
//!
//! Lives in the *binary* crate only (it references `crate::server::ServerState`, which the
//! library crate doesn't expose). The pure, unit-testable loop is in `runtime.rs`; this
//! module wires that loop into the delivery path: it resolves consent scopes, assembles the
//! per-requester tool registry, loads thread history, runs the loop, and streams the reply.

use crate::llm::{ChatMessage, ToolSchema};
use crate::runtime::run_loop;
use crate::server::ServerState;

/// Server entry point: invoked from the delivery path when the hosted agent is a
/// participant in `thread_id` and the inbound message is addressed to/mentions it.
/// `requester` is the human whose consent scopes gate everything.
pub async fn handle_turn(
    state: &ServerState,
    thread_id: &str,
    requester: &str,
    inbound_user_text: &str,
) {
    let (Some(agent), Some(llm), Some(db)) =
        (state.hosted_agent.as_ref(), state.llm_client.as_ref(), state.db.as_ref())
    else {
        return; // hosted agent not provisioned → nothing to do
    };
    if !agent.enabled {
        return; // agent disabled
    }

    // Resolve the requester's granted scopes (consent gate spine).
    let (scopes, _level) = match crate::briefing::resolve_scopes(
        db, requester, &agent.address, crate::briefing::ConsentLevel::Standard,
    ).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("[runtime] resolve_scopes failed: {e}");
            (vec![], crate::briefing::ConsentLevel::Minimal)
        }
    };

    // Build the per-requester tool registry: memory tools + curated MCP providers.
    let mem = crate::tools_memory::MemoryToolProvider {
        db: db.clone(),
        user_addr: requester.to_string(),
        agent_addr: agent.address.clone(),
        thread_id: thread_id.to_string(),
        default_consent: crate::briefing::ConsentLevel::Standard,
    };
    let mut providers: Vec<Box<dyn crate::tools::ToolProvider>> = vec![Box::new(mem)];
    for p in state.mcp_providers.iter() {
        providers.push(Box::new(SharedProvider(p.clone())));
    }
    let registry = crate::tools::ToolRegistry::new(providers);

    // Recent thread history as chat messages (oldest→newest).
    let history = fetch_history_as_chat(db, thread_id, &agent.address).await;

    let outcome = run_loop(
        llm.as_ref(), &registry, &agent.system_prompt, &scopes, history,
        inbound_user_text, 8,
    ).await;

    stream_reply(state, agent, thread_id, &outcome.final_text).await;
}

/// Wrap an Arc<dyn ToolProvider> so it can join the Box<dyn> registry.
struct SharedProvider(std::sync::Arc<dyn crate::tools::ToolProvider>);
#[async_trait::async_trait]
impl crate::tools::ToolProvider for SharedProvider {
    fn name(&self) -> &'static str { self.0.name() }
    async fn list_tools(&self, scopes: &[String]) -> Vec<ToolSchema> { self.0.list_tools(scopes).await }
    async fn call(&self, name: &str, args: &serde_json::Value, scopes: &[String]) -> crate::tools::ToolResult {
        self.0.call(name, args, scopes).await
    }
}

/// Load recent thread messages and map them to chat messages (the agent's own messages
/// become assistant turns; everyone else becomes user turns, prefixed with the sender).
async fn fetch_history_as_chat(
    db: &sqlx::PgPool, thread_id: &str, agent_addr: &str,
) -> Vec<ChatMessage> {
    let rows: Vec<(String, serde_json::Value)> = sqlx::query_as(
        "SELECT from_addr, content FROM messages WHERE thread_id = $1 ORDER BY sequence ASC LIMIT 20",
    )
    .bind(thread_id)
    .fetch_all(db)
    .await
    .unwrap_or_else(|e| {
        tracing::warn!("[runtime] failed to load thread history for {thread_id}: {e}");
        Vec::new()
    });

    rows.into_iter().map(|(from, content)| {
        let text = content_to_text(&content);
        if from == agent_addr {
            ChatMessage::assistant_text(text)
        } else {
            ChatMessage::user(format!("{from}: {text}"))
        }
    }).collect()
}

/// Flatten a content block array to plain text. Content blocks in this codebase carry the
/// readable string under `text` (with `fallback` as a back-up), so prefer those.
fn content_to_text(content: &serde_json::Value) -> String {
    if let Some(arr) = content.as_array() {
        arr.iter()
            .filter_map(|b| {
                b.get("text")
                    .or_else(|| b.get("fallback"))
                    .and_then(|t| t.as_str())
            })
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        content.as_str().unwrap_or("").to_string()
    }
}

/// Stream the reply to thread participants: start → one delta with full text → end,
/// then a final signed `ainm.message.deliver`.
async fn stream_reply(state: &ServerState, agent: &crate::hosted_agent::HostedAgent, thread_id: &str, text: &str) {
    let stream_id = uuid::Uuid::now_v7().to_string();

    let start = serde_json::json!({
        "jsonrpc": "2.0", "method": "ainm.stream.start",
        "params": {"stream_id": stream_id, "thread_id": thread_id, "from": agent.address}
    });
    state.human_registry.send_to_thread(thread_id, &start.to_string()).await;

    let delta = serde_json::json!({
        "jsonrpc": "2.0", "method": "ainm.stream.delta",
        "params": {"stream_id": stream_id, "thread_id": thread_id, "delta": text}
    });
    state.human_registry.send_to_thread(thread_id, &delta.to_string()).await;

    let end = serde_json::json!({
        "jsonrpc": "2.0", "method": "ainm.stream.end",
        "params": {"stream_id": stream_id, "thread_id": thread_id}
    });
    state.human_registry.send_to_thread(thread_id, &end.to_string()).await;

    // Build the inner message in the same shape the server uses for normal sends
    // (`id`, `from`, `thread_id`, `content`, `timestamp`), sign it, then wrap it in the
    // standard `ainm.message.deliver` envelope (`params = {message, verified}`) so the
    // iOS/human client can correlate and render it like any other delivered message.
    let mut message = serde_json::json!({
        "id": uuid::Uuid::now_v7().to_string(),
        "from": agent.address,
        "thread_id": thread_id,
        "content": [{"type": "text", "text": text}],
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    // Sign the complete message — this MUST be the last mutation before delivery.
    if let Err(e) = crate::signing::sign_envelope(&agent.key_pair, &mut message) {
        tracing::warn!("[runtime] failed to sign agent message: {e}");
    }

    let envelope = serde_json::json!({
        "jsonrpc": "2.0", "method": "ainm.message.deliver",
        "params": {
            "message": message,
            "verified": true,
        }
    });
    state.human_registry.send_to_thread(thread_id, &envelope.to_string()).await;
}
