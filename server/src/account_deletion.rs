//! Account deletion (M7 / App Store guideline 5.1.1).
//!
//! One transaction removes the account row — FK CASCADE takes the keys, escrow,
//! auth sessions, and LLM settings with it — plus every table keyed by the
//! account's ADDRESS rather than its id (those have no FK to cascade from):
//! memory graph, grants, briefing log, compilation passes, archival facts, the
//! caller's own blocks, usage meters, and email verifications.
//!
//! Deliberately retained: `messages` (they belong to shared conversations other
//! participants still see) and `content_reports` (the operator's moderation
//! record must survive the reporter or subject leaving).

use sqlx::PgPool;

/// Delete `account_id` and all of its personal data. No-op for an unknown id.
pub async fn delete_account(pool: &PgPool, account_id: uuid::Uuid) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    // Resolve the address + email hash before the row disappears.
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT address, email_hash FROM accounts WHERE id = $1")
            .bind(account_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((address, email_hash)) = row else {
        return Ok(());
    };

    // Address-scoped personal data (no FK to accounts).
    // memory_edges cascade from memory_nodes, but delete by user_addr first so
    // cross-user edges referencing this user's nodes can't strand a row.
    for sql in [
        "DELETE FROM memory_edges WHERE user_addr = $1",
        "DELETE FROM memory_nodes WHERE user_addr = $1",
        "DELETE FROM compilation_passes WHERE user_addr = $1",
        "DELETE FROM briefing_log WHERE user_addr = $1",
        "DELETE FROM memory_grants WHERE user_addr = $1",
        "DELETE FROM archival_facts WHERE participant_addr = $1",
        "DELETE FROM user_blocks WHERE blocker_addr = $1",
        "DELETE FROM llm_usage WHERE requester_address = $1",
    ] {
        sqlx::query(sql).bind(&address).execute(&mut *tx).await?;
    }
    sqlx::query("DELETE FROM email_verifications WHERE email_hash = $1")
        .bind(&email_hash)
        .execute(&mut *tx)
        .await?;

    // The account row last: cascades account_keys, key_escrow,
    // account_auth_tokens, and account_llm_settings.
    sqlx::query("DELETE FROM accounts WHERE id = $1")
        .bind(account_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await
}
