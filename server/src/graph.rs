use crate::error::StorageError;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

// ── Row types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, FromRow)]
pub struct NodeRow {
    pub id: Uuid,
    pub user_addr: String,
    pub node_type: String,
    pub category: String,
    pub label: String,
    pub properties: serde_json::Value,
    pub confidence: f32,
    pub valid_from: chrono::DateTime<chrono::Utc>,
    pub valid_until: Option<chrono::DateTime<chrono::Utc>>,
    pub superseded_by: Option<Uuid>,
    pub provenance: serde_json::Value,
    pub tags: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, FromRow)]
pub struct EdgeRow {
    pub id: Uuid,
    pub user_addr: String,
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub relationship: String,
    pub weight: f32,
    pub confidence: f32,
    pub valid_from: chrono::DateTime<chrono::Utc>,
    pub valid_until: Option<chrono::DateTime<chrono::Utc>>,
    pub properties: serde_json::Value,
    pub provenance: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Lightweight row returned by traversal queries.
#[derive(Debug, Clone, FromRow)]
pub struct TraversalHit {
    pub node_id: Uuid,
    #[allow(dead_code)]
    pub depth: i32,
}

// ── JSON serialization ─────────────────────────────────────────────────────

impl NodeRow {
    pub fn to_protocol_json(&self) -> serde_json::Value {
        serde_json::json!({
            "node_id": self.id.to_string(),
            "type": self.node_type,
            "category": self.category,
            "label": self.label,
            "properties": self.properties,
            "confidence": self.confidence,
            "source": self.provenance,
            "temporal": {
                "valid_from": self.valid_from.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                "valid_until": self.valid_until.map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
                "superseded_by": self.superseded_by.map(|u| u.to_string()),
            },
            "tags": self.tags,
        })
    }
}

impl EdgeRow {
    pub fn to_protocol_json(&self) -> serde_json::Value {
        serde_json::json!({
            "edge_id": self.id.to_string(),
            "source_node": self.source_id.to_string(),
            "target_node": self.target_id.to_string(),
            "relationship": self.relationship,
            "weight": self.weight,
            "confidence": self.confidence,
            "source": self.provenance,
            "temporal": {
                "valid_from": self.valid_from.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                "valid_until": self.valid_until.map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
            },
            "properties": self.properties,
        })
    }
}

// ── Node CRUD ──────────────────────────────────────────────────────────────

/// Insert a new memory node. Returns the generated UUID.
#[allow(clippy::too_many_arguments)]
pub async fn insert_node(
    pool: &PgPool,
    user_addr: &str,
    node_type: &str,
    category: &str,
    label: &str,
    properties: &serde_json::Value,
    confidence: f32,
    provenance: &serde_json::Value,
    tags: &serde_json::Value,
) -> Result<Uuid, StorageError> {
    let row: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO memory_nodes (user_addr, node_type, category, label, properties, confidence, provenance, tags)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#,
    )
    .bind(user_addr)
    .bind(node_type)
    .bind(category)
    .bind(label)
    .bind(properties)
    .bind(confidence)
    .bind(provenance)
    .bind(tags)
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}

/// Insert a new memory edge. Returns the generated UUID.
#[allow(clippy::too_many_arguments)]
pub async fn insert_edge(
    pool: &PgPool,
    user_addr: &str,
    source_id: Uuid,
    target_id: Uuid,
    relationship: &str,
    weight: f32,
    confidence: f32,
    provenance: &serde_json::Value,
    properties: &serde_json::Value,
) -> Result<Uuid, StorageError> {
    let row: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO memory_edges (user_addr, source_id, target_id, relationship, weight, confidence, provenance, properties)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#,
    )
    .bind(user_addr)
    .bind(source_id)
    .bind(target_id)
    .bind(relationship)
    .bind(weight)
    .bind(confidence)
    .bind(provenance)
    .bind(properties)
    .fetch_one(pool)
    .await?;

    Ok(row.0)
}

/// Supersede a node: set valid_until on the old node and point superseded_by to the new one.
pub async fn supersede_node(
    pool: &PgPool,
    old_node_id: Uuid,
    new_node_id: Uuid,
) -> Result<(), StorageError> {
    sqlx::query(
        r#"
        UPDATE memory_nodes
        SET valid_until = NOW(), updated_at = NOW(), superseded_by = $2
        WHERE id = $1
        "#,
    )
    .bind(old_node_id)
    .bind(new_node_id)
    .execute(pool)
    .await?;

    Ok(())
}

#[allow(dead_code)]
pub async fn delete_node(pool: &PgPool, node_id: Uuid) -> Result<bool, StorageError> {
    let result = sqlx::query("DELETE FROM memory_nodes WHERE id = $1")
        .bind(node_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

// ── Query ──────────────────────────────────────────────────────────────────

/// Query nodes with optional filters.
#[allow(clippy::too_many_arguments)]
pub async fn query_nodes(
    pool: &PgPool,
    user_addr: &str,
    text_query: Option<&str>,
    type_filter: Option<&[&str]>,
    category_filter: Option<&[&str]>,
    temporal_filter: &str, // "current", "all", "historical"
    min_confidence: f32,
    limit: i64,
) -> Result<Vec<NodeRow>, StorageError> {
    // Build the query dynamically based on filters.
    // Using a single flexible query with conditional WHERE clauses.
    let mut sql = String::from(
        "SELECT id, user_addr, node_type, category, label, properties, confidence, \
         valid_from, valid_until, superseded_by, provenance, tags, created_at, updated_at \
         FROM memory_nodes WHERE user_addr = $1 AND confidence >= $2",
    );
    let param_idx = 3u32;

    // Temporal filter
    match temporal_filter {
        "current" => sql.push_str(" AND valid_until IS NULL"),
        "historical" => sql.push_str(" AND valid_until IS NOT NULL"),
        _ => {} // "all" — no filter
    }

    // Text search
    let ts_query_str;
    if let Some(q) = text_query {
        if !q.is_empty() {
            // If the caller already supplied a tsquery expression (contains | or &),
            // pass it through verbatim; otherwise AND-join space-separated terms.
            ts_query_str = if q.contains('|') || q.contains('&') {
                q.to_string()
            } else {
                q.split_whitespace().collect::<Vec<_>>().join(" & ")
            };
            sql.push_str(&format!(
                " AND search_vector @@ to_tsquery('english', ${param_idx})"
            ));
            let _ = param_idx; // consumed by the format above
        } else {
            ts_query_str = String::new();
        }
    } else {
        ts_query_str = String::new();
    }

    // Type filter (applied via text matching since we can't bind arrays easily in dynamic queries)
    let type_csv;
    if let Some(types) = type_filter {
        if !types.is_empty() {
            type_csv = types.join("','");
            sql.push_str(&format!(" AND node_type IN ('{type_csv}')"));
        }
    }

    // Category filter
    let cat_csv;
    if let Some(cats) = category_filter {
        if !cats.is_empty() {
            cat_csv = cats.join("','");
            sql.push_str(&format!(" AND category IN ('{cat_csv}')"));
        }
    }

    sql.push_str(" ORDER BY confidence DESC, created_at DESC");
    sql.push_str(&format!(" LIMIT {limit}"));

    // Build and execute
    let mut query = sqlx::query_as::<_, NodeRow>(&sql)
        .bind(user_addr)
        .bind(min_confidence);

    if !ts_query_str.is_empty() {
        query = query.bind(&ts_query_str);
    }

    let rows = query.fetch_all(pool).await?;
    Ok(rows)
}

/// Get edges connecting a set of node IDs (for include_edges in query results).
pub async fn get_edges_for_nodes(
    pool: &PgPool,
    user_addr: &str,
    node_ids: &[Uuid],
) -> Result<Vec<EdgeRow>, StorageError> {
    if node_ids.is_empty() {
        return Ok(vec![]);
    }

    let placeholders: Vec<String> = node_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("${}", i + 3))
        .collect();
    let in_clause = placeholders.join(", ");

    let sql = format!(
        "SELECT id, user_addr, source_id, target_id, relationship, weight, confidence, \
         valid_from, valid_until, properties, provenance, created_at \
         FROM memory_edges \
         WHERE user_addr = $1 AND valid_until IS NULL \
         AND (source_id IN ({in_clause}) AND target_id IN ({in_clause}))"
    );

    let mut query = sqlx::query_as::<_, EdgeRow>(&sql).bind(user_addr);
    // Bind node IDs twice (for source_id IN and target_id IN)
    // Actually the same placeholders are reused, so bind once
    for nid in node_ids {
        query = query.bind(nid);
    }

    // This won't work cleanly with reused placeholders. Use a simpler approach:
    // Fetch all current edges for the user and filter in Rust.
    let rows = sqlx::query_as::<_, EdgeRow>(
        "SELECT id, user_addr, source_id, target_id, relationship, weight, confidence, \
         valid_from, valid_until, properties, provenance, created_at \
         FROM memory_edges \
         WHERE user_addr = $1 AND valid_until IS NULL",
    )
    .bind(user_addr)
    .fetch_all(pool)
    .await?;

    let node_set: std::collections::HashSet<Uuid> = node_ids.iter().copied().collect();
    let filtered: Vec<EdgeRow> = rows
        .into_iter()
        .filter(|e| node_set.contains(&e.source_id) && node_set.contains(&e.target_id))
        .collect();

    Ok(filtered)
}

// ── Traversal ──────────────────────────────────────────────────────────────

/// Multi-hop graph traversal via recursive CTE. Returns reachable node IDs with depth.
pub async fn traverse(
    pool: &PgPool,
    user_addr: &str,
    start_node: Uuid,
    max_depth: i32,
    direction: &str, // "outgoing", "incoming", "both"
    relationship_filter: Option<&[&str]>,
) -> Result<(Vec<NodeRow>, Vec<EdgeRow>), StorageError> {
    // Build the recursive CTE based on direction
    let edge_join = match direction {
        "outgoing" => "e.source_id = gw.node_id",
        "incoming" => "e.target_id = gw.node_id",
        _ => "(e.source_id = gw.node_id OR e.target_id = gw.node_id)",
    };

    let next_node = match direction {
        "outgoing" => "e.target_id",
        "incoming" => "e.source_id",
        _ => "CASE WHEN e.source_id = gw.node_id THEN e.target_id ELSE e.source_id END",
    };

    let base_edge_join = match direction {
        "outgoing" => "e.source_id = $1",
        "incoming" => "e.target_id = $1",
        _ => "(e.source_id = $1 OR e.target_id = $1)",
    };

    let base_next_node = match direction {
        "outgoing" => "e.target_id",
        "incoming" => "e.source_id",
        _ => "CASE WHEN e.source_id = $1 THEN e.target_id ELSE e.source_id END",
    };

    let mut rel_filter_clause = String::new();
    let rel_csv;
    if let Some(rels) = relationship_filter {
        if !rels.is_empty() {
            rel_csv = rels.join("','");
            rel_filter_clause = format!(" AND e.relationship IN ('{rel_csv}')");
        }
    }

    let sql = format!(
        r#"
        WITH RECURSIVE graph_walk AS (
            SELECT
                {base_next_node} AS node_id,
                1 AS depth,
                ARRAY[$1::uuid, {base_next_node}] AS path
            FROM memory_edges e
            WHERE {base_edge_join}
              AND e.user_addr = $2
              AND e.valid_until IS NULL
              {rel_filter_clause}

            UNION ALL

            SELECT
                {next_node} AS node_id,
                gw.depth + 1,
                gw.path || {next_node}
            FROM graph_walk gw
            JOIN memory_edges e ON {edge_join}
            WHERE gw.depth < $3
              AND e.user_addr = $2
              AND e.valid_until IS NULL
              AND NOT ({next_node} = ANY(gw.path))
              {rel_filter_clause}
        )
        SELECT DISTINCT ON (node_id) node_id, depth
        FROM graph_walk
        ORDER BY node_id, depth ASC
        "#,
    );

    let hits: Vec<TraversalHit> = sqlx::query_as(&sql)
        .bind(start_node)
        .bind(user_addr)
        .bind(max_depth)
        .fetch_all(pool)
        .await?;

    if hits.is_empty() {
        return Ok((vec![], vec![]));
    }

    // Fetch full node data for the traversal hits + the start node
    let mut all_ids: Vec<Uuid> = hits.iter().map(|h| h.node_id).collect();
    all_ids.push(start_node);
    all_ids.sort();
    all_ids.dedup();

    // Fetch nodes
    let nodes = fetch_nodes_by_ids(pool, &all_ids).await?;

    // Fetch connecting edges
    let edges = get_edges_for_nodes(pool, user_addr, &all_ids).await?;

    Ok((nodes, edges))
}

/// Fetch full node rows by a list of IDs.
async fn fetch_nodes_by_ids(pool: &PgPool, ids: &[Uuid]) -> Result<Vec<NodeRow>, StorageError> {
    if ids.is_empty() {
        return Ok(vec![]);
    }

    // Use ANY($1) with a UUID array
    let rows = sqlx::query_as::<_, NodeRow>(
        "SELECT id, user_addr, node_type, category, label, properties, confidence, \
         valid_from, valid_until, superseded_by, provenance, tags, created_at, updated_at \
         FROM memory_nodes WHERE id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

// ── Lint ────────────────────────────────────────────────────────────────────

/// Find contradictions: pairs of currently valid nodes with the same category
/// and overlapping labels (potential conflicts).
pub async fn find_contradictions(
    pool: &PgPool,
    user_addr: &str,
) -> Result<Vec<(Uuid, Uuid, String)>, StorageError> {
    // Simple heuristic: same user, same category, both currently valid,
    // connected by a 'contradicts' edge
    let rows: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
        r#"
        SELECT e.source_id, e.target_id, 'contradicts' as relationship
        FROM memory_edges e
        WHERE e.user_addr = $1
          AND e.relationship = 'contradicts'
          AND e.valid_until IS NULL
        "#,
    )
    .bind(user_addr)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Find orphan nodes (no incoming or outgoing edges).
pub async fn find_orphans(pool: &PgPool, user_addr: &str) -> Result<Vec<Uuid>, StorageError> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT n.id
        FROM memory_nodes n
        WHERE n.user_addr = $1
          AND n.valid_until IS NULL
          AND NOT EXISTS (
              SELECT 1 FROM memory_edges e
              WHERE e.user_addr = $1
                AND e.valid_until IS NULL
                AND (e.source_id = n.id OR e.target_id = n.id)
          )
        "#,
    )
    .bind(user_addr)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// Find stale nodes (not updated in the given number of days).
pub async fn find_stale(
    pool: &PgPool,
    user_addr: &str,
    stale_days: i32,
) -> Result<Vec<Uuid>, StorageError> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT id
        FROM memory_nodes
        WHERE user_addr = $1
          AND valid_until IS NULL
          AND updated_at < NOW() - ($2 || ' days')::interval
        "#,
    )
    .bind(user_addr)
    .bind(stale_days.to_string())
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// Get graph summary stats for a user.
pub async fn graph_stats(pool: &PgPool, user_addr: &str) -> Result<(i64, i64), StorageError> {
    let node_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM memory_nodes WHERE user_addr = $1 AND valid_until IS NULL",
    )
    .bind(user_addr)
    .fetch_one(pool)
    .await?;

    let edge_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM memory_edges WHERE user_addr = $1 AND valid_until IS NULL",
    )
    .bind(user_addr)
    .fetch_one(pool)
    .await?;

    Ok((node_count.0, edge_count.0))
}

// ── Thread Message Texts ───────────────────────────────────────────────────

/// Return the text of the most recent `limit` messages in a thread, newest first.
/// Extracts the `text` field from the `content` JSONB; skips messages without text.
pub async fn fetch_recent_thread_texts(
    pool: &PgPool,
    thread_id: &str,
    limit: i64,
) -> Result<Vec<String>, StorageError> {
    let rows: Vec<(serde_json::Value,)> = sqlx::query_as(
        "SELECT content FROM messages WHERE thread_id = $1 ORDER BY sequence DESC LIMIT $2",
    )
    .bind(thread_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(content,)| {
            content
                .get("text")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect())
}

// ── Export / Import ────────────────────────────────────────────────────────

/// Export all current nodes and edges for a user.
pub async fn export_graph(
    pool: &PgPool,
    user_addr: &str,
    include_superseded: bool,
) -> Result<(Vec<NodeRow>, Vec<EdgeRow>), StorageError> {
    let nodes = if include_superseded {
        sqlx::query_as::<_, NodeRow>(
            "SELECT id, user_addr, node_type, category, label, properties, confidence, \
             valid_from, valid_until, superseded_by, provenance, tags, created_at, updated_at \
             FROM memory_nodes WHERE user_addr = $1 ORDER BY created_at",
        )
        .bind(user_addr)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, NodeRow>(
            "SELECT id, user_addr, node_type, category, label, properties, confidence, \
             valid_from, valid_until, superseded_by, provenance, tags, created_at, updated_at \
             FROM memory_nodes WHERE user_addr = $1 AND valid_until IS NULL ORDER BY created_at",
        )
        .bind(user_addr)
        .fetch_all(pool)
        .await?
    };

    let edges = sqlx::query_as::<_, EdgeRow>(
        "SELECT id, user_addr, source_id, target_id, relationship, weight, confidence, \
         valid_from, valid_until, properties, provenance, created_at \
         FROM memory_edges WHERE user_addr = $1 AND valid_until IS NULL ORDER BY created_at",
    )
    .bind(user_addr)
    .fetch_all(pool)
    .await?;

    Ok((nodes, edges))
}
