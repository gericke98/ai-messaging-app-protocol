# Account Auth Sessions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A DB-backed, revocable, per-device account **session token** (minted by signing a domain-separated challenge with the account's bound Ed25519 key) that authenticates the account-management REST endpoints — closing sub-project C's gap where those endpoints reused the dead 15-minute enrollment token.

**Architecture:** New `server/src/auth_session.rs` owns the `account_auth_tokens` repo + pure token-gen/hash + sliding-cap helpers + a testable `authenticate(pool, token, now) -> Option<Caller>` (validate-and-slide). `signing.rs` gains a domain-separated `verify_auth_challenge`. `server.rs` adds the mint + 3 session-management handlers and rewrites the thin `account_from_bearer` wrapper to call `auth_session::authenticate`. The HMAC `auth_token.rs` (enrollment/recovery) is untouched.

**Tech Stack:** Rust 2021, axum 0.8, sqlx 0.8 (Postgres), `ring` (SHA-256 digest + Ed25519 verify), `rand` (OsRng), `base64` URL-safe-no-pad, `chrono`. Integration tests run against a live Postgres via the existing `pool_or_skip` pattern (skip when no DB). Two known pre-existing test failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`) stay failing — unrelated.

---

## File Structure

| File | Responsibility | Change |
|------|----------------|--------|
| `server/migrations/006_account_auth_tokens.sql` | `account_auth_tokens` table + index | Create |
| `server/src/auth_session.rs` | repo + pure token/hash/expiry helpers + `authenticate` + `Caller`/`AuthSession` | Create |
| `server/src/signing.rs` | `verify_auth_challenge` + `sign_auth_challenge` test helper | Modify |
| `server/src/server.rs` | mint + list + revoke-one + revoke-others handlers; rewrite `account_from_bearer`; register routes on 3 routers | Modify |
| `server/src/lib.rs` | `pub mod auth_session;` | Modify |
| `server/src/main.rs` | `mod auth_session;` | Modify |
| `server/tests/auth_sessions_test.rs` | live-PG integration tests | Create |

---

## Task AT-1: Migration + `auth_session.rs` pure helpers

**Files:**
- Create: `server/migrations/006_account_auth_tokens.sql`
- Create: `server/src/auth_session.rs`
- Modify: `server/src/lib.rs`, `server/src/main.rs`

- [ ] **Step 1: Write the migration**

Create `server/migrations/006_account_auth_tokens.sql`:

```sql
-- Account auth sessions (sub-project: account-auth-sessions).
-- DB-backed, revocable, per-device session tokens that authenticate the
-- account-management REST endpoints. Plaintext tokens are NEVER stored; only a
-- SHA-256 hash. Closes the gap where those endpoints reused the enrollment token.
CREATE TABLE IF NOT EXISTS account_auth_tokens (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id   UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    token_hash   BYTEA NOT NULL UNIQUE,
    device_label TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at   TIMESTAMPTZ NOT NULL,
    revoked_at   TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_aat_account ON account_auth_tokens(account_id);
```

- [ ] **Step 2: Write the failing test for the pure helpers**

Create `server/src/auth_session.rs` with the pure helpers + their tests:

```rust
//! Account auth sessions: DB-backed, revocable, per-device session tokens.
//!
//! A session is minted by signing a domain-separated challenge with the account's
//! bound Ed25519 key (see `signing::verify_auth_challenge`) and authenticates the
//! account-management REST endpoints. Plaintext tokens are NEVER stored or logged —
//! only their SHA-256 hash. Sessions are individually revocable (Telegram-style) and
//! expire on a sliding window capped at an absolute ceiling.

use crate::error::StorageError;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;

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
        // 32 bytes URL-safe-base64-no-pad = 43 chars
        assert_eq!(t1.len(), 43);
        // hash of the returned plaintext equals the returned hash, and is deterministic
        assert_eq!(hash_token(&t1), h1);
        assert_eq!(hash_token(&t1), hash_token(&t1));
        assert_eq!(h1.len(), 32);
    }

    #[test]
    fn next_expiry_slides_then_clamps() {
        let created = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        // Early in life: now+90d is well under the 365d ceiling.
        let early = created + Duration::days(10);
        assert_eq!(next_expiry(created, early), early + Duration::days(90));
        // Near the ceiling: now+90d would exceed created+365d, so it clamps.
        let late = created + Duration::days(300);
        assert_eq!(next_expiry(created, late), created + Duration::days(365));
    }
}
```

- [ ] **Step 3: Register the module**

In `server/src/lib.rs` add (alphabetical, near `auth_token`): `pub mod auth_session;`
In `server/src/main.rs` add (matching its module list): `mod auth_session;`

- [ ] **Step 4: Run the pure tests**

Run: `cargo test -p ainm-server --lib auth_session::pure_tests`
Expected: PASS (2 tests). Confirms token entropy/length, hash determinism, sliding-cap math.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add server/migrations/006_account_auth_tokens.sql server/src/auth_session.rs server/src/lib.rs server/src/main.rs
rtk proxy git commit -m "feat(auth-sessions): migration 006 + auth_session pure helpers"
```

---

## Task AT-2: Domain-separated challenge in `signing.rs`

**Files:**
- Modify: `server/src/signing.rs`

The mint challenge must NOT reuse the handshake payload (`"{identity}|{timestamp}"`), so a captured handshake signature can't be replayed to mint a session. Add a sibling of `verify_init` with a distinct prefix.

- [ ] **Step 1: Write the failing test**

Add to `server/src/signing.rs` (a new `#[cfg(test)] mod auth_challenge_tests`):

```rust
#[cfg(test)]
mod auth_challenge_tests {
    use super::*;
    use crate::identity::generate_keypair;

    #[test]
    fn accepts_valid_rejects_tamper_stale_and_handshake_payload() {
        let id = generate_keypair("alice@test.local").unwrap();
        let (sig, ts) = sign_auth_challenge(&id.key_pair, "alice@test.local").unwrap();
        let now = chrono::DateTime::parse_from_rfc3339(&ts)
            .unwrap()
            .with_timezone(&chrono::Utc);

        // Valid
        assert!(verify_auth_challenge(&id.public_key_bytes, "alice@test.local", &ts, &sig, now, 300));
        // Tampered identity
        assert!(!verify_auth_challenge(&id.public_key_bytes, "mallory@test.local", &ts, &sig, now, 300));
        // Wrong key
        let other = generate_keypair("bob@test.local").unwrap();
        assert!(!verify_auth_challenge(&other.public_key_bytes, "alice@test.local", &ts, &sig, now, 300));
        // Stale timestamp
        let later = now + chrono::Duration::seconds(10_000);
        assert!(!verify_auth_challenge(&id.public_key_bytes, "alice@test.local", &ts, &sig, later, 300));
        // DOMAIN SEPARATION: a handshake-style signature (no "ainm-auth-token|" prefix)
        // must NOT verify as an auth challenge.
        let (handshake_sig, hts) = sign_init(&id.key_pair, "alice@test.local").unwrap();
        let hnow = chrono::DateTime::parse_from_rfc3339(&hts).unwrap().with_timezone(&chrono::Utc);
        assert!(!verify_auth_challenge(&id.public_key_bytes, "alice@test.local", &hts, &handshake_sig, hnow, 300));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p ainm-server --lib signing::auth_challenge_tests`
Expected: FAIL — `sign_auth_challenge`/`verify_auth_challenge` undefined.

- [ ] **Step 3: Implement the two functions**

Add to `server/src/signing.rs` (after `verify_init`). Note the existing imports already include `URL_SAFE_NO_PAD`, `Ed25519KeyPair`, `UnparsedPublicKey`, `ED25519`, and the crate uses `anyhow::Result`.

```rust
/// Domain-separation prefix for the auth-session mint challenge. Distinct from the
/// handshake payload (`"{identity}|{timestamp}"`) so signatures cannot be replayed
/// across the two protocols.
const AUTH_CHALLENGE_DOMAIN: &str = "ainm-auth-token";

/// Sign the auth-session mint challenge: `"ainm-auth-token|{identity}|{timestamp}"`.
/// Returns `(signature_b64url, timestamp_rfc3339)`. Test/reference helper mirroring `sign_init`.
pub fn sign_auth_challenge(key_pair: &Ed25519KeyPair, identity: &str) -> Result<(String, String)> {
    let timestamp = crate::protocol::now_rfc3339();
    let payload = format!("{AUTH_CHALLENGE_DOMAIN}|{identity}|{timestamp}");
    let sig = key_pair.sign(payload.as_bytes());
    Ok((URL_SAFE_NO_PAD.encode(sig.as_ref()), timestamp))
}

/// Verify an auth-session mint challenge signature against a known public key.
/// Reconstructs `"ainm-auth-token|{identity}|{timestamp}"`, checks the Ed25519
/// signature, and rejects timestamps outside +/- `max_skew_secs`.
pub fn verify_auth_challenge(
    public_key_bytes: &[u8],
    identity: &str,
    timestamp: &str,
    signature_b64: &str,
    now: chrono::DateTime<chrono::Utc>,
    max_skew_secs: i64,
) -> bool {
    let Ok(ts) = chrono::DateTime::parse_from_rfc3339(timestamp) else { return false; };
    let skew = (now - ts.with_timezone(&chrono::Utc)).num_seconds().abs();
    if skew > max_skew_secs { return false; }

    let Ok(sig_bytes) = URL_SAFE_NO_PAD.decode(signature_b64) else { return false; };
    let payload = format!("{AUTH_CHALLENGE_DOMAIN}|{identity}|{timestamp}");
    let public_key = UnparsedPublicKey::new(&ED25519, public_key_bytes);
    public_key.verify(payload.as_bytes(), &sig_bytes).is_ok()
}
```

(If `sign_init` uses a slightly different signing call or `now_rfc3339` path, match it exactly — read `sign_init` just above and mirror its style. `Ed25519KeyPair::sign` is correct per `sign_init`.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p ainm-server --lib signing::`
Expected: PASS (the new `auth_challenge_tests` + the existing `verify_init_tests`).

- [ ] **Step 5: Commit**

```bash
rtk proxy git add server/src/signing.rs
rtk proxy git commit -m "feat(auth-sessions): domain-separated verify_auth_challenge"
```

---

## Task AT-3: `auth_session.rs` DB repo + `authenticate`

**Files:**
- Modify: `server/src/auth_session.rs`
- Create: `server/tests/auth_sessions_test.rs` (the repo-level integration tests; extended in AT-5)

This adds the DB layer. The per-request validation+slide lives in `authenticate` so it is testable without booting axum (the HTTP `account_from_bearer` in AT-4 is a thin wrapper over it).

- [ ] **Step 1: Implement the repo + `authenticate`**

Append to `server/src/auth_session.rs`. Match the existing sqlx style in `accounts.rs`/`credentials.rs` (e.g. `sqlx::query_as`/`query`/`query_scalar`, `.fetch_optional`, mapping errors to `StorageError`). Read `accounts.rs::find_active_key` for the row-mapping idiom before writing.

```rust
/// Row → AuthSession mapper (used by the queries below).
fn map_row(r: &sqlx::postgres::PgRow) -> AuthSession {
    use sqlx::Row;
    AuthSession {
        id: r.get("id"),
        account_id: r.get("account_id"),
        device_label: r.get("device_label"),
        created_at: r.get("created_at"),
        last_used_at: r.get("last_used_at"),
        expires_at: r.get("expires_at"),
        revoked_at: r.get("revoked_at"),
    }
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
    .await
    .map_err(StorageError::from)?;
    Ok(id)
}

/// Look up an ACTIVE (not revoked, not expired) session by token hash.
pub async fn find_active_by_hash(
    pool: &PgPool,
    token_hash: &[u8],
    now: DateTime<Utc>,
) -> Result<Option<AuthSession>, StorageError> {
    let row = sqlx::query(
        "SELECT id, account_id, device_label, created_at, last_used_at, expires_at, revoked_at \
         FROM account_auth_tokens \
         WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > $2",
    )
    .bind(token_hash)
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(StorageError::from)?;
    Ok(row.as_ref().map(map_row))
}

/// Slide a session forward on use: bump `last_used_at` and `expires_at`.
pub async fn slide(
    pool: &PgPool,
    id: uuid::Uuid,
    now: DateTime<Utc>,
    new_expires: DateTime<Utc>,
) -> Result<(), StorageError> {
    sqlx::query(
        "UPDATE account_auth_tokens SET last_used_at = $2, expires_at = $3 WHERE id = $1",
    )
    .bind(id)
    .bind(now)
    .bind(new_expires)
    .execute(pool)
    .await
    .map_err(StorageError::from)?;
    Ok(())
}

/// Validate a presented bearer token and slide it forward. Returns the `Caller`
/// (account_id + token_hash) iff the session is active. This is the load-bearing
/// per-request auth used by `account_from_bearer`.
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
    let rows = sqlx::query(
        "SELECT id, account_id, device_label, created_at, last_used_at, expires_at, revoked_at \
         FROM account_auth_tokens \
         WHERE account_id = $1 AND revoked_at IS NULL AND expires_at > $2 \
         ORDER BY last_used_at DESC",
    )
    .bind(account_id)
    .bind(now)
    .fetch_all(pool)
    .await
    .map_err(StorageError::from)?;
    Ok(rows.iter().map(map_row).collect())
}

/// Revoke one session owned by `account_id`. Returns true if a matching row was
/// updated (idempotent: already-revoked owned rows still return true if matched).
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
    .await
    .map_err(StorageError::from)?;
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
    .await
    .map_err(StorageError::from)?;
    Ok(res.rows_affected())
}
```

Notes for the engineer:
- Verify `StorageError` has a `From<sqlx::Error>` impl (used across `accounts.rs`/`credentials.rs`). If the idiom there is `.map_err(|e| StorageError::Db(e.to_string()))` or similar, match it exactly rather than `StorageError::from`.
- `chrono::DateTime<Utc>` binds/decodes against `TIMESTAMPTZ` with sqlx's `chrono` feature (already enabled — `accounts.rs` uses it). Confirm the row getter types compile.

- [ ] **Step 2: Write repo integration tests (live PG)**

Create `server/tests/auth_sessions_test.rs`:

```rust
//! Integration tests for account auth sessions (live Postgres; skips without a DB).

use ainm_server::auth_session;
use chrono::{Duration, Utc};

async fn pool_or_skip() -> Option<sqlx::PgPool> {
    let url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("AINM_DATABASE__URL"))
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()?;
    let pool = sqlx::PgPool::connect(&url).await.ok()?;
    let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    sqlx::migrate::Migrator::new(migrations_dir).await.ok()?.run(&pool).await.ok()?;
    Some(pool)
}

async fn make_account(pool: &sqlx::PgPool, tag: &str) -> uuid::Uuid {
    let suffix = format!("{:x}", uuid::Uuid::new_v4().as_u128());
    let username = format!("as-{tag}-{suffix}");
    let address = format!("{username}@ainm.local");
    sqlx::query_scalar(
        "INSERT INTO accounts (username, domain, address, email_hash, email_verified, status) \
         VALUES ($1, 'ainm.local', $2, $3, true, 'active') RETURNING id",
    )
    .bind(&username).bind(&address).bind(format!("hash-{address}"))
    .fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn mint_use_list_then_revoke_cycle() {
    let Some(pool) = pool_or_skip().await else { eprintln!("no DB; skipping"); return; };
    let account_id = make_account(&pool, "cycle").await;
    let now = Utc::now();

    // Mint: generate + insert (mirrors the handler).
    let (token, hash) = auth_session::generate_token();
    let expires = auth_session::next_expiry(now, now);
    let session_id = auth_session::insert(&pool, account_id, &hash, Some("iPhone"), now, expires).await.unwrap();

    // Authenticate the token → Caller with the right account + hash.
    let caller = auth_session::authenticate(&pool, &token, now).await.unwrap().expect("active");
    assert_eq!(caller.account_id, account_id);
    assert_eq!(caller.token_hash, hash);

    // List shows exactly one active session.
    let sessions = auth_session::list_active(&pool, account_id, now).await.unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, session_id);
    assert_eq!(sessions[0].device_label.as_deref(), Some("iPhone"));

    // Revoke it → authenticate now returns None.
    assert!(auth_session::revoke_one(&pool, account_id, session_id, now).await.unwrap());
    assert!(auth_session::authenticate(&pool, &token, now).await.unwrap().is_none());
}

#[tokio::test]
async fn expired_session_is_rejected() {
    let Some(pool) = pool_or_skip().await else { eprintln!("no DB; skipping"); return; };
    let account_id = make_account(&pool, "expired").await;
    let now = Utc::now();
    let (token, hash) = auth_session::generate_token();
    // Insert already-expired.
    let past = now - Duration::days(1);
    auth_session::insert(&pool, account_id, &hash, None, now - Duration::days(100), past).await.unwrap();
    assert!(auth_session::authenticate(&pool, &token, now).await.unwrap().is_none());
}

#[tokio::test]
async fn revoke_others_keeps_current() {
    let Some(pool) = pool_or_skip().await else { eprintln!("no DB; skipping"); return; };
    let account_id = make_account(&pool, "others").await;
    let now = Utc::now();
    let (tok_a, hash_a) = auth_session::generate_token();
    let (tok_b, hash_b) = auth_session::generate_token();
    let exp = auth_session::next_expiry(now, now);
    auth_session::insert(&pool, account_id, &hash_a, Some("A"), now, exp).await.unwrap();
    auth_session::insert(&pool, account_id, &hash_b, Some("B"), now, exp).await.unwrap();

    // From session A, revoke others → B dies, A lives.
    let revoked = auth_session::revoke_others(&pool, account_id, &hash_a, now).await.unwrap();
    assert_eq!(revoked, 1);
    assert!(auth_session::authenticate(&pool, &tok_a, now).await.unwrap().is_some());
    assert!(auth_session::authenticate(&pool, &tok_b, now).await.unwrap().is_none());
}

#[tokio::test]
async fn revoke_one_is_account_scoped() {
    let Some(pool) = pool_or_skip().await else { eprintln!("no DB; skipping"); return; };
    let victim = make_account(&pool, "victim").await;
    let attacker = make_account(&pool, "attacker").await;
    let now = Utc::now();
    let (_tok, hash) = auth_session::generate_token();
    let exp = auth_session::next_expiry(now, now);
    let sid = auth_session::insert(&pool, victim, &hash, None, now, exp).await.unwrap();
    // Attacker cannot revoke the victim's session id.
    assert!(!auth_session::revoke_one(&pool, attacker, sid, now).await.unwrap());
}
```

- [ ] **Step 3: Run the integration tests**

Run: `cargo test -p ainm-server --test auth_sessions_test -- --nocapture`
Expected: PASS if a Postgres is reachable (`TEST_DATABASE_URL`/`AINM_DATABASE__URL`/`DATABASE_URL`); otherwise each test prints "no DB; skipping" and passes. Report which path ran.

- [ ] **Step 4: Commit**

```bash
rtk proxy git add server/src/auth_session.rs server/tests/auth_sessions_test.rs
rtk proxy git commit -m "feat(auth-sessions): DB repo + authenticate + integration tests"
```

---

## Task AT-4: HTTP handlers + rewrite `account_from_bearer` + routes

**Files:**
- Modify: `server/src/server.rs`

This wires the mint + 3 session-management handlers and replaces the enrollment-token bearer check with the session-token one. Read `server/src/server.rs:356-419` (the current `account_from_bearer` + the C handlers) and `server/src/server.rs:937-947` (the handshake key-decode + verify pattern) before editing.

- [ ] **Step 1: Rewrite `account_from_bearer` over `auth_session::authenticate`**

Replace the current `account_from_bearer` (server.rs:356-368) with a token-extractor + a thin wrapper. The extractor returns the full `Caller`; the existing C endpoints keep using the `account_id`-only form.

```rust
/// Resolve the calling account + presented-token hash from the `Authorization: Bearer
/// <session-token>` header, validating the DB-backed session (reject revoked/expired,
/// slide on use). Replaces the old enrollment-token check.
async fn caller_from_bearer(
    state: &ServerState,
    headers: &HeaderMap,
) -> Result<crate::auth_session::Caller, (StatusCode, String)> {
    let pool = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    let raw = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::UNAUTHORIZED, "missing Authorization header".into()))?;
    let token = raw.strip_prefix("Bearer ").ok_or((StatusCode::UNAUTHORIZED, "expected Bearer token".into()))?;
    match crate::auth_session::authenticate(pool, token, chrono::Utc::now()).await {
        Ok(Some(caller)) => Ok(caller),
        Ok(None) => Err((StatusCode::UNAUTHORIZED, "invalid session".into())),
        Err(_) => Err((StatusCode::INTERNAL_SERVER_ERROR, "session lookup failed".into())),
    }
}

/// Account-id-only convenience for endpoints that don't need the token hash.
async fn account_from_bearer(
    state: &ServerState,
    headers: &HeaderMap,
) -> Result<uuid::Uuid, (StatusCode, String)> {
    Ok(caller_from_bearer(state, headers).await?.account_id)
}
```

IMPORTANT: `account_from_bearer` was a **sync** fn; it's now **async**. Update its three call sites (`llm_credential_put_handler`, `llm_credential_delete_handler`, `usage_get_handler` at server.rs:382/399/411) to `account_from_bearer(&state, &headers).await?`.

- [ ] **Step 2: Add the mint handler**

Add near the other account handlers in `server.rs`:

```rust
#[derive(serde::Deserialize)]
pub struct MintTokenReq {
    pub identity: String,
    pub timestamp: String,
    pub signature: String,
    pub device_label: Option<String>,
}
#[derive(serde::Serialize)]
pub struct MintTokenResp {
    pub token: String,
    pub session_id: uuid::Uuid,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// `POST /accounts/auth/token` — mint a session by proving possession of the account's
/// bound Ed25519 key (domain-separated challenge). Uniform 401 on any auth failure.
pub async fn auth_token_mint_handler(
    State(state): State<ServerState>,
    Json(req): Json<MintTokenReq>,
) -> Result<Json<MintTokenResp>, (StatusCode, String)> {
    let pool = state.db.as_ref().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    let unauthorized = || (StatusCode::UNAUTHORIZED, "authentication failed".to_string());

    // Resolve account + its active bound key (uniform 401 — no enumeration).
    let account = crate::accounts::find_account_by_address(pool, &req.identity)
        .await.map_err(|_| unauthorized())?.ok_or_else(unauthorized)?;
    let key = crate::accounts::find_active_key(pool, account.id)
        .await.map_err(|_| unauthorized())?.ok_or_else(unauthorized)?;
    let pk_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&key.public_key).map_err(|_| unauthorized())?;

    if !crate::signing::verify_auth_challenge(
        &pk_bytes, &req.identity, &req.timestamp, &req.signature, chrono::Utc::now(), 300,
    ) {
        return Err(unauthorized());
    }

    let now = chrono::Utc::now();
    let (token, hash) = crate::auth_session::generate_token();
    let expires_at = crate::auth_session::next_expiry(now, now);
    let session_id = crate::auth_session::insert(
        pool, account.id, &hash, req.device_label.as_deref(), now, expires_at,
    ).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(MintTokenResp { token, session_id, expires_at }))
}
```

(`base64::Engine` is already `use`d at server.rs:15. Confirm `find_account_by_address`/`find_active_key` return types — `account.id`, `key.public_key` — match `accounts.rs`.)

- [ ] **Step 3: Add the session-management handlers**

```rust
#[derive(serde::Serialize)]
pub struct SessionView {
    pub id: uuid::Uuid,
    pub device_label: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub current: bool,
}

/// `GET /accounts/auth/sessions` — list the caller's active sessions (current flagged).
pub async fn sessions_list_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SessionView>>, (StatusCode, String)> {
    let caller = caller_from_bearer(&state, &headers).await?;
    let pool = state.db.clone().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    let now = chrono::Utc::now();
    let sessions = crate::auth_session::list_active(&pool, caller.account_id, now)
        .await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // `list_active` rows don't carry the token hash, so resolve the current session's id
    // by looking it up from the caller's presented-token hash, then flag the matching row.
    let current_id = crate::auth_session::find_active_by_hash(&pool, &caller.token_hash, now)
        .await.ok().flatten().map(|s| s.id);
    let views = sessions.into_iter().map(|s| SessionView {
        current: Some(s.id) == current_id,
        id: s.id,
        device_label: s.device_label,
        created_at: s.created_at,
        last_used_at: s.last_used_at,
        expires_at: s.expires_at,
    }).collect();
    Ok(Json(views))
}

/// `DELETE /accounts/auth/sessions/{id}` — revoke one owned session.
pub async fn session_revoke_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Path(session_id): Path<uuid::Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    let caller = caller_from_bearer(&state, &headers).await?;
    let pool = state.db.clone().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    let ok = crate::auth_session::revoke_one(&pool, caller.account_id, session_id, chrono::Utc::now())
        .await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if ok { Ok(StatusCode::NO_CONTENT) } else { Err((StatusCode::NOT_FOUND, "no such session".into())) }
}

#[derive(serde::Serialize)]
pub struct RevokeOthersResp { pub revoked: u64 }

/// `DELETE /accounts/auth/sessions/others` — revoke all the caller's sessions but the current.
pub async fn sessions_revoke_others_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<RevokeOthersResp>, (StatusCode, String)> {
    let caller = caller_from_bearer(&state, &headers).await?;
    let pool = state.db.clone().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    let revoked = crate::auth_session::revoke_others(&pool, caller.account_id, &caller.token_hash, chrono::Utc::now())
        .await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(RevokeOthersResp { revoked }))
}
```

- [ ] **Step 4: Register routes on all three routers**

In each of the three router builders (`start` ~server.rs:642, `start_with_port` ~709, `start_federation` ~785), add these routes alongside the existing `/accounts/llm-credential` + `/accounts/usage` lines. **Register the static `/others` path BEFORE the `{id}` path** so axum doesn't capture `others` as an id:

```rust
        .route("/accounts/auth/token", axum::routing::post(auth_token_mint_handler))
        .route("/accounts/auth/sessions", axum::routing::get(sessions_list_handler))
        .route("/accounts/auth/sessions/others", axum::routing::delete(sessions_revoke_others_handler))
        .route("/accounts/auth/sessions/{id}", axum::routing::delete(session_revoke_handler))
```

(axum 0.8 path-param syntax is `{id}` as used by the existing `/.well-known/ainm-identity/{username}` route. In axum 0.8 static segments take precedence over captures, but registering `/others` first is belt-and-suspenders + clearer.)

- [ ] **Step 5: Build + verify compilation across the crate**

Run: `cargo build -p ainm-server`
Expected: clean. Likely fixups: the three C-endpoint call sites now need `.await` on `account_from_bearer`; ensure `Path` is imported (it is — used by `well_known_identity_handler`); ensure `caller_from_bearer`/`account_from_bearer` are reachable from the handlers.

- [ ] **Step 6: Clippy**

Run: `cargo clippy -p ainm-server --lib --tests 2>&1 | grep -E "server.rs|auth_session.rs" || echo "clean"`
Expected: no new warnings in the touched files.

- [ ] **Step 7: Commit**

```bash
rtk proxy git add server/src/server.rs
rtk proxy git commit -m "feat(auth-sessions): mint + session-management handlers; session-token account_from_bearer"
```

---

## Task AT-5: End-to-end integration test + full suite + DoD

**Files:**
- Modify: `server/tests/auth_sessions_test.rs`

- [ ] **Step 1: Add the end-to-end mint→authenticate test that proves the C gap is closed**

This test exercises the full library path the mint handler uses (resolve key → verify challenge → insert → authenticate), proving a session minted from a signature is usable — and that an enrollment-style token (any non-session bearer) is rejected. Append to `server/tests/auth_sessions_test.rs`:

```rust
use ainm_server::{identity, signing};

/// Bind a real Ed25519 key to an account (mirrors what the bind-key endpoint stores:
/// URL-safe-base64-no-pad of the public key bytes).
async fn bind_key(pool: &sqlx::PgPool, account_id: uuid::Uuid) -> identity::Identity {
    use base64::Engine;
    let id = identity::generate_keypair("unused@ainm.local").unwrap();
    let pk_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&id.public_key_bytes);
    sqlx::query("INSERT INTO account_keys (account_id, public_key) VALUES ($1, $2)")
        .bind(account_id).bind(&pk_b64).execute(pool).await.unwrap();
    id
}

#[tokio::test]
async fn mint_via_signature_then_authenticate() {
    let Some(pool) = pool_or_skip().await else { eprintln!("no DB; skipping"); return; };
    // Build an account WITH a known address + bound key.
    let suffix = format!("{:x}", uuid::Uuid::new_v4().as_u128());
    let address = format!("as-mint-{suffix}@ainm.local");
    let account_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (username, domain, address, email_hash, email_verified, status) \
         VALUES ($1, 'ainm.local', $2, $3, true, 'active') RETURNING id",
    ).bind(format!("as-mint-{suffix}")).bind(&address).bind(format!("hash-{address}"))
    .fetch_one(&pool).await.unwrap();
    let id = bind_key(&pool, account_id).await;

    // Client signs the domain-separated challenge.
    let (sig, ts) = signing::sign_auth_challenge(&id.key_pair, &address).unwrap();
    let now = chrono::DateTime::parse_from_rfc3339(&ts).unwrap().with_timezone(&chrono::Utc);

    // Server-side verify (what the mint handler does) must accept it.
    let pk = id.public_key_bytes.clone();
    assert!(signing::verify_auth_challenge(&pk, &address, &ts, &sig, now, 300));

    // Mint + authenticate round-trip.
    let (token, hash) = auth_session::generate_token();
    let exp = auth_session::next_expiry(now, now);
    auth_session::insert(&pool, account_id, &hash, Some("dev"), now, exp).await.unwrap();
    let caller = auth_session::authenticate(&pool, &token, now).await.unwrap().expect("active");
    assert_eq!(caller.account_id, account_id);

    // An arbitrary non-session bearer (e.g. a stale enrollment token string) is rejected.
    assert!(auth_session::authenticate(&pool, "not-a-real-session-token", now).await.unwrap().is_none());
}
```

(Confirm `identity::Identity` field names — `key_pair`, `public_key_bytes` — match `identity.rs`/`generate_keypair`; adjust if the struct differs.)

- [ ] **Step 2: Run the auth-session integration tests**

Run: `cargo test -p ainm-server --test auth_sessions_test -- --nocapture`
Expected: all PASS (or uniform "no DB; skipping"). Report which path ran.

- [ ] **Step 3: Run the full server suite**

Run: `cargo test -p ainm-server 2>&1 | tail -40`
Expected: green EXCEPT the two known pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`). Name any other failure and classify it (regression / known / needs-DB-env).

- [ ] **Step 4: Clippy**

Run: `cargo clippy -p ainm-server --lib --tests 2>&1 | grep -E "auth_session.rs|signing.rs|server.rs|auth_sessions_test" || echo "no warnings in touched files"`
Expected: no warnings in the files this plan touched. (Pre-existing repo-wide `-D warnings` debt in unrelated files is out of scope — confirm none of the warnings are in touched files.)

- [ ] **Step 5: Verify DoD against the spec**

Re-read `docs/superpowers/specs/2026-06-02-account-auth-sessions-design.md` §10 and confirm each of the 8 items in code. In particular confirm: plaintext token + hash never logged (grep the touched files for `token`/`hash` in `tracing::`/`println!`/`format!`-to-log); enrollment tokens no longer authenticate the C endpoints (the rewrite routes through `auth_session::authenticate`, which only accepts session tokens stored in the table).

```bash
rtk proxy grep -n "tracing::\|println!\|eprintln!" server/src/auth_session.rs server/src/server.rs | grep -i "token\|hash" || echo "OK: no token/hash logging"
```

- [ ] **Step 6: Final commit**

```bash
rtk proxy git add server/tests/auth_sessions_test.rs
rtk proxy git commit -m "test(auth-sessions): mint-via-signature E2E + DoD verification"
```

---

## Self-Review (completed during planning)

- **Spec coverage:** §2 table → AT-1 migration; §3 token/hash → AT-1 helpers; §4 mint + `verify_auth_challenge` → AT-2 (challenge) + AT-4 (handler); §5 `account_from_bearer` rewrite + `authenticate` → AT-3 (`authenticate`) + AT-4 (wrapper); §6 session endpoints → AT-4; §7 module boundaries → AT-1/AT-3/AT-4; §8 errors → AT-4 (uniform 401s); §9 tests → AT-1/AT-3/AT-5; §10 DoD → AT-5. All covered.
- **Placeholder scan:** no TBD/TODO; every code step has complete code.
- **Type consistency:** `generate_token() -> (String, Vec<u8>)`, `hash_token(&str) -> Vec<u8>`, `next_expiry(created, now)`, `authenticate(pool, &str, now) -> Result<Option<Caller>>`, `Caller { account_id, token_hash }`, `insert(pool, account_id, &hash, label, created, expires)`, `revoke_one/revoke_others` signatures are referenced identically across AT-3/AT-4/AT-5.
- **Known risk flagged inline:** `StorageError` From-impl idiom (AT-3 note) and `identity::Identity` field names (AT-5 note) are the two spots the engineer must confirm against existing code; both are mechanical.
- **`account_from_bearer` sync→async** is called out explicitly in AT-4 Step 1 with the three call sites to update — the one cross-cutting change most likely to break the build if missed.
