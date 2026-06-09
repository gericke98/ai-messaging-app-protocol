//! In-process memory tools exposed to the hosted agent, consent-scoped per requester.

use crate::briefing::{self, ConsentLevel};
use crate::llm::ToolSchema;
use crate::tools::{ToolProvider, ToolResult};
use sqlx::PgPool;

/// Map each memory tool to the scope a requester must have been granted to be offered it.
/// `None` means "offered whenever the requester has any granted scope".
fn required_scope(tool: &str) -> Option<&'static str> {
    match tool {
        "retrieve_briefing" => None,
        "query_memory" => None,
        "store_fact" => Some("preferences"),
        _ => Some("__never__"),
    }
}

/// Names of tools offered for the given granted scopes (pure; used by the consent gate + tests).
pub fn offered_tool_names(scopes: &[String]) -> Vec<String> {
    if scopes.is_empty() {
        return vec![];
    }
    ["retrieve_briefing", "query_memory", "store_fact"]
        .into_iter()
        .filter(|t| match required_scope(t) {
            None => true,
            Some(req) => scopes.iter().any(|s| s == req),
        })
        .map(|s| s.to_string())
        .collect()
}

/// The union of memory categories visible for the granted scopes (pure; used to filter results).
pub fn granted_categories(scopes: &[String]) -> Vec<&'static str> {
    let mut cats: Vec<&'static str> = Vec::new();
    for s in scopes {
        for c in briefing::scope_to_categories(s) {
            if !cats.contains(&c) {
                cats.push(c);
            }
        }
    }
    cats
}

fn schema_for(name: &str) -> ToolSchema {
    match name {
        "retrieve_briefing" => ToolSchema {
            name: name.into(),
            description: "Retrieve the consent-scoped memory briefing about the person you are talking with.".into(),
            parameters: serde_json::json!({"type":"object","properties":{}}),
        },
        "query_memory" => ToolSchema {
            name: name.into(),
            description: "Search the person's memory graph for facts you are permitted to see.".into(),
            parameters: serde_json::json!({
                "type":"object",
                "properties":{"query":{"type":"string","description":"free-text search"}},
                "required":["query"]
            }),
        },
        "store_fact" => ToolSchema {
            name: name.into(),
            description: "Store a durable preference/fact about the person (only when they ask you to remember something).".into(),
            parameters: serde_json::json!({
                "type":"object",
                "properties":{
                    "label":{"type":"string"},
                    "value":{"type":"string"}
                },
                "required":["label","value"]
            }),
        },
        _ => ToolSchema {
            name: name.into(),
            description: String::new(),
            parameters: serde_json::json!({"type":"object"}),
        },
    }
}

pub struct MemoryToolProvider {
    pub(crate) db: PgPool,
    pub(crate) user_addr: String,
    pub(crate) agent_addr: String,
    pub(crate) thread_id: String,
    pub(crate) default_consent: ConsentLevel,
}

#[async_trait::async_trait]
impl ToolProvider for MemoryToolProvider {
    fn name(&self) -> &'static str {
        "memory"
    }

    async fn list_tools(&self, scopes: &[String]) -> Vec<ToolSchema> {
        offered_tool_names(scopes)
            .iter()
            .map(|n| schema_for(n))
            .collect()
    }

    async fn call(&self, name: &str, args: &serde_json::Value, scopes: &[String]) -> ToolResult {
        // Consent gate (direction 2): refuse any tool not in the offered set.
        if !offered_tool_names(scopes).iter().any(|t| t == name) {
            return ToolResult::error(format!(
                "tool '{name}' is not permitted by current consent scopes"
            ));
        }
        match name {
            "retrieve_briefing" => {
                match briefing::generate_briefing(
                    &self.db,
                    &self.user_addr,
                    &self.agent_addr,
                    &self.thread_id,
                    self.default_consent,
                )
                .await
                {
                    Ok(b) => ToolResult::ok(b.to_string()),
                    Err(e) => ToolResult::error(format!("briefing failed: {e}")),
                }
            }
            "query_memory" => {
                let q = args["query"].as_str().unwrap_or("");
                let cats = granted_categories(scopes);
                if cats.is_empty() {
                    // No granted categories → nothing is visible. Short-circuit so the
                    // visibility guarantee doesn't depend on the downstream SQL filter.
                    return ToolResult::ok(serde_json::json!({"results": []}).to_string());
                }
                let cat_refs: Vec<&str> = cats.to_vec();
                match crate::graph::query_nodes(
                    &self.db,
                    &self.user_addr,
                    Some(q),
                    None,
                    Some(&cat_refs),
                    "current",
                    0.0,
                    10,
                )
                .await
                {
                    Ok(nodes) => {
                        // Defense in depth: filter again to granted categories.
                        let visible: Vec<_> = nodes
                            .into_iter()
                            .filter(|n| cats.contains(&n.category.as_str()))
                            .map(|n| {
                                serde_json::json!({
                                    "label": n.label,
                                    "category": n.category,
                                    "confidence": n.confidence,
                                })
                            })
                            .collect();
                        ToolResult::ok(serde_json::json!({"results": visible}).to_string())
                    }
                    Err(e) => ToolResult::error(format!("query failed: {e}")),
                }
            }
            "store_fact" => {
                let label = args["label"].as_str().unwrap_or("");
                let value = args["value"].as_str().unwrap_or("");
                if label.is_empty() {
                    return ToolResult::error("label is required".to_string());
                }
                if value.is_empty() {
                    return ToolResult::error("value is required".to_string());
                }
                let props = serde_json::json!({"value": value});
                let prov = serde_json::json!({"source": "hosted-agent", "agent": self.agent_addr});
                match crate::graph::insert_node(
                    &self.db,
                    &self.user_addr,
                    "preference",
                    "preference",
                    label,
                    &props,
                    0.8,
                    &prov,
                    &serde_json::json!([]),
                )
                .await
                {
                    Ok(_) => ToolResult::ok(format!("stored: {label}")),
                    Err(e) => ToolResult::error(format!("store failed: {e}")),
                }
            }
            other => ToolResult::error(format!("unknown memory tool: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_scope_does_not_offer_store_fact() {
        let scopes = vec!["identity".to_string()];
        let names = offered_tool_names(&scopes);
        assert!(names.contains(&"retrieve_briefing".to_string()));
        assert!(names.contains(&"query_memory".to_string()));
        assert!(!names.contains(&"store_fact".to_string()));
    }

    #[test]
    fn preferences_scope_offers_store_fact() {
        let scopes = vec!["identity".to_string(), "preferences".to_string()];
        let names = offered_tool_names(&scopes);
        assert!(names.contains(&"store_fact".to_string()));
    }

    #[test]
    fn empty_scopes_offers_nothing() {
        assert!(offered_tool_names(&[]).is_empty());
    }

    #[test]
    fn granted_categories_unions_scopes() {
        let scopes = vec!["identity".to_string(), "preferences".to_string()];
        let cats = granted_categories(&scopes);
        assert!(cats.contains(&"fact")); // from identity
        assert!(cats.contains(&"preference")); // from preferences
        assert!(!cats.contains(&"skill")); // expertise not granted
    }
}
