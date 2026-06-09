//! User blocks + content reports (App Store guideline 1.2 — UGC safety).
//!
//! Blocking suppresses delivery TO the blocker of messages FROM the blocked
//! address (the blocker stops seeing the blocked party's content). Reports are
//! persisted `open` for human review.

use sqlx::PgPool;
use uuid::Uuid;

/// Block `blocked` for `blocker`. Idempotent.
pub async fn block(pool: &PgPool, blocker: &str, blocked: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO user_blocks (blocker_addr, blocked_addr) VALUES ($1, $2) \
         ON CONFLICT DO NOTHING",
    )
    .bind(blocker)
    .bind(blocked)
    .execute(pool)
    .await?;
    Ok(())
}

/// Remove a block. Idempotent.
pub async fn unblock(pool: &PgPool, blocker: &str, blocked: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM user_blocks WHERE blocker_addr = $1 AND blocked_addr = $2")
        .bind(blocker)
        .bind(blocked)
        .execute(pool)
        .await?;
    Ok(())
}

/// Addresses `blocker` has blocked, oldest first.
pub async fn list_blocks(pool: &PgPool, blocker: &str) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT blocked_addr FROM user_blocks WHERE blocker_addr = $1 ORDER BY created_at",
    )
    .bind(blocker)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(a,)| a).collect())
}

/// Everyone who blocks `sender` — the delivery-time exclusion set.
pub async fn blockers_of(pool: &PgPool, sender: &str) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT blocker_addr FROM user_blocks WHERE blocked_addr = $1")
            .bind(sender)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|(a,)| a).collect())
}

/// File a content report; returns the report id. Status starts `open`.
pub async fn report(
    pool: &PgPool,
    reporter: &str,
    subject: &str,
    thread_id: Option<&str>,
    message_id: Option<&str>,
    reason: &str,
) -> Result<Uuid, sqlx::Error> {
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO content_reports (reporter_addr, subject_addr, thread_id, message_id, reason) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(reporter)
    .bind(subject)
    .bind(thread_id)
    .bind(message_id)
    .bind(reason)
    .fetch_one(pool)
    .await?;
    Ok(id)
}
