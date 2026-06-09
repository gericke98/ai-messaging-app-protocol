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
    #[allow(clippy::should_implement_trait)] // infallible (defaults on unknown), unlike FromStr
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
///
/// The union of all scopes' categories must equal `crate::categories::all()`
/// (enforced by `tests::scope_map_exactly_covers_the_canonical_set`). Scopes may
/// overlap: `goals ⊂ history` by design — `history` simply adds `outcome` and
/// `interaction` on top of the `event`/`decision` that `goals` also grants.
pub fn scope_to_categories(scope: &str) -> Vec<&'static str> {
    match scope {
        "identity" => vec!["fact", "relationship"],
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

    // Consent enforcement: an agent sees only the categories its resolved scopes permit.
    let mut allowed: Vec<&str> = scopes
        .iter()
        .flat_map(|s| scope_to_categories(s))
        .collect();
    allowed.sort_unstable();
    allowed.dedup();

    // Granted nothing → see nothing. graph::query_nodes treats Some(&[]) as "no filter"
    // (it would return ALL categories), so we must short-circuit rather than pass an empty slice.
    if allowed.is_empty() {
        let _ = sqlx::query(
            "INSERT INTO briefing_log (user_addr, agent_addr, thread_id, scope, consent_level, node_count) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(user_addr)
        .bind(agent_addr)
        .bind(thread_id)
        .bind("none")
        .bind(consent_level.as_str())
        .bind(0i32)
        .execute(db)
        .await;

        return Ok(serde_json::json!({
            "briefing_id": briefing_id,
            "thread_id": thread_id,
            "user_identity": user_addr,
            "generated_at": protocol::now_rfc3339(),
            "scope": "none",
            "consent_level": consent_level.as_str(),
            "sections": serde_json::Map::<String, serde_json::Value>::new(),
            "graph_snapshot": serde_json::Value::Null,
            "peer_card": "",
            "char_budget_used": 0,
        }));
    }

    // --- hybrid retrieval: query as BOOST, not filter (within the consent-allowed categories) ---

    // 1) Broad candidate set: all CURRENT facts within the allowed categories.
    let candidates = graph::query_nodes(
        db, user_addr, None, None, Some(allowed.as_slice()), "current", 0.0, 200,
    ).await.map_err(AppError::Storage)?;

    // 2) Query-matched ids (used as a ranking BOOST, not a filter).
    let recent = graph::fetch_recent_thread_texts(db, thread_id, 8).await.unwrap_or_default();
    let query = retrieval::build_query_string(&recent);
    let matched_ids: std::collections::HashSet<uuid::Uuid> = match query.as_deref() {
        Some(q) => graph::query_nodes(db, user_addr, Some(q), None, Some(allowed.as_slice()), "current", 0.0, 200)
            .await.unwrap_or_default().into_iter().map(|n| n.id).collect(),
        None => std::collections::HashSet::new(),
    };

    // 3) Score: recency x confidence, boosted if query-matched.
    const QUERY_BOOST: f32 = 3.0;
    let now = chrono::Utc::now();
    let ranked: Vec<retrieval::RankedItem> = candidates.iter().enumerate().map(|(i, n)| {
        let base = retrieval::score_node(n.valid_from, now, n.confidence);
        let boost = if matched_ids.contains(&n.id) { QUERY_BOOST } else { 1.0 };
        retrieval::RankedItem { id: i as i64, score: base * boost, char_len: n.label.len() }
    }).collect();
    const BRIEFING_CHAR_BUDGET: usize = 4000;
    const BRIEFING_MIN_SCORE: f32 = 0.05; // drop facts whose recency×confidence(×boost) is negligible (e.g. >~4 months stale)
    let kept_idx = retrieval::pack_to_budget(ranked, BRIEFING_CHAR_BUDGET, BRIEFING_MIN_SCORE);
    let nodes: Vec<graph::NodeRow> = kept_idx.iter().map(|&i| candidates[i as usize].clone()).collect();

    // 4) Peer card from identity facts — gated by the identity scope (full consent enforcement).
    // Categories fact/relationship; the compiler never emits a literal "identity" category.
    let peer_card = if scopes.iter().any(|s| s.as_str() == "identity") {
        let identity_cats: Vec<&str> = scope_to_categories("identity");
        let identity_nodes = graph::query_nodes(
            db, user_addr, None, None, Some(&identity_cats), "current", 0.0, 20,
        ).await.unwrap_or_default();
        let identity_labels: Vec<String> = identity_nodes.iter().map(|n| n.label.clone()).collect();
        retrieval::build_peer_card(&identity_labels, 400)
    } else {
        String::new()
    };
    let char_budget_used: usize = nodes.iter().map(|n| n.label.len()).sum();

    // Keep a reference to all_nodes for graph_snapshot and briefing_log (unchanged logic below).
    let all_nodes = nodes.clone();

    // Determine scope label
    let scope_label = if scopes.len() >= 6 {
        "full"
    } else if scopes.len() >= 3 {
        "domain-scoped"
    } else {
        "minimal"
    };

    // 5) Sections grouped DYNAMICALLY by the node's actual category (never empty when facts exist).
    let mut sections: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    for n in &all_nodes {
        sections.entry(n.category.clone())
            .or_insert_with(|| serde_json::Value::Array(Vec::new()))
            .as_array_mut().unwrap()
            .push(n.to_protocol_json());
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
        "char_budget_used": char_budget_used,
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
        assert_eq!(scope_to_categories("identity"), vec!["fact", "relationship"]);
        assert_eq!(scope_to_categories("preferences"), vec!["preference", "tool"]);
        assert_eq!(scope_to_categories("expertise"), vec!["skill"]);
        assert!(scope_to_categories("unknown").is_empty());
    }

    #[test]
    fn scope_map_exactly_covers_the_canonical_set() {
        use std::collections::BTreeSet;
        let mapped: BTreeSet<&str> = SCOPES
            .iter()
            .copied()
            .flat_map(scope_to_categories)
            .collect();
        let canonical: BTreeSet<&str> = crate::categories::all().into_iter().collect();
        assert_eq!(
            mapped, canonical,
            "every canonical category must be reachable by some scope, and no scope may \
             map to a non-canonical category"
        );
    }
}
