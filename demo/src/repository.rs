use crate::error::StorageError;
use sqlx::{FromRow, PgPool};

/// Archival fact as stored in PostgreSQL.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, FromRow)]
pub struct ArchivalFactRow {
    pub fact_id: String,
    pub participant_addr: String,
    pub thread_id: Option<String>,
    pub key: String,
    pub value: serde_json::Value,
    pub tags: serde_json::Value,
    pub stored_by: String,
    pub stored_at: chrono::DateTime<chrono::Utc>,
}

impl ArchivalFactRow {
    /// Convert to the JSON shape expected by the AINM protocol.
    pub fn to_protocol_json(&self) -> serde_json::Value {
        serde_json::json!({
            "fact_id": self.fact_id,
            "key": self.key,
            "value": self.value,
            "tags": self.tags,
            "stored_by": self.stored_by,
            "stored_at": self.stored_at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        })
    }
}

/// Store a fact. On key collision for the same participant, replaces the existing fact.
pub async fn store_fact(
    pool: &PgPool,
    fact_id: &str,
    participant_addr: &str,
    thread_id: Option<&str>,
    key: &str,
    value: &serde_json::Value,
    tags: &serde_json::Value,
    stored_by: &str,
) -> Result<(), StorageError> {
    sqlx::query(
        r#"
        INSERT INTO archival_facts (fact_id, participant_addr, thread_id, key, value, tags, stored_by, stored_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
        ON CONFLICT (participant_addr, key)
        DO UPDATE SET fact_id = $1, value = $5, tags = $6, stored_by = $7, stored_at = NOW(), thread_id = $3
        "#,
    )
    .bind(fact_id)
    .bind(participant_addr)
    .bind(thread_id)
    .bind(key)
    .bind(value)
    .bind(tags)
    .bind(stored_by)
    .execute(pool)
    .await?;

    Ok(())
}

/// Query archival facts for a participant, optionally filtering by key prefix.
pub async fn query_facts(
    pool: &PgPool,
    participant_addr: &str,
    key_prefix: Option<&str>,
    limit: i64,
) -> Result<Vec<ArchivalFactRow>, StorageError> {
    let rows = match key_prefix {
        Some(prefix) if !prefix.is_empty() => {
            let pattern = format!("{}%", prefix);
            sqlx::query_as::<_, ArchivalFactRow>(
                r#"
                SELECT fact_id, participant_addr, thread_id, key, value, tags, stored_by, stored_at
                FROM archival_facts
                WHERE participant_addr = $1 AND key LIKE $2
                ORDER BY stored_at DESC
                LIMIT $3
                "#,
            )
            .bind(participant_addr)
            .bind(&pattern)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        _ => {
            sqlx::query_as::<_, ArchivalFactRow>(
                r#"
                SELECT fact_id, participant_addr, thread_id, key, value, tags, stored_by, stored_at
                FROM archival_facts
                WHERE participant_addr = $1
                ORDER BY stored_at DESC
                LIMIT $2
                "#,
            )
            .bind(participant_addr)
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
    };

    Ok(rows)
}

/// Evict (delete) a fact by key for a participant.
pub async fn evict_fact(
    pool: &PgPool,
    participant_addr: &str,
    key: &str,
) -> Result<bool, StorageError> {
    let result = sqlx::query(
        "DELETE FROM archival_facts WHERE participant_addr = $1 AND key = $2",
    )
    .bind(participant_addr)
    .bind(key)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Get all facts for a participant (for context frame assembly).
pub async fn get_all_facts(
    pool: &PgPool,
    participant_addr: &str,
) -> Result<Vec<ArchivalFactRow>, StorageError> {
    let rows = sqlx::query_as::<_, ArchivalFactRow>(
        r#"
        SELECT fact_id, participant_addr, thread_id, key, value, tags, stored_by, stored_at
        FROM archival_facts
        WHERE participant_addr = $1
        ORDER BY stored_at DESC
        LIMIT 100
        "#,
    )
    .bind(participant_addr)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
