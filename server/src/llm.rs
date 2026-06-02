//! LLM client abstraction for memory compilation.
//!
//! Provides a trait-based interface so production uses OpenAI/Anthropic
//! while tests use a deterministic mock.

use crate::error::AppError;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};

// ── Extraction types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateNode {
    pub node_type: String,   // semantic, episodic, procedural
    pub category: String,    // preference, fact, skill, event, decision, pattern, etc.
    pub label: String,
    pub confidence: f32,
    pub properties: serde_json::Value,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateEdge {
    pub source_label: String,
    pub target_label: String,
    pub relationship: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub nodes: Vec<CandidateNode>,
    pub edges: Vec<CandidateEdge>,
}

// ── Tool-use types ─────────────────────────────────────────────────────────

/// A tool the agent may call, advertised to the LLM (OpenAI function shape).
#[derive(Debug, Clone, Serialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    /// JSON Schema object for the tool's arguments.
    pub parameters: serde_json::Value,
}

/// A tool call requested by the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// One message in the running chat (system/user/assistant/tool).
#[derive(Debug, Clone)]
pub enum ChatMessage {
    System(String),
    User(String),
    AssistantText(String),
    AssistantToolCalls(Vec<ToolCall>),
    ToolResult { tool_call_id: String, content: String },
}

impl ChatMessage {
    pub fn system(s: impl Into<String>) -> Self { ChatMessage::System(s.into()) }
    pub fn user(s: impl Into<String>) -> Self { ChatMessage::User(s.into()) }
    pub fn assistant_text(s: impl Into<String>) -> Self { ChatMessage::AssistantText(s.into()) }
    pub fn assistant_tool_calls(calls: Vec<ToolCall>) -> Self { ChatMessage::AssistantToolCalls(calls) }
    pub fn tool_result(id: impl Into<String>, content: impl Into<String>) -> Self {
        ChatMessage::ToolResult { tool_call_id: id.into(), content: content.into() }
    }
}

impl Serialize for ChatMessage {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            ChatMessage::System(c) => {
                let mut st = s.serialize_struct("m", 2)?;
                st.serialize_field("role", "system")?;
                st.serialize_field("content", c)?;
                st.end()
            }
            ChatMessage::User(c) => {
                let mut st = s.serialize_struct("m", 2)?;
                st.serialize_field("role", "user")?;
                st.serialize_field("content", c)?;
                st.end()
            }
            ChatMessage::AssistantText(c) => {
                let mut st = s.serialize_struct("m", 2)?;
                st.serialize_field("role", "assistant")?;
                st.serialize_field("content", c)?;
                st.end()
            }
            ChatMessage::AssistantToolCalls(calls) => {
                let tool_calls: Vec<serde_json::Value> = calls.iter().map(|c| {
                    serde_json::json!({
                        "id": c.id,
                        "type": "function",
                        "function": { "name": c.name, "arguments": c.arguments.to_string() }
                    })
                }).collect();
                let mut st = s.serialize_struct("m", 3)?;
                st.serialize_field("role", "assistant")?;
                st.serialize_field("content", &serde_json::Value::Null)?;
                st.serialize_field("tool_calls", &tool_calls)?;
                st.end()
            }
            ChatMessage::ToolResult { tool_call_id, content } => {
                let mut st = s.serialize_struct("m", 3)?;
                st.serialize_field("role", "tool")?;
                st.serialize_field("tool_call_id", tool_call_id)?;
                st.serialize_field("content", content)?;
                st.end()
            }
        }
    }
}

/// The result of one LLM turn.
#[derive(Debug, Clone)]
pub enum AssistantTurn {
    Text(String),
    ToolCalls(Vec<ToolCall>),
}

// ── LLM Client trait ───────────────────────────────────────────────────────

#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    /// Extract entities and relationships from a batch of conversation messages.
    async fn extract(
        &self,
        messages: &[ConversationMessage],
    ) -> Result<ExtractionResult, AppError>;

    /// Run one chat turn with tools available. Returns either final assistant text
    /// or the tool calls the model wants executed. Single request/response (the runtime
    /// loop calls this repeatedly).
    async fn chat_with_tools(
        &self,
        _messages: &[ChatMessage],
        _tools: &[ToolSchema],
    ) -> Result<AssistantTurn, AppError> {
        Err(AppError::Protocol { code: -32601, message: "chat_with_tools not supported".into() })
    }
}

/// A simplified conversation message for the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub from: String,
    pub content: String,
    pub timestamp: String,
}

// ── OpenAI-compatible client ───────────────────────────────────────────────

pub struct OpenAiClient {
    http: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAiClient {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("failed to build HTTP client"),
            api_key,
            model,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
        }
    }

    async fn chat_completion(&self, system: &str, user: &str) -> Result<String, AppError> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user },
            ],
            "temperature": 0.1,
            "response_format": { "type": "json_object" },
        });

        let resp = self.http
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Protocol {
                code: -32050,
                message: format!("LLM request failed: {e}"),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Protocol {
                code: -32050,
                message: format!("LLM returned {status}: {text}"),
            });
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| AppError::Protocol {
            code: -32050,
            message: format!("LLM response parse failed: {e}"),
        })?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("{}")
            .to_string();

        Ok(content)
    }
}

#[async_trait::async_trait]
impl LlmClient for OpenAiClient {
    async fn extract(
        &self,
        messages: &[ConversationMessage],
    ) -> Result<ExtractionResult, AppError> {
        let conversation = messages
            .iter()
            .map(|m| format!("[{}] {}: {}", m.timestamp, m.from, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        // Run entity and relationship extraction in a single call for cost efficiency
        let system = extraction_system_prompt();
        let user_prompt = format!(
            "Extract entities and relationships from this conversation:\n\n{conversation}\n\n\
             Respond with JSON matching the schema described in the system prompt."
        );

        let response_text = self.chat_completion(&system, &user_prompt).await?;

        // Parse the JSON response
        let parsed: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
            tracing::warn!("[llm] Failed to parse extraction response: {e}");
            AppError::Protocol {
                code: -32050,
                message: format!("LLM output parse failed: {e}"),
            }
        })?;

        let nodes = parse_nodes(&parsed);
        let edges = parse_edges(&parsed);

        tracing::info!(
            "[compiler] Extracted {} nodes, {} edges from {} messages",
            nodes.len(),
            edges.len(),
            messages.len()
        );

        Ok(ExtractionResult { nodes, edges })
    }

    async fn chat_with_tools(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolSchema],
    ) -> Result<AssistantTurn, AppError> {
        let tools_json: Vec<serde_json::Value> = tools.iter().map(|t| {
            serde_json::json!({
                "type": "function",
                "function": { "name": t.name, "description": t.description, "parameters": t.parameters }
            })
        }).collect();

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.1,
        });
        if !tools_json.is_empty() {
            body["tools"] = serde_json::Value::Array(tools_json);
            body["tool_choice"] = serde_json::json!("auto");
        }

        let resp = self.http
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Protocol {
                code: -32050,
                message: format!("LLM request failed: {e}"),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Protocol {
                code: -32050,
                message: format!("LLM returned {status}: {text}"),
            });
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| AppError::Protocol {
            code: -32050,
            message: format!("LLM parse failed: {e}"),
        })?;

        let message = &json["choices"][0]["message"];
        if let Some(tcs) = message["tool_calls"].as_array() {
            if !tcs.is_empty() {
                let calls = tcs.iter().filter_map(|tc| {
                    let id = tc["id"].as_str()?.to_string();
                    let name = tc["function"]["name"].as_str()?.to_string();
                    let raw = tc["function"]["arguments"].as_str().unwrap_or("{}");
                    let arguments = serde_json::from_str(raw).unwrap_or_else(|e| {
                        tracing::warn!("[llm] tool_call {id} has unparseable arguments: {e}");
                        serde_json::json!({})
                    });
                    Some(ToolCall { id, name, arguments })
                }).collect::<Vec<_>>();
                if !calls.is_empty() {
                    return Ok(AssistantTurn::ToolCalls(calls));
                }
                tracing::warn!(
                    "[llm] tool_calls array had {} entries but all were malformed; treating as text",
                    tcs.len()
                );
            }
        }
        let text = message["content"].as_str().unwrap_or("").to_string();
        Ok(AssistantTurn::Text(text))
    }
}

// ── Mock client for tests ──────────────────────────────────────────────────

pub struct MockLlmClient {
    /// Pre-configured extraction results returned in order.
    pub results: std::sync::Mutex<Vec<ExtractionResult>>,
    /// Pre-configured chat_with_tools turns returned in order.
    pub turns: std::sync::Mutex<Vec<AssistantTurn>>,
}

impl MockLlmClient {
    pub fn new(results: Vec<ExtractionResult>) -> Self {
        Self {
            results: std::sync::Mutex::new(results),
            turns: std::sync::Mutex::new(vec![]),
        }
    }

    /// Create a mock that returns an empty extraction result.
    pub fn empty() -> Self {
        Self::new(vec![ExtractionResult {
            nodes: vec![],
            edges: vec![],
        }])
    }

    /// Build a mock that scripts chat_with_tools turns (FIFO).
    pub fn with_turns(turns: Vec<AssistantTurn>) -> Self {
        Self {
            results: std::sync::Mutex::new(vec![]),
            turns: std::sync::Mutex::new(turns),
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for MockLlmClient {
    async fn extract(
        &self,
        _messages: &[ConversationMessage],
    ) -> Result<ExtractionResult, AppError> {
        let mut results = self.results.lock().unwrap();
        if results.is_empty() {
            Ok(ExtractionResult {
                nodes: vec![],
                edges: vec![],
            })
        } else {
            Ok(results.remove(0))
        }
    }

    async fn chat_with_tools(
        &self,
        _messages: &[ChatMessage],
        _tools: &[ToolSchema],
    ) -> Result<AssistantTurn, AppError> {
        let mut turns = self.turns.lock().unwrap();
        if turns.is_empty() {
            Ok(AssistantTurn::Text(String::new()))
        } else {
            Ok(turns.remove(0))
        }
    }
}

// ── Prompt ─────────────────────────────────────────────────────────────────

const EXTRACTION_SYSTEM_PROMPT_TEMPLATE: &str = r#"You are a knowledge extraction specialist. Extract structured entities and relationships from conversations.

## Rules
- Extract SPECIFIC entities only: named people, technologies, tools, projects, preferences, decisions, events, patterns
- Do NOT extract abstract concepts, pronouns, generic nouns, or vague terms
- Prefer specific labels: "TypeScript" not "a programming language", "VS Code" not "code editor"
- Relationships must reference extracted entity labels exactly
- Assign confidence: 0.9-1.0 for explicitly stated facts, 0.7-0.89 for strongly implied, 0.5-0.69 for inferred

## Entity Types
- semantic: stable facts (categories: <<SEMANTIC>>)
- episodic: time-bound events (categories: <<EPISODIC>>)
- procedural: learned patterns (categories: <<PROCEDURAL>>)

## Output Schema (JSON)
{
  "entities": [
    {
      "type": "semantic|episodic|procedural",
      "category": "<<ALL>>",
      "label": "human-readable description",
      "confidence": 0.0-1.0,
      "properties": {},
      "tags": []
    }
  ],
  "relationships": [
    {
      "source": "entity label (must match an entity above)",
      "target": "entity label (must match an entity above)",
      "type": "uses|prefers_over|works_at|decided_on|led_to|requires|part_of|learned_from",
      "confidence": 0.0-1.0
    }
  ]
}

The "category" field MUST be exactly one of: <<ALL>>. Do not invent new categories.
Only output valid JSON. No markdown, no explanation."#;

/// Build the extraction system prompt with the canonical, closed category set.
fn extraction_system_prompt() -> String {
    EXTRACTION_SYSTEM_PROMPT_TEMPLATE
        .replace("<<SEMANTIC>>", &crate::categories::SEMANTIC.join(", "))
        .replace("<<EPISODIC>>", &crate::categories::EPISODIC.join(", "))
        .replace("<<PROCEDURAL>>", &crate::categories::PROCEDURAL.join(", "))
        .replace("<<ALL>>", &crate::categories::all().join("|"))
}

// ── Response parsing helpers ───────────────────────────────────────────────

fn parse_nodes(json: &serde_json::Value) -> Vec<CandidateNode> {
    let entities = json.get("entities").and_then(|e| e.as_array());
    let Some(entities) = entities else { return vec![] };

    entities
        .iter()
        .filter_map(|e| {
            let node_type = e.get("type").and_then(|t| t.as_str())?.to_string();
            let category = e.get("category").and_then(|c| c.as_str())?.to_string();
            let label = e.get("label").and_then(|l| l.as_str())?.to_string();
            if label.is_empty() {
                return None;
            }
            let confidence = e
                .get("confidence")
                .and_then(|c| c.as_f64())
                .unwrap_or(0.7) as f32;
            let properties = e.get("properties").cloned().unwrap_or(serde_json::json!({}));
            let tags: Vec<String> = e
                .get("tags")
                .and_then(|t| t.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            Some(CandidateNode {
                node_type,
                category,
                label,
                confidence,
                properties,
                tags,
            })
        })
        .collect()
}

fn parse_edges(json: &serde_json::Value) -> Vec<CandidateEdge> {
    let relationships = json.get("relationships").and_then(|r| r.as_array());
    let Some(relationships) = relationships else {
        return vec![];
    };

    relationships
        .iter()
        .filter_map(|r| {
            let source = r.get("source").and_then(|s| s.as_str())?.to_string();
            let target = r.get("target").and_then(|t| t.as_str())?.to_string();
            let relationship = r.get("type").and_then(|t| t.as_str())?.to_string();
            let confidence = r
                .get("confidence")
                .and_then(|c| c.as_f64())
                .unwrap_or(0.7) as f32;

            Some(CandidateEdge {
                source_label: source,
                target_label: target,
                relationship,
                confidence,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_message_serializes_openai_shape() {
        let m = ChatMessage::user("hello");
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["role"], "user");
        assert_eq!(v["content"], "hello");

        let tool_call = ToolCall {
            id: "call_1".into(),
            name: "query_memory".into(),
            arguments: serde_json::json!({"q": "x"}),
        };
        let assistant = ChatMessage::assistant_tool_calls(vec![tool_call.clone()]);
        let av = serde_json::to_value(&assistant).unwrap();
        assert_eq!(av["role"], "assistant");
        assert_eq!(av["tool_calls"][0]["function"]["name"], "query_memory");

        let tool_result = ChatMessage::tool_result("call_1", "the answer");
        let tv = serde_json::to_value(&tool_result).unwrap();
        assert_eq!(tv["role"], "tool");
        assert_eq!(tv["tool_call_id"], "call_1");
    }

    #[test]
    fn assistant_turn_distinguishes_text_from_tool_calls() {
        let text = AssistantTurn::Text("done".into());
        assert!(matches!(text, AssistantTurn::Text(_)));
        let calls = AssistantTurn::ToolCalls(vec![ToolCall {
            id: "c".into(), name: "n".into(), arguments: serde_json::json!({}),
        }]);
        assert!(matches!(calls, AssistantTurn::ToolCalls(_)));
    }

    #[tokio::test]
    async fn mock_scripts_multi_turn_tool_use() {
        let mock = MockLlmClient::with_turns(vec![
            AssistantTurn::ToolCalls(vec![ToolCall {
                id: "call_1".into(), name: "query_memory".into(),
                arguments: serde_json::json!({"q": "prefs"}),
            }]),
            AssistantTurn::Text("Based on your prefs, done.".into()),
        ]);

        let t1 = mock.chat_with_tools(&[ChatMessage::user("hi")], &[]).await.unwrap();
        assert!(matches!(t1, AssistantTurn::ToolCalls(_)));
        let t2 = mock.chat_with_tools(&[ChatMessage::user("hi")], &[]).await.unwrap();
        match t2 { AssistantTurn::Text(s) => assert!(s.contains("done")), _ => panic!("expected text") }
    }

    #[test]
    fn extraction_prompt_is_closed_over_canonical_categories() {
        let p = extraction_system_prompt();
        for c in crate::categories::all() {
            assert!(p.contains(c), "prompt must mention canonical category `{c}`");
        }
        assert!(
            !p.contains("<<"),
            "all template tokens must be substituted (found a leftover `<<`)"
        );
        assert!(
            !p.contains("|...") && !p.contains("...\""),
            "category set must be closed — no open-ended `...`"
        );
    }
}
