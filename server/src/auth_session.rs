//! Account auth sessions: DB-backed, revocable, per-device session tokens.
//!
//! A session is minted by signing a domain-separated challenge with the account's
//! bound Ed25519 key (see `signing::verify_auth_challenge`) and authenticates the
//! account-management REST endpoints. Plaintext tokens are NEVER stored or logged —
//! only their SHA-256 hash. Sessions are individually revocable (Telegram-style) and
//! expire on a sliding window capped at an absolute ceiling.
//!
//! AT-1 lands only the migration + pure helpers/structs; the DB repo (AT-3),
//! signing challenge (AT-2), and HTTP wiring (AT-4) consume these. Until then the
//! public items are unreferenced from the binary target, so suppress dead_code at
//! the module boundary rather than per-item.
#![allow(dead_code)]

use crate::error::StorageError;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;

/// Sliding TTL: each authenticated use extends `expires_at` to `now + 90d`...
pub const SESSION_TTL_DAYS: i64 = 90;
/// ...but never past `created_at + 365d`.
pub const SESSION_ABSOLUTE_CAP_DAYS: i64 = 365;

/// A stored session row (never carries the plaintext token).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuthSession {
    pub id: uuid::Uuid,
    pub account_id: uuid::Uuid,
    pub device_label: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// The authenticated caller resolved from a bearer session token.
#[derive(Debug, Clone)]
pub struct Caller {
    pub account_id: uuid::Uuid,
    /// SHA-256 of the presented token — lets session-management endpoints flag the
    /// *current* session and *keep* it during revoke-others.
    pub token_hash: Vec<u8>,
}

/// Generate a fresh opaque session token. Returns `(plaintext, hash)`; the plaintext
/// is shown to the client exactly once and never persisted.
pub fn generate_token() -> (String, Vec<u8>) {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let plaintext = URL_SAFE_NO_PAD.encode(bytes);
    let hash = hash_token(&plaintext);
    (plaintext, hash)
}

/// SHA-256 of a token string (the value stored in `token_hash`).
pub fn hash_token(token: &str) -> Vec<u8> {
    ring::digest::digest(&ring::digest::SHA256, token.as_bytes())
        .as_ref()
        .to_vec()
}

/// Sliding expiry with absolute cap: `min(now + 90d, created_at + 365d)`.
pub fn next_expiry(created_at: DateTime<Utc>, now: DateTime<Utc>) -> DateTime<Utc> {
    let sliding = now + Duration::days(SESSION_TTL_DAYS);
    let ceiling = created_at + Duration::days(SESSION_ABSOLUTE_CAP_DAYS);
    sliding.min(ceiling)
}

/// Insert a new session. `expires_at` is computed by the caller (mint = now + 90d).
pub async fn insert(
    pool: &PgPool,
    account_id: uuid::Uuid,
    token_hash: &[u8],
    device_label: Option<&str>,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
) -> Result<uuid::Uuid, StorageError> {
    let id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO account_auth_tokens \
         (account_id, token_hash, device_label, created_at, last_used_at, expires_at) \
         VALUES ($1, $2, $3, $4, $4, $5) RETURNING id",
    )
    .bind(account_id)
    .bind(token_hash)
    .bind(device_label)
    .bind(created_at)
    .bind(expires_at)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Look up an ACTIVE (not revoked, not expired) session by token hash.
pub async fn find_active_by_hash(
    pool: &PgPool,
    token_hash: &[u8],
    now: DateTime<Utc>,
) -> Result<Option<AuthSession>, StorageError> {
    let row = sqlx::query_as::<_, AuthSession>(
        "SELECT id, account_id, device_label, created_at, last_used_at, expires_at, revoked_at \
         FROM account_auth_tokens \
         WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > $2",
    )
    .bind(token_hash)
    .bind(now)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Slide a session forward on use: bump `last_used_at` and `expires_at`.
pub async fn slide(
    pool: &PgPool,
    id: uuid::Uuid,
    now: DateTime<Utc>,
    new_expires: DateTime<Utc>,
) -> Result<(), StorageError> {
    sqlx::query("UPDATE account_auth_tokens SET last_used_at = $2, expires_at = $3 WHERE id = $1")
        .bind(id)
        .bind(now)
        .bind(new_expires)
        .execute(pool)
        .await?;
    Ok(())
}

/// Validate a presented bearer token and slide it forward. Returns the `Caller`
/// (account_id + token_hash) iff the session is active. The per-request auth path.
pub async fn authenticate(
    pool: &PgPool,
    presented_token: &str,
    now: DateTime<Utc>,
) -> Result<Option<Caller>, StorageError> {
    let hash = hash_token(presented_token);
    let Some(session) = find_active_by_hash(pool, &hash, now).await? else {
        return Ok(None);
    };
    let new_expires = next_expiry(session.created_at, now);
    slide(pool, session.id, now, new_expires).await?;
    Ok(Some(Caller { account_id: session.account_id, token_hash: hash }))
}

/// List a caller's ACTIVE sessions, newest first.
pub async fn list_active(
    pool: &PgPool,
    account_id: uuid::Uuid,
    now: DateTime<Utc>,
) -> Result<Vec<AuthSession>, StorageError> {
    let rows = sqlx::query_as::<_, AuthSession>(
        "SELECT id, account_id, device_label, created_at, last_used_at, expires_at, revoked_at \
         FROM account_auth_tokens \
         WHERE account_id = $1 AND revoked_at IS NULL AND expires_at > $2 \
         ORDER BY last_used_at DESC",
    )
    .bind(account_id)
    .bind(now)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Revoke one session owned by `account_id`. Returns true if a matching row was updated.
pub async fn revoke_one(
    pool: &PgPool,
    account_id: uuid::Uuid,
    id: uuid::Uuid,
    now: DateTime<Utc>,
) -> Result<bool, StorageError> {
    let res = sqlx::query(
        "UPDATE account_auth_tokens SET revoked_at = COALESCE(revoked_at, $3) \
         WHERE id = $1 AND account_id = $2",
    )
    .bind(id)
    .bind(account_id)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

/// Revoke all of the caller's active sessions EXCEPT the one whose hash is `keep_hash`.
/// Returns the number revoked.
pub async fn revoke_others(
    pool: &PgPool,
    account_id: uuid::Uuid,
    keep_hash: &[u8],
    now: DateTime<Utc>,
) -> Result<u64, StorageError> {
    let res = sqlx::query(
        "UPDATE account_auth_tokens SET revoked_at = $3 \
         WHERE account_id = $1 AND token_hash <> $2 AND revoked_at IS NULL",
    )
    .bind(account_id)
    .bind(keep_hash)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

#[cfg(test)]
mod pure_tests {
    use super::*;

    #[test]
    fn generate_token_is_random_and_hash_matches() {
        let (t1, h1) = generate_token();
        let (t2, _h2) = generate_token();
        assert_ne!(t1, t2, "tokens must differ");
        assert_eq!(t1.len(), 43); // 32 bytes URL-safe-base64-no-pad
        assert_eq!(hash_token(&t1), h1);
        assert_eq!(hash_token(&t1), hash_token(&t1));
        assert_eq!(h1.len(), 32);
    }

    #[test]
    fn next_expiry_slides_then_clamps() {
        let created = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let early = created + Duration::days(10);
        assert_eq!(next_expiry(created, early), early + Duration::days(90));
        let late = created + Duration::days(300);
        assert_eq!(next_expiry(created, late), created + Duration::days(365));
    }
}
