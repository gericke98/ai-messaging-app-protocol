//! Agent briefing generation.
//!
//! When an agent joins a thread, the server generates a scoped view of the
//! user's compound memory tailored to the agent's capabilities and the
//! user's consent level.

use crate::error::AppError;
use crate::graph;
use crate::protocol;
use crate::retrieval;
use sqlx::PgPool;

// ── Types ──────────────────────────────────────────────────────────────────

/// Memory scopes that can be independently granted/revoked.
#[allow(dead_code)]
pub const SCOPES: &[&str] = &[
    "identity",
    "preferences",
    "expertise",
    "goals",
    "history",
    "patterns",
];

/// Consent levels control baseline sharing behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentLevel {
    Minimal,  // identity only
    Standard, // + preferences, expertise, goals
    Full,     // all scopes
}

impl ConsentLevel {
    pub fn from_str(s: &str) -> Self {
        match s {
            "minimal" => Self::Minimal,
            "full" => Self::Full,
            _ => Self::Standard,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Standard => "standard",
            Self::Full => "full",
        }
    }

    /// Which scopes are included at this consent level.
    pub fn included_scopes(&self) -> Vec<&'static str> {
        match self {
            Self::Minimal => vec!["identity"],
            Self::Standard => vec!["identity", "preferences", "expertise", "goals"],
            Self::Full => vec![
                "identity",
                "preferences",
                "expertise",
                "goals",
                "history",
                "patterns",
            ],
        }
    }
}

/// Map scopes to node categories for graph filtering.
fn scope_to_categories(scope: &str) -> Vec<&'static str> {
    match scope {
        "identity" => vec!["fact"],
        "preferences" => vec!["preference", "tool"],
        "expertise" => vec!["skill"],
        "goals" => vec!["event", "decision"],
        "history" => vec!["event", "decision", "outcome", "interaction"],
        "patterns" => vec!["pattern", "workflow", "strategy", "communication"],
        _ => vec![],
    }
}

// ── Grant/Revoke ───────────────────────────────────────────────────────────

/// Get the effective scopes for an agent, checking per-agent grants first,
/// then falling back to the user's global consent level.
pub async fn resolve_scopes(
    db: &PgPool,
    user_addr: &str,
    agent_addr: &str,
    default_consent: ConsentLevel,
) -> Result<(Vec<String>, ConsentLevel), AppError> {
    // Check for per-agent grant
    let grant: Option<(serde_json::Value, String)> = sqlx::query_as(
        "SELECT scopes, consent_level FROM memory_grants \
         WHERE user_addr = $1 AND agent_addr = $2",
    )
    .bind(user_addr)
    .bind(agent_addr)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::Storage(e.into()))?;

    if let Some((scopes_json, level_str)) = grant {
        let level = ConsentLevel::from_str(&level_str);
        let scopes: Vec<String> = scopes_json
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        if scopes.is_empty() {
            Ok((level.included_scopes().into_iter().map(String::from).collect(), level))
        } else {
            Ok((scopes, level))
        }
    } else {
        Ok((
            default_consent.included_scopes().into_iter().map(String::from).collect(),
            default_consent,
        ))
    }
}

/// Grant memory scopes to an agent.
pub async fn grant_scopes(
    db: &PgPool,
    user_addr: &str,
    agent_addr: &str,
    scopes: &[String],
    consent_level: ConsentLevel,
) -> Result<(), AppError> {
    let scopes_json = serde_json::json!(scopes);
    sqlx::query(
        r#"
        INSERT INTO memory_grants (user_addr, agent_addr, scopes, consent_level)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (user_addr, agent_addr)
        DO UPDATE SET scopes = $3, consent_level = $4, granted_at = NOW()
        "#,
    )
    .bind(user_addr)
    .bind(agent_addr)
    .bind(&scopes_json)
    .bind(consent_level.as_str())
    .execute(db)
    .await
    .map_err(|e| AppError::Storage(e.into()))?;

    Ok(())
}

/// Revoke specific scopes from an agent.
pub async fn revoke_scopes(
    db: &PgPool,
    user_addr: &str,
    agent_addr: &str,
    scopes_to_revoke: &[String],
) -> Result<(), AppError> {
    // Get current grant
    let current: Option<(serde_json::Value,)> = sqlx::query_as(
        "SELECT scopes FROM memory_grants WHERE user_addr = $1 AND agent_addr = $2",
    )
    .bind(user_addr)
    .bind(agent_addr)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::Storage(e.into()))?;

    if let Some((current_scopes,)) = current {
        let mut remaining: Vec<String> = current_scopes
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        remaining.retain(|s| !scopes_to_revoke.contains(s));

        if remaining.is_empty() {
            // Remove the grant entirely
            sqlx::query("DELETE FROM memory_grants WHERE user_addr = $1 AND agent_addr = $2")
                .bind(user_addr)
                .bind(agent_addr)
                .execute(db)
                .await
                .map_err(|e| AppError::Storage(e.into()))?;
        } else {
            let scopes_json = serde_json::json!(remaining);
            sqlx::query(
                "UPDATE memory_grants SET scopes = $3, granted_at = NOW() \
                 WHERE user_addr = $1 AND agent_addr = $2",
            )
            .bind(user_addr)
            .bind(agent_addr)
            .bind(&scopes_json)
            .execute(db)
            .await
            .map_err(|e| AppError::Storage(e.into()))?;
        }
    }

    Ok(())
}

// ── Briefing generation ────────────────────────────────────────────────────

/// Generate a briefing for an agent joining a thread.
pub async fn generate_briefing(
    db: &PgPool,
    user_addr: &str,
    agent_addr: &str,
    thread_id: &str,
    default_consent: ConsentLevel,
) -> Result<serde_json::Value, AppError> {
    let briefing_id = protocol::new_message_id();

    // Resolve effective scopes
    let (scopes, consent_level) =
        resolve_scopes(db, user_addr, agent_addr, default_consent).await?;

    // Gather all allowed categories from the scopes
    let mut allowed_categories: Vec<&str> = Vec::new();
    for scope in &scopes {
        allowed_categories.extend(scope_to_categories(scope));
    }
    allowed_categories.sort();
    allowed_categories.dedup();

    // --- query-conditioned retrieval (MVP) ---
    let recent = graph::fetch_recent_thread_texts(db, thread_id, 8).await.unwrap_or_default();
    let query = retrieval::build_query_string(&recent);

    // When a FTS query is available, search broadly (no category filter) so
    // query-relevant facts surface regardless of how the caller labelled them.
    // On empty query fall back to the consent-scoped category filter (no regression).
    let candidates = graph::query_nodes(
        db,
        user_addr,
        query.as_deref(),
        None,
        if query.is_some() { None } else { Some(&allowed_categories) },
        "current",
        0.0,
        200,
    )
    .await
    .map_err(|e| AppError::Storage(e))?;

    let now = chrono::Utc::now();
    let ranked: Vec<retrieval::RankedItem> = candidates.iter().enumerate().map(|(i, n)| {
        retrieval::RankedItem {
            id: i as i64,
            score: retrieval::score_node(n.valid_from, now, n.confidence),
            char_len: n.label.len(),
        }
    }).collect();
    const BRIEFING_CHAR_BUDGET: usize = 4000;
    let kept_idx = retrieval::pack_to_budget(ranked, BRIEFING_CHAR_BUDGET);
    let nodes: Vec<graph::NodeRow> =
        kept_idx.iter().map(|&i| candidates[i as usize].clone()).collect();

    // Peer card: identity facts from a dedicated query so they are always
    // present regardless of consent-scope category mapping or budget cuts.
    let identity_candidates = graph::query_nodes(
        db, user_addr, None, None,
        Some(&["identity"]),
        "current", 0.0, 20,
    ).await.unwrap_or_default();
    let identity_labels: Vec<String> = identity_candidates
        .iter()
        .map(|n| n.label.clone())
        .collect();
    // Also include fact-category nodes from candidates (traditional identity scope).
    let fact_labels: Vec<String> = candidates.iter()
        .filter(|n| n.category == "fact")
        .map(|n| n.label.clone())
        .collect();
    let all_identity_labels: Vec<String> = identity_labels.into_iter()
        .chain(fact_labels)
        .collect();
    let peer_card = retrieval::build_peer_card(&all_identity_labels, 400);
    let token_budget_used: usize = nodes.iter().map(|n| n.label.len()).sum();

    // Keep a reference to all_nodes for graph_snapshot and briefing_log (unchanged logic below).
    let all_nodes = nodes.clone();

    // Organize nodes into briefing sections
    let mut identity = serde_json::Map::new();
    let mut preferences = Vec::new();
    let mut expertise = Vec::new();
    let mut active_goals = Vec::new();
    let mut interaction_patterns = Vec::new();
    let mut relevant_history = Vec::new();

    for node in &all_nodes {
        let summary = serde_json::json!({
            "node_id": node.id.to_string(),
            "label": node.label,
            "confidence": node.confidence,
            "valid_from": node.valid_from.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        });

        match node.category.as_str() {
            "fact" => {
                // Extract identity facts into the identity section
                if let Some(props) = node.properties.as_object() {
                    for (k, v) in props {
                        if let Some(s) = v.as_str() {
                            identity.insert(k.clone(), serde_json::Value::String(s.to_string()));
                        }
                    }
                }
                // Also add the label as a general fact
                if !node.label.is_empty() {
                    identity
                        .entry("description".to_string())
                        .or_insert(serde_json::Value::String(node.label.clone()));
                }
            }
            "preference" | "tool" => preferences.push(summary),
            "skill" => expertise.push(summary),
            "event" | "decision" => {
                if scopes.iter().any(|s| s == "goals") {
                    active_goals.push(summary.clone());
                }
                if scopes.iter().any(|s| s == "history") {
                    relevant_history.push(summary);
                }
            }
            "outcome" | "interaction" => relevant_history.push(summary),
            "pattern" | "workflow" | "strategy" | "communication" => {
                interaction_patterns.push(summary)
            }
            _ => {}
        }
    }

    // Determine scope label
    let scope_label = if scopes.len() >= 6 {
        "full"
    } else if scopes.len() >= 3 {
        "domain-scoped"
    } else {
        "minimal"
    };

    // Build the briefing
    let mut sections = serde_json::Map::new();
    if !identity.is_empty() {
        sections.insert("identity".to_string(), serde_json::Value::Object(identity));
    }
    if !preferences.is_empty() {
        sections.insert("preferences".to_string(), serde_json::json!(preferences));
    }
    if !expertise.is_empty() {
        sections.insert("expertise".to_string(), serde_json::json!(expertise));
    }
    if !active_goals.is_empty() {
        sections.insert("active_goals".to_string(), serde_json::json!(active_goals));
    }
    if !interaction_patterns.is_empty() {
        sections.insert(
            "interaction_patterns".to_string(),
            serde_json::json!(interaction_patterns),
        );
    }
    if !relevant_history.is_empty() {
        sections.insert(
            "relevant_history".to_string(),
            serde_json::json!(relevant_history),
        );
    }

    // Optional graph snapshot for full/domain-scoped
    let graph_snapshot = if scope_label != "minimal" && !all_nodes.is_empty() {
        let node_ids: Vec<uuid::Uuid> = all_nodes.iter().map(|n| n.id).collect();
        let edges = graph::get_edges_for_nodes(db, user_addr, &node_ids)
            .await
            .unwrap_or_default();

        serde_json::json!({
            "nodes": all_nodes.iter().map(|n| n.to_protocol_json()).collect::<Vec<_>>(),
            "edges": edges.iter().map(|e| e.to_protocol_json()).collect::<Vec<_>>(),
        })
    } else {
        serde_json::json!(null)
    };

    let node_count = all_nodes.len();

    // Record in briefing log
    let _ = sqlx::query(
        "INSERT INTO briefing_log (user_addr, agent_addr, thread_id, scope, consent_level, node_count) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(user_addr)
    .bind(agent_addr)
    .bind(thread_id)
    .bind(scope_label)
    .bind(consent_level.as_str())
    .bind(node_count as i32)
    .execute(db)
    .await;

    let briefing = serde_json::json!({
        "briefing_id": briefing_id,
        "thread_id": thread_id,
        "user_identity": user_addr,
        "generated_at": protocol::now_rfc3339(),
        "scope": scope_label,
        "consent_level": consent_level.as_str(),
        "sections": sections,
        "graph_snapshot": graph_snapshot,
        "peer_card": peer_card,
        "token_budget_used": token_budget_used,
    });

    tracing::info!(
        "[briefing] Generated briefing {} for agent {} ({} nodes, scope: {}, consent: {})",
        briefing_id,
        agent_addr,
        node_count,
        scope_label,
        consent_level.as_str(),
    );

    Ok(briefing)
}

/// Rough token estimate for briefing payload.
fn estimate_tokens(nodes: &[graph::NodeRow]) -> usize {
    // ~4 chars per token, estimate from labels + properties
    let chars: usize = nodes
        .iter()
        .map(|n| n.label.len() + n.properties.to_string().len())
        .sum();
    chars / 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consent_level_from_str() {
        assert_eq!(ConsentLevel::from_str("minimal"), ConsentLevel::Minimal);
        assert_eq!(ConsentLevel::from_str("standard"), ConsentLevel::Standard);
        assert_eq!(ConsentLevel::from_str("full"), ConsentLevel::Full);
        assert_eq!(ConsentLevel::from_str("unknown"), ConsentLevel::Standard);
        assert_eq!(ConsentLevel::from_str(""), ConsentLevel::Standard);
    }

    #[test]
    fn test_consent_level_as_str() {
        assert_eq!(ConsentLevel::Minimal.as_str(), "minimal");
        assert_eq!(ConsentLevel::Standard.as_str(), "standard");
        assert_eq!(ConsentLevel::Full.as_str(), "full");
    }

    #[test]
    fn test_included_scopes_minimal() {
        let scopes = ConsentLevel::Minimal.included_scopes();
        assert_eq!(scopes, vec!["identity"]);
    }

    #[test]
    fn test_included_scopes_standard() {
        let scopes = ConsentLevel::Standard.included_scopes();
        assert_eq!(scopes, vec!["identity", "preferences", "expertise", "goals"]);
    }

    #[test]
    fn test_included_scopes_full() {
        let scopes = ConsentLevel::Full.included_scopes();
        assert_eq!(scopes.len(), 6);
        assert!(scopes.contains(&"history"));
        assert!(scopes.contains(&"patterns"));
    }

    #[test]
    fn test_scope_to_categories() {
        assert_eq!(scope_to_categories("identity"), vec!["fact"]);
        assert_eq!(scope_to_categories("preferences"), vec!["preference", "tool"]);
        assert_eq!(scope_to_categories("expertise"), vec!["skill"]);
        assert!(scope_to_categories("unknown").is_empty());
    }
}
