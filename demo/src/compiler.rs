//! Memory compilation engine.
//!
//! Transforms raw conversation messages into structured knowledge in the
//! user's memory graph. Inspired by Karpathy's LLM knowledge base (compiler
//! analogy) and Graphiti's task-separated extraction pipeline.

use crate::error::AppError;
use crate::graph;
use crate::llm::{CandidateEdge, CandidateNode, ConversationMessage, LlmClient};
use crate::protocol;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// A request to compile memory from a set of conversation messages.
#[derive(Debug, Clone)]
pub struct CompileRequest {
    pub user_addr: String,
    pub thread_id: String,
    pub messages: Vec<ConversationMessage>,
}

/// Result summary of a compilation pass.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CompileResult {
    pub compilation_id: String,
    pub nodes_created: usize,
    pub nodes_updated: usize,
    pub nodes_superseded: usize,
    pub edges_created: usize,
}

/// Run a full compilation pass: extract → dedup → integrate → record.
pub async fn run_compilation(
    db: PgPool,
    llm: Arc<dyn LlmClient>,
    req: CompileRequest,
) -> Result<CompileResult, AppError> {
    let compilation_id = protocol::new_message_id();
    let start = chrono::Utc::now();

    tracing::info!(
        "[compiler] Starting compilation {} for {} ({} messages from thread {})",
        compilation_id,
        req.user_addr,
        req.messages.len(),
        req.thread_id,
    );

    // Skip trivial conversations
    let meaningful: Vec<&ConversationMessage> = req
        .messages
        .iter()
        .filter(|m| is_meaningful(&m.content))
        .collect();

    if meaningful.is_empty() {
        tracing::info!("[compiler] No meaningful messages to compile, skipping");
        return Ok(CompileResult {
            compilation_id,
            nodes_created: 0,
            nodes_updated: 0,
            nodes_superseded: 0,
            edges_created: 0,
        });
    }

    // Step 1: LLM extraction
    let extraction = llm.extract(&req.messages).await?;

    // Step 1b: Ingestion-side category enforcement. The extraction prompt
    // instructs the model to use the canonical closed set, but the model may
    // still deviate. Normalize trivial casing/whitespace variance to the
    // canonical form; drop (and log) any node whose category is genuinely
    // off-list so storage only ever holds consent-mappable categories.
    let valid_nodes: Vec<CandidateNode> = extraction
        .nodes
        .into_iter()
        .filter_map(|mut c| match crate::categories::normalize(&c.category) {
            Some(canon) => {
                c.category = canon.to_string();
                Some(c)
            }
            None => {
                tracing::warn!(
                    "[compiler] dropping node with off-list category {:?} (label {:?})",
                    c.category,
                    c.label
                );
                None
            }
        })
        .collect();

    // Step 2: Dedup candidates against existing graph
    let (new_nodes, updated_count, superseded_count) =
        dedup_and_integrate_nodes(&db, &req.user_addr, &req.thread_id, &valid_nodes).await?;

    // Step 3: Create edges between nodes
    let edges_created =
        integrate_edges(&db, &req.user_addr, &req.thread_id, &extraction.edges, &new_nodes)
            .await?;

    // Step 4: Record compilation pass
    record_compilation(
        &db,
        &req.user_addr,
        &req.thread_id,
        new_nodes.len(),
        updated_count,
        superseded_count,
        edges_created,
        start,
    )
    .await?;

    let result = CompileResult {
        compilation_id: compilation_id.clone(),
        nodes_created: new_nodes.len(),
        nodes_updated: updated_count,
        nodes_superseded: superseded_count,
        edges_created,
    };

    tracing::info!(
        "[compiler] Compilation {} complete: {} nodes created, {} updated, {} superseded, {} edges",
        compilation_id,
        result.nodes_created,
        result.nodes_updated,
        result.nodes_superseded,
        result.edges_created,
    );

    Ok(result)
}

/// Filter out trivial messages (very short, emoji-only, system messages).
fn is_meaningful(content: &str) -> bool {
    let trimmed = content.trim();
    if trimmed.len() < 5 {
        return false;
    }
    // Skip pure punctuation/emoji
    let alpha_count = trimmed.chars().filter(|c| c.is_alphanumeric()).count();
    alpha_count >= 3
}

/// Dedup candidate nodes against the existing graph and insert new ones.
/// Returns (map of label→node_id for newly created nodes, updated_count, superseded_count).
async fn dedup_and_integrate_nodes(
    db: &PgPool,
    user_addr: &str,
    thread_id: &str,
    candidates: &[CandidateNode],
) -> Result<(HashMap<String, Uuid>, usize, usize), AppError> {
    let mut node_map: HashMap<String, Uuid> = HashMap::new();
    let mut updated = 0usize;
    let mut superseded = 0usize;

    for candidate in candidates {
        // Check for existing node with same label and type (simple text-match dedup)
        let existing = find_existing_node(db, user_addr, &candidate.label, &candidate.node_type).await?;

        match existing {
            Some((existing_id, existing_confidence)) => {
                // Node already exists — check if we should update or supersede
                if candidate.confidence > existing_confidence {
                    // Higher confidence: update the existing node's confidence
                    sqlx::query(
                        "UPDATE memory_nodes SET confidence = $1, updated_at = NOW() WHERE id = $2",
                    )
                    .bind(candidate.confidence)
                    .bind(existing_id)
                    .execute(db)
                    .await
                    .map_err(|e| AppError::Storage(e.into()))?;
                    updated += 1;
                }
                // Map this label to the existing node
                node_map.insert(candidate.label.clone(), existing_id);
            }
            None => {
                // New node — check for contradictions with same category
                let contradicted = find_contradicting_node(
                    db,
                    user_addr,
                    &candidate.category,
                    &candidate.label,
                )
                .await?;

                let provenance = serde_json::json!({
                    "thread_id": thread_id,
                    "contributed_by": format!("compiler@{}", user_addr.split('@').nth(1).unwrap_or("local")),
                    "extracted_at": protocol::now_rfc3339(),
                });
                let tags = serde_json::json!(candidate.tags);

                let new_id = graph::insert_node(
                    db,
                    user_addr,
                    &candidate.node_type,
                    &candidate.category,
                    &candidate.label,
                    &candidate.properties,
                    candidate.confidence,
                    &provenance,
                    &tags,
                )
                .await
                .map_err(|e| AppError::Storage(e))?;

                // If there's a contradicting node, supersede it
                if let Some(old_id) = contradicted {
                    graph::supersede_node(db, old_id, new_id)
                        .await
                        .map_err(|e| AppError::Storage(e))?;

                    // Create a 'contradicts' edge for auditability
                    let edge_prov = serde_json::json!({
                        "thread_id": thread_id,
                        "contributed_by": "compiler",
                        "reason": "supersession",
                    });
                    let _ = graph::insert_edge(
                        db,
                        user_addr,
                        new_id,
                        old_id,
                        "contradicts",
                        1.0,
                        1.0,
                        &edge_prov,
                        &serde_json::json!({}),
                    )
                    .await;
                    superseded += 1;
                }

                node_map.insert(candidate.label.clone(), new_id);
            }
        }
    }

    Ok((node_map, updated, superseded))
}

/// Find an existing node with the same label and type (case-insensitive).
async fn find_existing_node(
    db: &PgPool,
    user_addr: &str,
    label: &str,
    node_type: &str,
) -> Result<Option<(Uuid, f32)>, AppError> {
    let row: Option<(Uuid, f32)> = sqlx::query_as(
        "SELECT id, confidence FROM memory_nodes \
         WHERE user_addr = $1 AND LOWER(label) = LOWER($2) AND node_type = $3 AND valid_until IS NULL \
         LIMIT 1",
    )
    .bind(user_addr)
    .bind(label)
    .bind(node_type)
    .fetch_optional(db)
    .await
    .map_err(|e| AppError::Storage(e.into()))?;

    Ok(row)
}

/// Find a contradicting node: same category, currently valid, with a "preference"
/// or "fact" category where having two active would be contradictory.
/// Simple heuristic: for preference and tool categories, only one active node per subcategory.
async fn find_contradicting_node(
    _db: &PgPool,
    _user_addr: &str,
    category: &str,
    _label: &str,
) -> Result<Option<Uuid>, AppError> {
    // Only check for contradictions in categories where duplicates are meaningful
    if !matches!(category, "preference" | "tool") {
        return Ok(None);
    }

    // For now, we don't auto-supersede based on category alone.
    // Contradictions are detected more precisely when the LLM explicitly identifies them.
    // Future: use embedding similarity to detect semantic contradictions.
    Ok(None)
}

/// Create edges between newly integrated nodes based on candidate edges.
async fn integrate_edges(
    db: &PgPool,
    user_addr: &str,
    thread_id: &str,
    candidates: &[CandidateEdge],
    node_map: &HashMap<String, Uuid>,
) -> Result<usize, AppError> {
    let mut created = 0usize;

    let provenance = serde_json::json!({
        "thread_id": thread_id,
        "contributed_by": format!("compiler@{}", user_addr.split('@').nth(1).unwrap_or("local")),
        "extracted_at": protocol::now_rfc3339(),
    });

    for candidate in candidates {
        // Resolve source and target labels to node IDs
        let source_id = match node_map.get(&candidate.source_label) {
            Some(id) => *id,
            None => {
                // Try to find by label in the graph
                match find_existing_node(db, user_addr, &candidate.source_label, "semantic").await? {
                    Some((id, _)) => id,
                    None => continue, // Skip edge if source not found
                }
            }
        };

        let target_id = match node_map.get(&candidate.target_label) {
            Some(id) => *id,
            None => {
                match find_existing_node(db, user_addr, &candidate.target_label, "semantic").await? {
                    Some((id, _)) => id,
                    None => continue,
                }
            }
        };

        // Skip self-loops
        if source_id == target_id {
            continue;
        }

        // Check for existing edge between the same nodes with same relationship
        let exists: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM memory_edges \
             WHERE user_addr = $1 AND source_id = $2 AND target_id = $3 \
             AND relationship = $4 AND valid_until IS NULL \
             LIMIT 1",
        )
        .bind(user_addr)
        .bind(source_id)
        .bind(target_id)
        .bind(&candidate.relationship)
        .fetch_optional(db)
        .await
        .map_err(|e| AppError::Storage(e.into()))?;

        if exists.is_some() {
            continue; // Edge already exists
        }

        match graph::insert_edge(
            db,
            user_addr,
            source_id,
            target_id,
            &candidate.relationship,
            1.0,
            candidate.confidence,
            &provenance,
            &serde_json::json!({}),
        )
        .await
        {
            Ok(_) => created += 1,
            Err(e) => {
                tracing::warn!("[compiler] Failed to create edge: {e}");
            }
        }
    }

    Ok(created)
}

/// Record the compilation pass in the database for audit.
async fn record_compilation(
    db: &PgPool,
    user_addr: &str,
    thread_id: &str,
    nodes_created: usize,
    nodes_updated: usize,
    nodes_superseded: usize,
    edges_created: usize,
    started_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO compilation_passes
        (user_addr, source_threads, nodes_created, nodes_updated, nodes_superseded, edges_created, started_at, completed_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
        "#,
    )
    .bind(user_addr)
    .bind(serde_json::json!([thread_id]))
    .bind(nodes_created as i32)
    .bind(nodes_updated as i32)
    .bind(nodes_superseded as i32)
    .bind(edges_created as i32)
    .bind(started_at)
    .execute(db)
    .await
    .map_err(|e| AppError::Storage(e.into()))?;

    Ok(())
}

// ── Background compilation worker ──────────────────────────────────────────

/// Spawn a background worker that processes compilation requests.
pub fn spawn_compilation_worker(
    mut rx: tokio::sync::mpsc::Receiver<CompileRequest>,
    db: PgPool,
    llm: Arc<dyn LlmClient>,
    notify_tx: tokio::sync::broadcast::Sender<String>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!("[compiler] Background compilation worker started");

        while let Some(req) = rx.recv().await {
            let db = db.clone();
            let llm = llm.clone();
            let notify = notify_tx.clone();
            let user_addr = req.user_addr.clone();

            tokio::spawn(async move {
                match run_compilation(db, llm, req).await {
                    Ok(result) => {
                        // Send compilation notification to user's connected clients
                        let notification = protocol::new_notification(
                            "ainm.memory.compile",
                            serde_json::json!({
                                "user_identity": user_addr,
                                "compilation_id": result.compilation_id,
                                "timestamp": protocol::now_rfc3339(),
                                "result": {
                                    "nodes_created": result.nodes_created,
                                    "nodes_updated": result.nodes_updated,
                                    "nodes_superseded": result.nodes_superseded,
                                    "edges_created": result.edges_created,
                                    "contradictions_resolved": result.nodes_superseded,
                                    "lint_warnings": 0,
                                }
                            }),
                        );
                        let _ = notify.send(serde_json::to_string(&notification).unwrap());
                    }
                    Err(e) => {
                        tracing::error!("[compiler] Compilation failed: {e}");
                    }
                }
            });
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_meaningful_rejects_short() {
        assert!(!is_meaningful("hi"));
        assert!(!is_meaningful("ok"));
        assert!(!is_meaningful(""));
        assert!(!is_meaningful("   "));
    }

    #[test]
    fn test_is_meaningful_rejects_punctuation() {
        assert!(!is_meaningful("!!!???"));
        assert!(!is_meaningful("..."));
    }

    #[test]
    fn test_is_meaningful_accepts_content() {
        assert!(is_meaningful("I like Rust programming"));
        assert!(is_meaningful("Can you help me with this?"));
        assert!(is_meaningful("Hello world"));
    }

    #[test]
    fn test_is_meaningful_edge_cases() {
        assert!(!is_meaningful("ab"));        // < 5 chars
        assert!(is_meaningful("a b c"));      // 3 alpha, len 5 — passes both checks
        assert!(!is_meaningful(".. .."));     // len 5 but only 0 alpha
    }
}
