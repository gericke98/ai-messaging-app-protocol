//! The hosted agent's bounded, consent-gated, tool-calling loop.

use crate::llm::{AssistantTurn, ChatMessage, LlmClient, ToolSchema};
use crate::tools::ToolRegistry;

/// Outcome of one full agent turn.
pub struct TurnOutcome {
    pub final_text: String,
    pub tool_rounds: usize,
    pub stopped_early: bool,
}

/// Run the LLM↔tool loop. Collaborators are injected so this is unit-testable.
/// `scopes` are the requester's granted consent scopes (gates the offered tool set).
pub async fn run_loop(
    llm: &dyn LlmClient,
    tools: &ToolRegistry,
    system_prompt: &str,
    scopes: &[String],
    history: Vec<ChatMessage>,
    inbound_user_text: &str,
    max_tool_rounds: usize,
) -> TurnOutcome {
    let offered: Vec<ToolSchema> = tools.list_tools(scopes).await;

    let mut messages: Vec<ChatMessage> = Vec::new();
    messages.push(ChatMessage::system(system_prompt));
    messages.extend(history);
    messages.push(ChatMessage::user(inbound_user_text));

    let mut rounds = 0usize;
    loop {
        let turn = match llm.chat_with_tools(&messages, &offered).await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("[runtime] LLM error: {e}");
                return TurnOutcome {
                    final_text: "Sorry — I hit an error contacting my model. Please try again.".into(),
                    tool_rounds: rounds,
                    stopped_early: true,
                };
            }
        };

        match turn {
            AssistantTurn::Text(text) => {
                return TurnOutcome { final_text: text, tool_rounds: rounds, stopped_early: false };
            }
            AssistantTurn::ToolCalls(calls) => {
                if rounds >= max_tool_rounds {
                    return TurnOutcome {
                        final_text: "I wasn't able to reach a final answer within my allowed number of \
                                     tool-use steps. Please try rephrasing or narrowing your question.".into(),
                        tool_rounds: rounds,
                        stopped_early: true,
                    };
                }
                rounds += 1;
                // Record the assistant's tool-call message, then each tool result.
                messages.push(ChatMessage::assistant_tool_calls(calls.clone()));
                for call in calls {
                    let result = tools.call(&call.name, &call.arguments, scopes).await;
                    let payload = if result.is_error {
                        format!("ERROR: {}", result.content)
                    } else {
                        result.content
                    };
                    messages.push(ChatMessage::tool_result(&call.id, payload));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{AssistantTurn, MockLlmClient, ToolCall};
    use crate::tools::{ToolProvider, ToolRegistry, ToolResult};

    struct CannedProvider;
    #[async_trait::async_trait]
    impl ToolProvider for CannedProvider {
        async fn list_tools(&self, _scopes: &[String]) -> Vec<crate::llm::ToolSchema> {
            vec![crate::llm::ToolSchema {
                name: "query_memory".into(), description: "q".into(),
                parameters: serde_json::json!({"type":"object"}),
            }]
        }
        async fn call(&self, name: &str, _args: &serde_json::Value, _scopes: &[String]) -> ToolResult {
            if name == "query_memory" { ToolResult::ok("user likes Rust") }
            else { ToolResult::error("no") }
        }
    }

    #[tokio::test]
    async fn loop_runs_tool_then_finalizes() {
        let llm = MockLlmClient::with_turns(vec![
            AssistantTurn::ToolCalls(vec![ToolCall {
                id: "c1".into(), name: "query_memory".into(), arguments: serde_json::json!({"query":"prefs"}),
            }]),
            AssistantTurn::Text("You like Rust.".into()),
        ]);
        let reg = ToolRegistry::new(vec![Box::new(CannedProvider)]);
        let out = run_loop(
            &llm, &reg,
            "system", &["identity".to_string()],
            vec![], "tell me about myself",
            8,
        ).await;
        assert_eq!(out.final_text, "You like Rust.");
        assert_eq!(out.tool_rounds, 1);
        assert!(!out.stopped_early);
    }

    #[tokio::test]
    async fn loop_stops_at_cap() {
        // LLM always asks for a tool → must stop at max_tool_rounds.
        let turns = (0..20).map(|_| AssistantTurn::ToolCalls(vec![ToolCall {
            id: "c".into(), name: "query_memory".into(), arguments: serde_json::json!({}),
        }])).collect();
        let llm = MockLlmClient::with_turns(turns);
        let reg = ToolRegistry::new(vec![Box::new(CannedProvider)]);
        let out = run_loop(&llm, &reg, "system", &["identity".into()], vec![], "go", 3).await;
        assert_eq!(out.tool_rounds, 3);
        assert!(out.stopped_early);
        assert!(!out.final_text.is_empty()); // a "stopped early" note
    }
}
