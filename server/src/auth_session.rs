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

use chrono::{DateTime, Duration, Utc};

/// Sliding TTL: each authenticated use extends `expires_at` to `now + 90d`...
pub const SESSION_TTL_DAYS: i64 = 90;
/// ...but never past `created_at + 365d`.
pub const SESSION_ABSOLUTE_CAP_DAYS: i64 = 365;

/// A stored session row (never carries the plaintext token).
#[derive(Debug, Clone)]
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
