//! LLM client abstraction for memory compilation.
//!
//! Provides a trait-based interface so production uses OpenAI/Anthropic
//! while tests use a deterministic mock.

use crate::error::AppError;
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

// ── LLM Client trait ───────────────────────────────────────────────────────

#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    /// Extract entities and relationships from a batch of conversation messages.
    async fn extract(
        &self,
        messages: &[ConversationMessage],
    ) -> Result<ExtractionResult, AppError>;
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
        let system = EXTRACTION_SYSTEM_PROMPT;
        let user_prompt = format!(
            "Extract entities and relationships from this conversation:\n\n{conversation}\n\n\
             Respond with JSON matching the schema described in the system prompt."
        );

        let response_text = self.chat_completion(system, &user_prompt).await?;

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
}

// ── Mock client for tests ──────────────────────────────────────────────────

pub struct MockLlmClient {
    /// Pre-configured extraction results returned in order.
    pub results: std::sync::Mutex<Vec<ExtractionResult>>,
}

impl MockLlmClient {
    pub fn new(results: Vec<ExtractionResult>) -> Self {
        Self {
            results: std::sync::Mutex::new(results),
        }
    }

    /// Create a mock that returns an empty extraction result.
    pub fn empty() -> Self {
        Self::new(vec![ExtractionResult {
            nodes: vec![],
            edges: vec![],
        }])
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
}

// ── Prompt ─────────────────────────────────────────────────────────────────

const EXTRACTION_SYSTEM_PROMPT: &str = r#"You are a knowledge extraction specialist. Extract structured entities and relationships from conversations.

## Rules
- Extract SPECIFIC entities only: named people, technologies, tools, projects, preferences, decisions, events, patterns
- Do NOT extract abstract concepts, pronouns, generic nouns, or vague terms
- Prefer specific labels: "TypeScript" not "a programming language", "VS Code" not "code editor"
- Relationships must reference extracted entity labels exactly
- Assign confidence: 0.9-1.0 for explicitly stated facts, 0.7-0.89 for strongly implied, 0.5-0.69 for inferred

## Entity Types
- semantic: stable facts (categories: preference, fact, skill, relationship, tool)
- episodic: time-bound events (categories: event, decision, outcome, interaction)
- procedural: learned patterns (categories: pattern, workflow, strategy, communication)

## Output Schema (JSON)
{
  "entities": [
    {
      "type": "semantic|episodic|procedural",
      "category": "preference|fact|skill|tool|event|decision|pattern|...",
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

Only output valid JSON. No markdown, no explanation."#;

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
