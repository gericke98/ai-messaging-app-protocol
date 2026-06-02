# Accounts & Identity Binding (Server) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give AINM real, non-spoofable user accounts (email-code signup → handle↔key binding → anti-spoof handshake enforcement → PIN + zero-knowledge OPRF key-escrow recovery), all on the Rust server and fully covered by `cargo test`.

**Architecture:** A new `003_accounts.sql` migration adds `accounts`, `account_keys`, `email_verifications`, `key_escrow`. New server modules — `accounts.rs` (lifecycle + validation + repository queries), `email.rs` (`EmailSender` trait + mock, mirroring `LlmClient`), `auth_token.rs` (stateless HMAC enrollment/recovery tokens), `escrow.rs` (RFC 9497 OPRF(ristretto255, SHA-512) via the `voprf` crate + Argon2id + HKDF + XChaCha20-Poly1305) — are wired into the Axum router as REST endpoints, and the WebSocket `ainm.initialize` path is upgraded to verify the client signature against the **registered** `account_keys` row instead of trusting the claimed identity.

**Tech Stack:** Rust, Axum 0.8, sqlx 0.8 (Postgres), ring 0.17 (Ed25519 + HMAC), `voprf` 0.5 (Ristretto255 OPRF), `argon2` 0.5, `hkdf` 0.12 + `sha2` 0.10, `chacha20poly1305` 0.10 (XChaCha20-Poly1305), `serde_json`, `chrono`, `uuid`, `base64`.

**Scope boundary:** This plan delivers Definition-of-Done items 1–5 and 7 from `docs/superpowers/specs/2026-06-01-accounts-identity-binding-design.md` (all server-side, all `cargo test`-verifiable). The minimal iOS signup/recovery flow (DoD #6, spec §10) is a **separate follow-up plan** written after this lands, because (a) it is a different toolchain not in the Rust CI and (b) the iOS Ristretto255 OPRF client must be built against the wire format this plan freezes. The integration tests here use a Rust `OprfClient` as a stand-in for the iOS client — this **is** the early interop test the spec §13 asks for.

---

## Important codebase facts (read before starting)

These were confirmed by a codebase survey. They differ in places from the spec's assumptions — trust this section.

- **The handshake currently verifies NOTHING.** `demo/src/server.rs` `handle_ws_connection` (around line 435–456) reads `params.identity` as a trusted string (`from_addr`) and never extracts or checks a signature. There is **no** `verify_init` function — only `sign_init` exists in `demo/src/signing.rs`. So this plan *adds* verification, it does not redirect an existing check.
- **`sign_init` hides the timestamp.** `demo/src/signing.rs::sign_init(key_pair, identity)` signs `format!("{}|{}", identity, timestamp)` where `timestamp = crate::protocol::now_rfc3339()` is generated internally and **not returned**. The handshake params carry only `signature`, not `timestamp`, so today the server could not reconstruct the signed payload even if it wanted to. Task 8 fixes this by making the timestamp explicit and putting it in the handshake params.
- **Repository pattern is free functions, not a struct.** `demo/src/repository.rs` is module-level `pub async fn foo(pool: &PgPool, ...) -> Result<T, StorageError>` using `sqlx::query(...)` / `sqlx::query_as::<_, Row>(...)`, `.bind(...)`, `.fetch_all/.fetch_one/.execute(pool)`. New account queries follow the same shape.
- **Migrations are runtime-loaded** from `demo/migrations/` by `sqlx::migrate::Migrator` in `demo/src/db.rs::init_pool` and in tests by `test_pool()`. Naming is `NNN_name.sql`. Latest is `002_compound_memory.sql`.
- **Errors:** `demo/src/error.rs` has `StorageError` (`Query(#[from] sqlx::Error)`, `NotFound{entity,id}`, `Duplicate(String)`) and `AppError` (`Storage(#[from] StorageError)`, `Protocol{code,message}`, `Unauthorized(String)`, `NoDB`).
- **Trait+mock template:** `demo/src/llm.rs` — `#[async_trait::async_trait] pub trait LlmClient` + `OpenAiClient` + `MockLlmClient { results: std::sync::Mutex<Vec<...>> }` with `new(...)` / `empty()`. `EmailSender` mirrors this exactly.
- **Server state:** `demo/src/server.rs` `ServerState { identity_store, agent_card, to_agent_tx, human_registry, agent_registry, threads, archival_store, federation: Option<FederationContext>, db: Option<sqlx::PgPool>, compile_tx, compile_notify_tx }` (derives `Clone`). Routers are built in `server.rs::start` (legacy) and `server.rs::start_federation` with `.route(...).with_state(state)`.
- **Test harness:** `demo/tests/integration.rs` — `async fn test_pool() -> Option<PgPool>` (returns `None`, test silently passes, if no DB), `fn unique_addr(prefix: &str) -> String`, tests are `#[tokio::test]`. Use these.
- **Known pre-existing failures (NOT regressions, do not try to fix):** `test_graph_traversal`, `test_compilation_dedup_updates_confidence`.
- **RTK caveat:** the shell hook rewrites `git ...` → `rtk git ...` and mangles output. For ground-truth git, run `rtk proxy git <args>`.

### Wire & crypto contract (frozen here for the future iOS client)

- **Handshake signature:** Ed25519 over the UTF-8 bytes of `"{identity}|{timestamp}"`, signature base64url-no-pad. `timestamp` is RFC3339 (`crate::protocol::now_rfc3339()` format). Both are carried in `ainm.initialize` `params`.
- **OPRF:** RFC 9497 `OPRF(ristretto255, SHA-512)` via `voprf` 0.5 ciphersuite `voprf::Ristretto255`.
  - OPRF **input** = `Argon2id(pin_utf8, salt)` → 32 bytes. `kdf_params` JSON = `{"salt": "<base64>", "m_cost": 19456, "t_cost": 2, "p_cost": 1}` (Argon2id defaults; `m_cost` in KiB).
  - Client: `OprfClient::<Ristretto255>::blind(&input, &mut OsRng)` → `{ state, message }`. `message.serialize()` → bytes → base64url for the wire (`blinded_point`).
  - Server: per-account secret = `OprfServer::<Ristretto255>::serialize()` bytes, stored in `key_escrow.oprf_key`. Evaluate with `OprfServer::deserialize(&oprf_key)?.blind_evaluate(&blinded)` → `EvaluationElement`; `.serialize()` → base64url (`evaluated_point`).
  - Client finalize: `state.finalize(&input, &evaluated_element)?` → 64-byte `oprf_output`.
  - `wrapping_key` = `HKDF-SHA256(ikm = oprf_output, salt = b"ainm-escrow-v1", info = b"wrapping-key").expand` → 32 bytes.
  - Secret blob (the 32-byte Ed25519 private-key seed) is sealed with `XChaCha20Poly1305::new(wrapping_key)` using a random 24-byte nonce; stored `ciphertext = nonce(24) || aead_ciphertext`.

---

## File structure

| File | Responsibility |
|------|----------------|
| `demo/Cargo.toml` | Add `voprf`, `argon2`, `hkdf`, `sha2`, `chacha20poly1305` deps |
| `demo/migrations/003_accounts.sql` | New tables: `accounts`, `account_keys`, `email_verifications`, `key_escrow` |
| `demo/src/accounts.rs` | Username validation + reserved blocklist (pure); account/key repository queries (DB) |
| `demo/src/email.rs` | `EmailSender` trait + `MockEmailSender` + minimal `ResendEmailSender`; OTP gen/hash |
| `demo/src/auth_token.rs` | Stateless HMAC tokens (issue/verify; purpose + expiry) |
| `demo/src/escrow.rs` | OPRF enrollment/evaluation + KDF + AEAD seal/open (pure crypto); escrow repository queries (DB) |
| `demo/src/signing.rs` | Add `verify_init`; change `sign_init` to take + return timestamp |
| `demo/src/server.rs` | New REST routes; handshake binding enforcement; `.well-known/ainm-identity/<username>` |
| `demo/src/main.rs` / `demo/src/lib.rs` | Register new modules |
| `demo/tests/integration.rs` | DB-backed integration tests for all flows |

Module registration: add `mod accounts; mod email; mod auth_token; mod escrow;` wherever the existing `mod`s are declared (check `demo/src/main.rs` and/or `demo/src/lib.rs` — follow the existing `mod compiler;` / `mod repository;` declarations).

---

## WAVE 1 — Schema, account lifecycle, email

### Task 1: Add dependencies and the accounts migration

**Files:**
- Modify: `demo/Cargo.toml`
- Create: `demo/migrations/003_accounts.sql`
- Test: `demo/tests/integration.rs` (new test `test_accounts_migration_applies`)

- [ ] **Step 1: Add crypto/email dependencies to `demo/Cargo.toml`**

Under `[dependencies]`, add (place near the existing crypto deps `ring`, `base64`):

```toml
# Accounts & identity binding (sub-project A)
voprf = { version = "0.5", features = ["ristretto255"] }
argon2 = "0.5"
hkdf = "0.12"
sha2 = "0.10"
chacha20poly1305 = { version = "0.10", features = ["alloc"] }
```

- [ ] **Step 2: Verify the crate graph resolves**

Run: `cd demo && cargo fetch && cargo build`
Expected: builds (nothing uses the new crates yet, so just confirms versions resolve). If `voprf` fails to resolve the `ristretto255` feature, run `cargo search voprf` and pin the latest 0.5.x; the feature name `ristretto255` is correct for 0.5.

- [ ] **Step 3: Write the migration `demo/migrations/003_accounts.sql`**

```sql
-- AINM Accounts & Identity Binding (sub-project A)
-- Real, non-spoofable accounts: handle <-> Ed25519 key binding + email-code signup
-- + zero-knowledge OPRF key escrow for recovery.

CREATE TABLE accounts (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username       TEXT NOT NULL,
    domain         TEXT NOT NULL,
    address        TEXT NOT NULL,                 -- username@domain
    email_hash     TEXT NOT NULL,                 -- sha256(lower(email)) hex
    email_verified BOOLEAN NOT NULL DEFAULT false,
    status         TEXT NOT NULL DEFAULT 'pending'
                   CHECK (status IN ('pending', 'active', 'locked')),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
-- Case-insensitive unique username + unique address + unique email.
CREATE UNIQUE INDEX accounts_username_lower_idx ON accounts (lower(username));
CREATE UNIQUE INDEX accounts_address_idx        ON accounts (address);
CREATE UNIQUE INDEX accounts_email_hash_idx     ON accounts (email_hash);

CREATE TABLE account_keys (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id   UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    public_key   TEXT NOT NULL,                   -- base64url(no-pad) Ed25519 pubkey
    device_label TEXT,
    enrolled_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at   TIMESTAMPTZ
);
CREATE UNIQUE INDEX account_keys_public_key_idx ON account_keys (public_key);
-- v1: exactly one active (revoked_at IS NULL) key per account.
CREATE UNIQUE INDEX account_keys_one_active_idx
    ON account_keys (account_id) WHERE revoked_at IS NULL;

CREATE TABLE email_verifications (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email_hash  TEXT NOT NULL,
    code_hash   TEXT NOT NULL,                    -- sha256(code) hex
    purpose     TEXT NOT NULL CHECK (purpose IN ('signup', 'recovery')),
    expires_at  TIMESTAMPTZ NOT NULL,
    attempts    INT NOT NULL DEFAULT 0,
    consumed_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX email_verifications_lookup_idx
    ON email_verifications (email_hash, purpose);

CREATE TABLE key_escrow (
    account_id   UUID PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    ciphertext   BYTEA NOT NULL,                  -- nonce(24) || XChaCha20Poly1305 ct
    kdf_params   JSONB NOT NULL,                  -- {salt, m_cost, t_cost, p_cost}
    oprf_key     BYTEA,                           -- serialized OprfServer; NULL once destroyed
    eval_attempts INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 5,
    destroyed    BOOLEAN NOT NULL DEFAULT false,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

- [ ] **Step 4: Write the failing test in `demo/tests/integration.rs`**

```rust
#[tokio::test]
async fn test_accounts_migration_applies() {
    let Some(pool) = test_pool().await else { return };
    // All four tables should exist after migrations run in test_pool().
    for table in ["accounts", "account_keys", "email_verifications", "key_escrow"] {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
        )
        .bind(table)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(exists, "table {table} should exist after migration");
    }
}
```

- [ ] **Step 5: Run the test**

Run: `cd demo && cargo test --test integration test_accounts_migration_applies -- --nocapture`
Expected: PASS if a Postgres is reachable; if not, `test_pool()` returns `None` and the test passes trivially (prints "Skipping DB test"). Either is acceptable.

- [ ] **Step 6: Commit**

```bash
rtk proxy git add demo/Cargo.toml demo/Cargo.lock demo/migrations/003_accounts.sql demo/tests/integration.rs
rtk proxy git commit -m "feat(accounts): add 003_accounts migration + crypto/email deps"
```

---

### Task 2: Username validation (pure)

**Files:**
- Create: `demo/src/accounts.rs`
- Modify: `demo/src/main.rs` and/or `demo/src/lib.rs` (add `mod accounts;`)
- Test: in `demo/src/accounts.rs` (`#[cfg(test)]`)

- [ ] **Step 1: Write the failing unit tests in `demo/src/accounts.rs`**

```rust
//! Account lifecycle: username validation (pure) + account/key repository queries (DB).

/// Reserved usernames that may never be registered.
const RESERVED: &[&str] = &[
    "admin", "administrator", "system", "compiler", "support",
    "root", "ainm", "server", "help", "abuse", "security", "postmaster",
];

#[derive(Debug, PartialEq, Eq)]
pub enum UsernameError {
    TooShort,
    TooLong,
    InvalidChars,
    Reserved,
}

/// Validate a username against the v1 rules: `[a-z0-9_]`, 3–30 chars, not reserved.
/// Input is lowercased before checking (usernames are case-insensitive).
pub fn validate_username(raw: &str) -> Result<String, UsernameError> {
    let name = raw.trim().to_ascii_lowercase();
    if name.len() < 3 { return Err(UsernameError::TooShort); }
    if name.len() > 30 { return Err(UsernameError::TooLong); }
    if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return Err(UsernameError::InvalidChars);
    }
    if RESERVED.contains(&name.as_str()) { return Err(UsernameError::Reserved); }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_and_normalizes_case() {
        assert_eq!(validate_username("Santi").unwrap(), "santi");
        assert_eq!(validate_username("  user_42 ").unwrap(), "user_42");
    }

    #[test]
    fn rejects_length_and_charset() {
        assert_eq!(validate_username("ab"), Err(UsernameError::TooShort));
        assert_eq!(validate_username(&"x".repeat(31)), Err(UsernameError::TooLong));
        assert_eq!(validate_username("bad name"), Err(UsernameError::InvalidChars));
        assert_eq!(validate_username("bad-name"), Err(UsernameError::InvalidChars));
    }

    #[test]
    fn rejects_reserved() {
        assert_eq!(validate_username("admin"), Err(UsernameError::Reserved));
        assert_eq!(validate_username("Compiler"), Err(UsernameError::Reserved));
    }
}
```

- [ ] **Step 2: Register the module**

Add `mod accounts;` alongside the other `mod` declarations (e.g. in `demo/src/main.rs`; if there is a `demo/src/lib.rs` that declares modules, add it there). Grep first: `rg "^mod (compiler|repository);" demo/src`.

- [ ] **Step 3: Run the tests**

Run: `cd demo && cargo test --lib accounts::tests`
Expected: 3 tests PASS.

- [ ] **Step 4: Commit**

```bash
rtk proxy git add demo/src/accounts.rs demo/src/main.rs demo/src/lib.rs
rtk proxy git commit -m "feat(accounts): username validation + reserved blocklist"
```

---

### Task 3: Account & key repository queries (DB)

**Files:**
- Modify: `demo/src/accounts.rs`
- Test: `demo/tests/integration.rs` (`test_account_repo_roundtrip`)

- [ ] **Step 1: Add row types and queries to `demo/src/accounts.rs`**

Add these imports at the top of the file and the code below the pure section:

```rust
use crate::error::StorageError;
use sqlx::PgPool;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct AccountRow {
    pub id: uuid::Uuid,
    pub username: String,
    pub domain: String,
    pub address: String,
    pub email_hash: String,
    pub email_verified: bool,
    pub status: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct AccountKeyRow {
    pub id: uuid::Uuid,
    pub account_id: uuid::Uuid,
    pub public_key: String,
    pub device_label: Option<String>,
}

/// Create a `pending` account. Returns `Duplicate` if username/address/email already exist.
pub async fn create_pending_account(
    pool: &PgPool,
    username: &str,
    domain: &str,
    email_hash: &str,
) -> Result<AccountRow, StorageError> {
    let address = format!("{username}@{domain}");
    let row = sqlx::query_as::<_, AccountRow>(
        r#"
        INSERT INTO accounts (username, domain, address, email_hash, status)
        VALUES ($1, $2, $3, $4, 'pending')
        RETURNING id, username, domain, address, email_hash, email_verified, status
        "#,
    )
    .bind(username)
    .bind(domain)
    .bind(&address)
    .bind(email_hash)
    .fetch_one(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            StorageError::Duplicate(format!("account {address}"))
        }
        _ => StorageError::Query(e),
    })?;
    Ok(row)
}

pub async fn find_account_by_address(
    pool: &PgPool,
    address: &str,
) -> Result<Option<AccountRow>, StorageError> {
    let row = sqlx::query_as::<_, AccountRow>(
        "SELECT id, username, domain, address, email_hash, email_verified, status \
         FROM accounts WHERE address = $1",
    )
    .bind(address)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn find_account_by_email_hash(
    pool: &PgPool,
    email_hash: &str,
) -> Result<Option<AccountRow>, StorageError> {
    let row = sqlx::query_as::<_, AccountRow>(
        "SELECT id, username, domain, address, email_hash, email_verified, status \
         FROM accounts WHERE email_hash = $1",
    )
    .bind(email_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn set_email_verified(pool: &PgPool, account_id: uuid::Uuid) -> Result<(), StorageError> {
    sqlx::query("UPDATE accounts SET email_verified = true, updated_at = NOW() WHERE id = $1")
        .bind(account_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_status(
    pool: &PgPool,
    account_id: uuid::Uuid,
    status: &str,
) -> Result<(), StorageError> {
    sqlx::query("UPDATE accounts SET status = $2, updated_at = NOW() WHERE id = $1")
        .bind(account_id)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(())
}

/// Bind a key to an account (v1: one active key). `Duplicate` if the pubkey is already bound
/// or the account already has an active key.
pub async fn insert_account_key(
    pool: &PgPool,
    account_id: uuid::Uuid,
    public_key: &str,
    device_label: Option<&str>,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO account_keys (account_id, public_key, device_label) VALUES ($1, $2, $3)",
    )
    .bind(account_id)
    .bind(public_key)
    .bind(device_label)
    .execute(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            StorageError::Duplicate(format!("key for account {account_id}"))
        }
        _ => StorageError::Query(e),
    })?;
    Ok(())
}

/// The active (non-revoked) key for an account, if any.
pub async fn find_active_key(
    pool: &PgPool,
    account_id: uuid::Uuid,
) -> Result<Option<AccountKeyRow>, StorageError> {
    let row = sqlx::query_as::<_, AccountKeyRow>(
        "SELECT id, account_id, public_key, device_label \
         FROM account_keys WHERE account_id = $1 AND revoked_at IS NULL",
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}
```

- [ ] **Step 2: Write the failing integration test in `demo/tests/integration.rs`**

Add `use ainm_demo::accounts;` to the imports if module-qualified imports are used in that file (check how `repository`/`graph` are imported at the top and match that style — they are likely imported as `use ainm_demo::{...}` or referenced as `ainm_demo::accounts`). Then:

```rust
#[tokio::test]
async fn test_account_repo_roundtrip() {
    let Some(pool) = test_pool().await else { return };
    let uniq = format!("{:x}", uuid::Uuid::new_v4().as_fields().0);
    let username = format!("user_{uniq}");
    let email_hash = format!("hash_{uniq}");

    let acct = accounts::create_pending_account(&pool, &username, "test.ainm.local", &email_hash)
        .await
        .expect("create");
    assert_eq!(acct.status, "pending");
    assert_eq!(acct.address, format!("{username}@test.ainm.local"));

    // Duplicate username/email rejected.
    let dup = accounts::create_pending_account(&pool, &username, "test.ainm.local", &email_hash).await;
    assert!(matches!(dup, Err(_)), "duplicate account must be rejected");

    accounts::set_email_verified(&pool, acct.id).await.unwrap();
    accounts::insert_account_key(&pool, acct.id, &format!("pk_{uniq}"), Some("iphone")).await.unwrap();

    let key = accounts::find_active_key(&pool, acct.id).await.unwrap().expect("active key");
    assert_eq!(key.public_key, format!("pk_{uniq}"));

    // Second active key rejected (one-active-key invariant).
    let second = accounts::insert_account_key(&pool, acct.id, &format!("pk2_{uniq}"), None).await;
    assert!(matches!(second, Err(_)), "second active key must be rejected");

    accounts::set_status(&pool, acct.id, "active").await.unwrap();
    let found = accounts::find_account_by_address(&pool, &acct.address).await.unwrap().unwrap();
    assert_eq!(found.status, "active");
}
```

- [ ] **Step 3: Run the test**

Run: `cd demo && cargo test --test integration test_account_repo_roundtrip -- --nocapture`
Expected: PASS (or trivially pass with no DB).

- [ ] **Step 4: Commit**

```bash
rtk proxy git add demo/src/accounts.rs demo/tests/integration.rs
rtk proxy git commit -m "feat(accounts): account + key repository queries"
```

---

### Task 4: EmailSender trait + mock + OTP helpers

**Files:**
- Create: `demo/src/email.rs`
- Modify: `demo/src/main.rs`/`lib.rs` (`mod email;`)
- Test: in `demo/src/email.rs` (`#[cfg(test)]`)

- [ ] **Step 1: Write `demo/src/email.rs` with the trait, mock, OTP helpers, and tests**

```rust
//! Email delivery abstraction (mirrors llm::LlmClient/MockLlmClient) + OTP helpers.

use sha2::{Digest, Sha256};

/// Sends a numeric verification code to an email address.
#[async_trait::async_trait]
pub trait EmailSender: Send + Sync {
    async fn send_code(&self, to_email: &str, code: &str, purpose: &str) -> Result<(), String>;
}

/// Test/dev sender: records every (email, code, purpose) and never hits the network.
pub struct MockEmailSender {
    pub sent: std::sync::Mutex<Vec<(String, String, String)>>,
}

impl MockEmailSender {
    pub fn new() -> Self {
        Self { sent: std::sync::Mutex::new(Vec::new()) }
    }
    /// The most recently sent code for an email+purpose, if any.
    pub fn last_code(&self, email: &str, purpose: &str) -> Option<String> {
        self.sent
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|(e, _, p)| e == email && p == purpose)
            .map(|(_, c, _)| c.clone())
    }
}

impl Default for MockEmailSender {
    fn default() -> Self { Self::new() }
}

#[async_trait::async_trait]
impl EmailSender for MockEmailSender {
    async fn send_code(&self, to_email: &str, code: &str, purpose: &str) -> Result<(), String> {
        tracing::info!("[email:mock] code {code} -> {to_email} ({purpose})");
        self.sent
            .lock()
            .unwrap()
            .push((to_email.to_string(), code.to_string(), purpose.to_string()));
        Ok(())
    }
}

/// Minimal real sender (Resend HTTP API). Used only when configured; never in CI.
pub struct ResendEmailSender {
    http: reqwest::Client,
    api_key: String,
    from: String,
}

impl ResendEmailSender {
    pub fn new(api_key: String, from: String) -> Self {
        Self { http: reqwest::Client::new(), api_key, from }
    }
}

#[async_trait::async_trait]
impl EmailSender for ResendEmailSender {
    async fn send_code(&self, to_email: &str, code: &str, purpose: &str) -> Result<(), String> {
        let subject = format!("Your AINM {purpose} code: {code}");
        let body = serde_json::json!({
            "from": self.from,
            "to": [to_email],
            "subject": subject,
            "text": format!("Your code is {code}. It expires in 10 minutes."),
        });
        let resp = self
            .http
            .post("https://api.resend.com/emails")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("resend returned {}", resp.status()));
        }
        Ok(())
    }
}

/// Hash an email (lowercased, trimmed) for storage/lookup. Hex sha256.
pub fn hash_email(email: &str) -> String {
    let mut h = Sha256::new();
    h.update(email.trim().to_ascii_lowercase().as_bytes());
    hex::encode(h.finalize())
}

/// Hash an OTP code for storage. Hex sha256.
pub fn hash_code(code: &str) -> String {
    let mut h = Sha256::new();
    h.update(code.as_bytes());
    hex::encode(h.finalize())
}

/// Generate a zero-padded 6-digit numeric OTP.
pub fn generate_code() -> String {
    use rand::Rng;
    let n: u32 = rand::thread_rng().gen_range(0..1_000_000);
    format!("{n:06}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_records_codes() {
        let m = MockEmailSender::new();
        m.send_code("a@b.com", "123456", "signup").await.unwrap();
        assert_eq!(m.last_code("a@b.com", "signup").as_deref(), Some("123456"));
        assert_eq!(m.last_code("a@b.com", "recovery"), None);
    }

    #[test]
    fn email_hash_is_case_insensitive() {
        assert_eq!(hash_email("  Foo@Bar.com "), hash_email("foo@bar.com"));
    }

    #[test]
    fn generate_code_is_six_digits() {
        for _ in 0..50 {
            let c = generate_code();
            assert_eq!(c.len(), 6);
            assert!(c.chars().all(|ch| ch.is_ascii_digit()));
        }
    }
}
```

- [ ] **Step 2: Register the module** (`mod email;`).

- [ ] **Step 3: Run the tests**

Run: `cd demo && cargo test --lib email::tests`
Expected: 3 PASS.

- [ ] **Step 4: Commit**

```bash
rtk proxy git add demo/src/email.rs demo/src/main.rs demo/src/lib.rs
rtk proxy git commit -m "feat(accounts): EmailSender trait + mock + OTP helpers"
```

---

### Task 5: OTP verification queries (DB)

**Files:**
- Modify: `demo/src/email.rs`
- Test: `demo/tests/integration.rs` (`test_otp_verify_flow`)

- [ ] **Step 1: Add OTP DB functions to `demo/src/email.rs`**

```rust
use crate::error::StorageError;
use sqlx::PgPool;

/// Insert a verification row (caller has already hashed code+email). 10-minute TTL.
pub async fn store_verification(
    pool: &PgPool,
    email_hash: &str,
    code_hash: &str,
    purpose: &str,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO email_verifications (email_hash, code_hash, purpose, expires_at) \
         VALUES ($1, $2, $3, NOW() + INTERVAL '10 minutes')",
    )
    .bind(email_hash)
    .bind(code_hash)
    .bind(purpose)
    .execute(pool)
    .await?;
    Ok(())
}

/// Outcome of verifying a code.
#[derive(Debug, PartialEq, Eq)]
pub enum VerifyOutcome {
    Ok,
    WrongCode,
    Expired,
    TooManyAttempts,
    NotFound,
}

const MAX_OTP_ATTEMPTS: i32 = 5;

/// Verify the latest unconsumed code for (email_hash, purpose). On success, marks it consumed.
/// On a wrong code, increments attempts; past MAX_OTP_ATTEMPTS returns TooManyAttempts.
pub async fn verify_code(
    pool: &PgPool,
    email_hash: &str,
    purpose: &str,
    code_hash: &str,
) -> Result<VerifyOutcome, StorageError> {
    // Newest unconsumed row for this email+purpose.
    let row: Option<(uuid::Uuid, String, i32, bool)> = sqlx::query_as(
        "SELECT id, code_hash, attempts, (expires_at < NOW()) AS expired \
         FROM email_verifications \
         WHERE email_hash = $1 AND purpose = $2 AND consumed_at IS NULL \
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(email_hash)
    .bind(purpose)
    .fetch_optional(pool)
    .await?;

    let Some((id, stored_hash, attempts, expired)) = row else {
        return Ok(VerifyOutcome::NotFound);
    };
    if attempts >= MAX_OTP_ATTEMPTS {
        return Ok(VerifyOutcome::TooManyAttempts);
    }
    if expired {
        return Ok(VerifyOutcome::Expired);
    }
    if stored_hash != code_hash {
        sqlx::query("UPDATE email_verifications SET attempts = attempts + 1 WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        return Ok(VerifyOutcome::WrongCode);
    }
    sqlx::query("UPDATE email_verifications SET consumed_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(VerifyOutcome::Ok)
}
```

- [ ] **Step 2: Write the failing integration test in `demo/tests/integration.rs`**

```rust
#[tokio::test]
async fn test_otp_verify_flow() {
    let Some(pool) = test_pool().await else { return };
    use ainm_demo::email;
    let eh = format!("eh_{:x}", uuid::Uuid::new_v4().as_fields().0);

    let code = "424242";
    email::store_verification(&pool, &eh, &email::hash_code(code), "signup").await.unwrap();

    // Wrong code increments attempts.
    assert_eq!(
        email::verify_code(&pool, &eh, "signup", &email::hash_code("000000")).await.unwrap(),
        email::VerifyOutcome::WrongCode
    );
    // Correct code consumes it.
    assert_eq!(
        email::verify_code(&pool, &eh, "signup", &email::hash_code(code)).await.unwrap(),
        email::VerifyOutcome::Ok
    );
    // Already consumed -> NotFound.
    assert_eq!(
        email::verify_code(&pool, &eh, "signup", &email::hash_code(code)).await.unwrap(),
        email::VerifyOutcome::NotFound
    );
}
```

- [ ] **Step 3: Run the test**

Run: `cd demo && cargo test --test integration test_otp_verify_flow -- --nocapture`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
rtk proxy git add demo/src/email.rs demo/tests/integration.rs
rtk proxy git commit -m "feat(accounts): OTP store + verify with attempt cap and expiry"
```

---

### Task 6: Stateless HMAC auth tokens

**Files:**
- Create: `demo/src/auth_token.rs`
- Modify: `demo/src/main.rs`/`lib.rs` (`mod auth_token;`)
- Test: in `demo/src/auth_token.rs`

- [ ] **Step 1: Write `demo/src/auth_token.rs`**

Single-purpose, short-lived tokens (`enrollment` and `recovery`) signed with an HMAC key the server holds. Stateless: payload is `account_id|purpose|expiry_unix`, tag is HMAC-SHA256, token = `base64url(payload).base64url(tag)`.

```rust
//! Stateless, single-purpose, short-lived auth tokens (HMAC-SHA256).
//! Used to gate the enrollment and recovery REST steps after email verification.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ring::hmac;

#[derive(Clone)]
pub struct TokenSigner {
    key: hmac::Key,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TokenError {
    Malformed,
    BadSignature,
    Expired,
    WrongPurpose,
}

impl TokenSigner {
    /// Build from raw secret bytes (from `AINM_TOKEN_SECRET` or a random startup value).
    pub fn new(secret: &[u8]) -> Self {
        Self { key: hmac::Key::new(hmac::HMAC_SHA256, secret) }
    }

    /// Issue a token for `account_id` + `purpose`, valid for `ttl_secs` from `now_unix`.
    pub fn issue(&self, account_id: &str, purpose: &str, now_unix: i64, ttl_secs: i64) -> String {
        let payload = format!("{account_id}|{purpose}|{}", now_unix + ttl_secs);
        let tag = hmac::sign(&self.key, payload.as_bytes());
        format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(payload.as_bytes()),
            URL_SAFE_NO_PAD.encode(tag.as_ref())
        )
    }

    /// Verify a token's signature, purpose, and expiry. Returns the `account_id` on success.
    pub fn verify(&self, token: &str, expected_purpose: &str, now_unix: i64) -> Result<String, TokenError> {
        let (p_b64, t_b64) = token.split_once('.').ok_or(TokenError::Malformed)?;
        let payload = URL_SAFE_NO_PAD.decode(p_b64).map_err(|_| TokenError::Malformed)?;
        let tag = URL_SAFE_NO_PAD.decode(t_b64).map_err(|_| TokenError::Malformed)?;
        hmac::verify(&self.key, &payload, &tag).map_err(|_| TokenError::BadSignature)?;

        let payload_str = String::from_utf8(payload).map_err(|_| TokenError::Malformed)?;
        let mut parts = payload_str.split('|');
        let account_id = parts.next().ok_or(TokenError::Malformed)?;
        let purpose = parts.next().ok_or(TokenError::Malformed)?;
        let exp: i64 = parts.next().ok_or(TokenError::Malformed)?
            .parse().map_err(|_| TokenError::Malformed)?;

        if purpose != expected_purpose { return Err(TokenError::WrongPurpose); }
        if now_unix >= exp { return Err(TokenError::Expired); }
        Ok(account_id.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_returns_account() {
        let s = TokenSigner::new(b"test-secret");
        let tok = s.issue("acct-1", "enrollment", 1000, 900);
        assert_eq!(s.verify(&tok, "enrollment", 1100).unwrap(), "acct-1");
    }

    #[test]
    fn rejects_wrong_purpose_expiry_and_tamper() {
        let s = TokenSigner::new(b"test-secret");
        let tok = s.issue("acct-1", "enrollment", 1000, 900);
        assert_eq!(s.verify(&tok, "recovery", 1100), Err(TokenError::WrongPurpose));
        assert_eq!(s.verify(&tok, "enrollment", 2000), Err(TokenError::Expired));

        let other = TokenSigner::new(b"different-secret");
        assert_eq!(other.verify(&tok, "enrollment", 1100), Err(TokenError::BadSignature));
        assert_eq!(s.verify("garbage", "enrollment", 1100), Err(TokenError::Malformed));
    }
}
```

- [ ] **Step 2: Register the module** (`mod auth_token;`).

- [ ] **Step 3: Run the tests**

Run: `cd demo && cargo test --lib auth_token::tests`
Expected: 2 PASS.

- [ ] **Step 4: Commit**

```bash
rtk proxy git add demo/src/auth_token.rs demo/src/main.rs demo/src/lib.rs
rtk proxy git commit -m "feat(accounts): stateless HMAC enrollment/recovery tokens"
```

---

### Task 7: Signup/verify/keys REST endpoints + wiring

**Files:**
- Modify: `demo/src/server.rs` (handlers + routes + state fields)
- Modify: `demo/src/main.rs` (construct `EmailSender` + `TokenSigner`, pass into state)
- Test: `demo/tests/integration.rs` (`test_signup_verify_bind_key`)

This task adds the three pre-escrow endpoints and the shared dependencies they need. Escrow endpoints come in Wave 3.

- [ ] **Step 1: Extend `ServerState` with the new dependencies**

In `demo/src/server.rs`, add fields to `ServerState` (it derives `Clone`; wrap non-`Clone` deps in `Arc`):

```rust
    pub email_sender: std::sync::Arc<dyn crate::email::EmailSender>,
    pub token_signer: crate::auth_token::TokenSigner,
    pub server_domain: String,
```

Update every place that constructs `ServerState` (grep: `rg "ServerState \{" demo/src`) to set these. For tests/legacy paths with no real email, default to `Arc::new(crate::email::MockEmailSender::new())`, a `TokenSigner::new` over a random or env secret, and the configured domain (e.g. `"demo.ainm.local"`).

- [ ] **Step 2: Construct dependencies in `demo/src/main.rs`**

Where `ServerState`/`start_federation` are wired (near the db_pool/compile_tx construction, ~line 417–499), add:

```rust
    let token_secret = std::env::var("AINM_TOKEN_SECRET")
        .map(|s| s.into_bytes())
        .unwrap_or_else(|_| {
            use rand::RngCore;
            let mut b = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut b);
            tracing::warn!("[accounts] AINM_TOKEN_SECRET unset; using a random per-process secret (tokens won't survive restart)");
            b.to_vec()
        });
    let token_signer = crate::auth_token::TokenSigner::new(&token_secret);

    let email_sender: std::sync::Arc<dyn crate::email::EmailSender> =
        match (std::env::var("AINM_EMAIL__RESEND_API_KEY").ok(), std::env::var("AINM_EMAIL__FROM").ok()) {
            (Some(key), Some(from)) => {
                tracing::info!("[email] using Resend sender");
                std::sync::Arc::new(crate::email::ResendEmailSender::new(key, from))
            }
            _ => {
                tracing::info!("[email] no AINM_EMAIL__RESEND_API_KEY; using mock email sender (codes logged)");
                std::sync::Arc::new(crate::email::MockEmailSender::new())
            }
        };
```

Thread `token_signer`, `email_sender`, and the server domain string into the `ServerState` construction (and into `start`/`start_federation` if those build the state — adjust their signatures to accept the new fields, or build `ServerState` in `main.rs` and pass it in; follow whichever pattern already exists).

- [ ] **Step 3: Add request/response types and handlers to `demo/src/server.rs`**

```rust
use axum::{extract::State, Json};
use axum::http::StatusCode;

#[derive(serde::Deserialize)]
pub struct SignupReq { pub email: String, pub username: String }

#[derive(serde::Serialize)]
pub struct SignupResp { pub address: String }

pub async fn signup_handler(
    State(state): State<ServerState>,
    Json(req): Json<SignupReq>,
) -> Result<Json<SignupResp>, (StatusCode, String)> {
    let Some(pool) = state.db.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "database not configured".into()));
    };
    let username = crate::accounts::validate_username(&req.username)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid username: {e:?}")))?;
    if !req.email.contains('@') {
        return Err((StatusCode::BAD_REQUEST, "invalid email".into()));
    }
    let email_hash = crate::email::hash_email(&req.email);

    let acct = crate::accounts::create_pending_account(pool, &username, &state.server_domain, &email_hash)
        .await
        .map_err(|e| match e {
            crate::error::StorageError::Duplicate(_) =>
                (StatusCode::CONFLICT, "username or email already registered".into()),
            other => (StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
        })?;

    let code = crate::email::generate_code();
    crate::email::store_verification(pool, &email_hash, &crate::email::hash_code(&code), "signup")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    state.email_sender.send_code(&req.email, &code, "signup").await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(SignupResp { address: acct.address }))
}

#[derive(serde::Deserialize)]
pub struct VerifyReq { pub email: String, pub code: String }

#[derive(serde::Serialize)]
pub struct VerifyResp { pub enrollment_token: String }

pub async fn verify_handler(
    State(state): State<ServerState>,
    Json(req): Json<VerifyReq>,
) -> Result<Json<VerifyResp>, (StatusCode, String)> {
    let Some(pool) = state.db.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "database not configured".into()));
    };
    let email_hash = crate::email::hash_email(&req.email);
    let outcome = crate::email::verify_code(pool, &email_hash, "signup", &crate::email::hash_code(&req.code))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if outcome != crate::email::VerifyOutcome::Ok {
        return Err((StatusCode::UNAUTHORIZED, format!("{outcome:?}")));
    }
    let acct = crate::accounts::find_account_by_email_hash(pool, &email_hash)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "account not found".into()))?;
    crate::accounts::set_email_verified(pool, acct.id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let token = state.token_signer.issue(&acct.id.to_string(), "enrollment", crate::protocol::now_unix(), 900);
    Ok(Json(VerifyResp { enrollment_token: token }))
}

#[derive(serde::Deserialize)]
pub struct BindKeyReq { pub enrollment_token: String, pub public_key: String, pub device_label: Option<String> }

pub async fn bind_key_handler(
    State(state): State<ServerState>,
    Json(req): Json<BindKeyReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    let Some(pool) = state.db.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "database not configured".into()));
    };
    let account_id_str = state.token_signer
        .verify(&req.enrollment_token, "enrollment", crate::protocol::now_unix())
        .map_err(|e| (StatusCode::UNAUTHORIZED, format!("{e:?}")))?;
    let account_id = uuid::Uuid::parse_str(&account_id_str)
        .map_err(|_| (StatusCode::BAD_REQUEST, "bad token subject".into()))?;
    crate::accounts::insert_account_key(pool, account_id, &req.public_key, req.device_label.as_deref())
        .await
        .map_err(|e| match e {
            crate::error::StorageError::Duplicate(_) =>
                (StatusCode::CONFLICT, "key already bound".into()),
            other => (StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
        })?;
    Ok(StatusCode::NO_CONTENT)
}
```

If `crate::protocol::now_unix()` does not exist, add it next to `now_rfc3339()` in `demo/src/protocol.rs`:

```rust
/// Current Unix time in seconds.
pub fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}
```

- [ ] **Step 4: Register the routes**

In **both** router builders (`server.rs::start` and `server.rs::start_federation`), add before `.with_state(state)`:

```rust
        .route("/accounts/signup", axum::routing::post(signup_handler))
        .route("/accounts/verify", axum::routing::post(verify_handler))
        .route("/accounts/keys", axum::routing::post(bind_key_handler))
```

- [ ] **Step 5: Write the failing integration test in `demo/tests/integration.rs`**

This test calls the handlers directly (constructing a `ServerState` with a `MockEmailSender`), avoiding a live HTTP server. Build a minimal `ServerState` the way other server tests do (grep `rg "ServerState \{" demo/tests`); if no server test exists yet, drive the flow through the repository + handler functions. Concretely, test the handler functions by constructing state with the mock and reading the code back from the mock:

```rust
#[tokio::test]
async fn test_signup_verify_bind_key() {
    let Some(pool) = test_pool().await else { return };
    use ainm_demo::{accounts, email, auth_token, protocol};

    let uniq = format!("{:x}", uuid::Uuid::new_v4().as_fields().0);
    let username = format!("u{uniq}");
    let address = format!("{username}@test.ainm.local");
    let user_email = format!("{username}@mail.test");
    let email_hash = email::hash_email(&user_email);

    // signup
    let _acct = accounts::create_pending_account(&pool, &username, "test.ainm.local", &email_hash)
        .await.unwrap();
    let code = email::generate_code();
    email::store_verification(&pool, &email_hash, &email::hash_code(&code), "signup").await.unwrap();
    let mock = email::MockEmailSender::new();
    mock.send_code(&user_email, &code, "signup").await.unwrap();
    assert!(mock.last_code(&user_email, "signup").is_some());

    // verify
    assert_eq!(
        email::verify_code(&pool, &email_hash, "signup", &email::hash_code(&code)).await.unwrap(),
        email::VerifyOutcome::Ok
    );
    let acct = accounts::find_account_by_email_hash(&pool, &email_hash).await.unwrap().unwrap();
    accounts::set_email_verified(&pool, acct.id).await.unwrap();

    // enrollment token gates key binding
    let signer = auth_token::TokenSigner::new(b"itest-secret");
    let token = signer.issue(&acct.id.to_string(), "enrollment", protocol::now_unix(), 900);
    let aid = signer.verify(&token, "enrollment", protocol::now_unix()).unwrap();
    accounts::insert_account_key(&pool, uuid::Uuid::parse_str(&aid).unwrap(), &format!("pk_{uniq}"), Some("test")).await.unwrap();

    let key = accounts::find_active_key(&pool, acct.id).await.unwrap().unwrap();
    assert_eq!(key.public_key, format!("pk_{uniq}"));
    let _ = address; // address asserted bound via account row
}
```

- [ ] **Step 6: Run the test and full build**

Run: `cd demo && cargo build && cargo test --test integration test_signup_verify_bind_key -- --nocapture`
Expected: build succeeds (all `ServerState` constructors updated), test PASSES.

- [ ] **Step 7: Commit**

```bash
rtk proxy git add demo/src/server.rs demo/src/main.rs demo/src/protocol.rs demo/tests/integration.rs
rtk proxy git commit -m "feat(accounts): signup/verify/bind-key REST endpoints + wiring"
```

---

## WAVE 2 — Handshake binding enforcement (the security fix)

### Task 8: Carry the handshake timestamp explicitly

**Files:**
- Modify: `demo/src/signing.rs` (`sign_init` returns `(sig, timestamp)`)
- Modify: all `sign_init` callers (grep `rg "sign_init" demo/src`)
- Test: in `demo/src/signing.rs`

- [ ] **Step 1: Change `sign_init` to return the timestamp**

Replace the existing `sign_init` in `demo/src/signing.rs`:

```rust
/// Sign the init payload `"{identity}|{timestamp}"`. Returns (signature_b64url, timestamp).
/// The caller MUST place both the signature and the timestamp in the handshake params so
/// the server can reconstruct and verify the payload (see verify_init).
pub fn sign_init(key_pair: &Ed25519KeyPair, identity: &str) -> Result<(String, String)> {
    let timestamp = crate::protocol::now_rfc3339();
    let payload = format!("{}|{}", identity, timestamp);
    let sig = key_pair.sign(payload.as_bytes());
    Ok((URL_SAFE_NO_PAD.encode(sig.as_ref()), timestamp))
}
```

- [ ] **Step 2: Update every caller**

Run `rg "sign_init\(" demo/src demo/tests` and update each call site to destructure the tuple and include `timestamp` in the init params JSON alongside `signature`. For example, wherever the init request `params` are built, add `"timestamp": timestamp`.

- [ ] **Step 3: Run build**

Run: `cd demo && cargo build`
Expected: the signature change forces every caller to be updated; build is the gate.

- [ ] **Step 4: Commit**

```bash
rtk proxy git add demo/src/signing.rs demo/src
rtk proxy git commit -m "refactor(handshake): carry init timestamp in params for verification"
```

---

### Task 9: `verify_init` (pure)

**Files:**
- Modify: `demo/src/signing.rs`
- Test: in `demo/src/signing.rs`

- [ ] **Step 1: Add `verify_init` with a freshness window and a unit test**

```rust
/// Verify a handshake signature against a known public key.
/// Reconstructs `"{identity}|{timestamp}"`, checks the Ed25519 signature, and rejects
/// timestamps outside +/- `max_skew_secs` (replay/clock-skew guard).
pub fn verify_init(
    public_key_bytes: &[u8],
    identity: &str,
    timestamp: &str,
    signature_b64: &str,
    now: chrono::DateTime<chrono::Utc>,
    max_skew_secs: i64,
) -> bool {
    // Freshness: timestamp must parse and be within the window.
    let Ok(ts) = chrono::DateTime::parse_from_rfc3339(timestamp) else { return false; };
    let skew = (now - ts.with_timezone(&chrono::Utc)).num_seconds().abs();
    if skew > max_skew_secs { return false; }

    let Ok(sig_bytes) = URL_SAFE_NO_PAD.decode(signature_b64) else { return false; };
    let payload = format!("{}|{}", identity, timestamp);
    let public_key = UnparsedPublicKey::new(&ED25519, public_key_bytes);
    public_key.verify(payload.as_bytes(), &sig_bytes).is_ok()
}

#[cfg(test)]
mod verify_init_tests {
    use super::*;
    use crate::identity::generate_keypair;

    #[test]
    fn accepts_valid_rejects_tampered_and_stale() {
        let id = generate_keypair("alice@test.local").unwrap();
        let (sig, ts) = sign_init(&id.key_pair, "alice@test.local").unwrap();
        let now = chrono::DateTime::parse_from_rfc3339(&ts).unwrap().with_timezone(&chrono::Utc);

        // valid
        assert!(verify_init(&id.public_key_bytes, "alice@test.local", &ts, &sig, now, 300));
        // wrong identity
        assert!(!verify_init(&id.public_key_bytes, "mallory@test.local", &ts, &sig, now, 300));
        // wrong key
        let other = generate_keypair("bob@test.local").unwrap();
        assert!(!verify_init(&other.public_key_bytes, "alice@test.local", &ts, &sig, now, 300));
        // stale timestamp
        let later = now + chrono::Duration::seconds(10_000);
        assert!(!verify_init(&id.public_key_bytes, "alice@test.local", &ts, &sig, later, 300));
    }
}
```

Ensure the needed imports (`URL_SAFE_NO_PAD`, `UnparsedPublicKey`, `ED25519`) are already present at the top of `signing.rs` (they are, used by `verify_signature`).

- [ ] **Step 2: Run the test**

Run: `cd demo && cargo test --lib signing::verify_init_tests`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
rtk proxy git add demo/src/signing.rs
rtk proxy git commit -m "feat(handshake): verify_init with freshness window"
```

---

### Task 10: Enforce the binding in the WebSocket handshake

**Files:**
- Modify: `demo/src/server.rs` (`handle_ws_connection`)
- Test: `demo/tests/integration.rs` (`test_handshake_enforcement`)

- [ ] **Step 1: Add enforcement after parsing `ainm.initialize`**

In `handle_ws_connection`, after `from_addr` is extracted (~line 451–456) and **before** the `ainm.initialized` response is sent (~line 459), insert the lookup-and-verify gate. If `state.db` is `None`, preserve today's behavior (no DB → no enforcement; legacy/in-memory mode). When DB is present, enforce:

```rust
    // --- Anti-spoof: verify the claimed address against its REGISTERED key (sub-project A) ---
    if let Some(pool) = state.db.clone() {
        let signature = init_req
            .get("params").and_then(|p| p.get("signature")).and_then(|v| v.as_str())
            .unwrap_or("");
        let timestamp = init_req
            .get("params").and_then(|p| p.get("timestamp")).and_then(|v| v.as_str())
            .unwrap_or("");

        let reject = |sender: std::sync::Arc<_>, req_id: serde_json::Value, msg: &str| async move {
            let err = protocol::new_error(req_id, -32021, msg);
            let mut s = sender.lock().await;
            let _ = s.send(Message::Text(serde_json::to_string(&err).unwrap().into())).await;
        };

        // 1. account must exist
        let account = match crate::accounts::find_account_by_address(&pool, &from_addr).await {
            Ok(Some(a)) => a,
            _ => {
                reject(ws_sender.clone(), req_id.clone(), "unknown address").await;
                return;
            }
        };
        // 2. account must be active
        if account.status != "active" {
            reject(ws_sender.clone(), req_id.clone(), "account not active").await;
            return;
        }
        // 3. must have an active registered key; verify signature against THAT key
        let registered_pk_b64 = match crate::accounts::find_active_key(&pool, account.id).await {
            Ok(Some(k)) => k.public_key,
            _ => {
                reject(ws_sender.clone(), req_id.clone(), "no registered key").await;
                return;
            }
        };
        let pk_bytes = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&registered_pk_b64) {
            Ok(b) => b,
            Err(_) => { reject(ws_sender.clone(), req_id.clone(), "corrupt key").await; return; }
        };
        if !crate::signing::verify_init(&pk_bytes, &from_addr, timestamp, signature, chrono::Utc::now(), 300) {
            reject(ws_sender.clone(), req_id.clone(), "signature does not match registered key").await;
            return;
        }
        tracing::info!("[handshake] verified binding for {from_addr}");
    }
```

Make sure `base64::Engine` is in scope (the file already uses base64 elsewhere; add `use base64::Engine;` if needed). `protocol::new_error(req_id, code, msg)` already exists (used in the not-initialized path).

- [ ] **Step 2: Update any pre-existing handshake test that connects without an account**

Run `rg "ainm.initialize|handle_ws_connection|ainm\.initialized" demo/tests`. Any existing test that opens a connection now needs either no DB on its state, or a pre-seeded active account+key. Update those tests to seed `accounts::create_pending_account` + `insert_account_key` + `set_status(active)` for the address they use, signing the init with the matching key.

- [ ] **Step 3: Write the enforcement integration test**

Because `handle_ws_connection` is private and consumes a real socket, test the **enforcement decision** at the unit boundary it relies on: seed accounts and assert `find_account_by_address` + `find_active_key` + `verify_init` together accept the bound key and reject a spoof / unknown address. This exercises the exact logic the handshake runs.

```rust
#[tokio::test]
async fn test_handshake_enforcement() {
    let Some(pool) = test_pool().await else { return };
    use ainm_demo::{accounts, signing, identity};
    use base64::Engine;
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;

    let uniq = format!("{:x}", uuid::Uuid::new_v4().as_fields().0);
    let username = format!("alice{uniq}");
    let address = format!("{username}@test.ainm.local");

    // Bind key A to the account and activate it.
    let key_a = identity::generate_keypair(&address).unwrap();
    let acct = accounts::create_pending_account(&pool, &username, "test.ainm.local", &format!("eh_{uniq}")).await.unwrap();
    accounts::insert_account_key(&pool, acct.id, &b64.encode(&key_a.public_key_bytes), Some("a")).await.unwrap();
    accounts::set_status(&pool, acct.id, "active").await.unwrap();

    // Resolve registered key for the address (what the handshake does).
    let found = accounts::find_account_by_address(&pool, &address).await.unwrap().unwrap();
    assert_eq!(found.status, "active");
    let reg = accounts::find_active_key(&pool, found.id).await.unwrap().unwrap();
    let reg_bytes = b64.decode(&reg.public_key).unwrap();

    // Correct key A signs -> accepted.
    let (sig_a, ts_a) = signing::sign_init(&key_a.key_pair, &address).unwrap();
    let now = chrono::Utc::now();
    assert!(signing::verify_init(&reg_bytes, &address, &ts_a, &sig_a, now, 300), "bound key must be accepted");

    // Spoof: key B signs as alice -> rejected (verified against registered key A).
    let key_b = identity::generate_keypair(&address).unwrap();
    let (sig_b, ts_b) = signing::sign_init(&key_b.key_pair, &address).unwrap();
    assert!(!signing::verify_init(&reg_bytes, &address, &ts_b, &sig_b, now, 300), "spoofed key must be rejected");

    // Unknown address -> no account row.
    let unknown = format!("ghost{uniq}@test.ainm.local");
    assert!(accounts::find_account_by_address(&pool, &unknown).await.unwrap().is_none());
}
```

- [ ] **Step 4: Run build + tests**

Run: `cd demo && cargo build && cargo test --test integration test_handshake_enforcement -- --nocapture`
Expected: build OK, test PASSES.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add demo/src/server.rs demo/tests/integration.rs
rtk proxy git commit -m "feat(handshake): enforce address<->registered-key binding on connect"
```

---

## WAVE 3 — Escrow & recovery

### Task 11: OPRF escrow crypto (pure, round-trip tested)

**Files:**
- Create: `demo/src/escrow.rs`
- Modify: `demo/src/main.rs`/`lib.rs` (`mod escrow;`)
- Test: in `demo/src/escrow.rs`

**Reference for the implementer:** RFC 9497 `OPRF(ristretto255, SHA-512)` via `voprf` 0.5, ciphersuite `voprf::Ristretto255`. Verify exact method arity against `cargo doc --open -p voprf` for the resolved version — `blind_evaluate` may take a single `&BlindedElement` or a slice depending on patch version; adapt the call accordingly. The TDD round-trip test below is the contract; make the API calls satisfy it.

- [ ] **Step 1: Write the failing round-trip test first (drives the API)**

In `demo/src/escrow.rs`:

```rust
//! Zero-knowledge key escrow: RFC 9497 OPRF(ristretto255, SHA-512) + Argon2id + HKDF + XChaCha20Poly1305.
//!
//! The server never learns the PIN or the wrapping key — only a serialized OPRF server state
//! (the per-account secret), the ciphertext, and a guess counter. See spec §6.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escrow_round_trip_correct_pin_recovers_wrong_pin_fails() {
        // The 32-byte secret we are escrowing (stands in for an Ed25519 private seed).
        let secret = [7u8; 32];
        let pin = "246810";

        // ---- Enrollment (client side) + server key creation ----
        let enrolled = enroll(&secret, pin).expect("enroll");
        // Server persists enrolled.oprf_key + enrolled.ciphertext + enrolled.kdf_params.

        // ---- Recovery with the CORRECT pin ----
        // client blinds:
        let blind = client_blind(pin, &enrolled.kdf_params).expect("blind");
        // server evaluates with the stored oprf_key:
        let evaluated = server_evaluate(&enrolled.oprf_key, &blind.blinded_b64).expect("eval");
        // client finalizes + decrypts:
        let recovered = client_finalize_and_open(
            pin, &enrolled.kdf_params, &blind, &evaluated, &enrolled.ciphertext,
        ).expect("recover");
        assert_eq!(recovered, secret, "correct PIN must recover the exact secret");

        // ---- Recovery with a WRONG pin must fail to decrypt ----
        let bad = "000000";
        let blind2 = client_blind(bad, &enrolled.kdf_params).expect("blind2");
        let evaluated2 = server_evaluate(&enrolled.oprf_key, &blind2.blinded_b64).expect("eval2");
        let res = client_finalize_and_open(
            bad, &enrolled.kdf_params, &blind2, &evaluated2, &enrolled.ciphertext,
        );
        assert!(res.is_err(), "wrong PIN must fail AEAD decryption");
    }
}
```

- [ ] **Step 2: Implement `escrow.rs` to satisfy the test**

```rust
use argon2::Argon2;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use voprf::{BlindedElement, EvaluationElement, OprfClient, OprfServer, Ristretto255};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfParams {
    pub salt: String, // base64url
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl KdfParams {
    fn fresh() -> Self {
        let mut salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut salt);
        Self { salt: URL_SAFE_NO_PAD.encode(salt), m_cost: 19_456, t_cost: 2, p_cost: 1 }
    }
}

/// Argon2id(PIN, salt) -> 32-byte OPRF input.
fn pin_to_input(pin: &str, kdf: &KdfParams) -> Result<[u8; 32], String> {
    let salt = URL_SAFE_NO_PAD.decode(&kdf.salt).map_err(|e| e.to_string())?;
    let params = argon2::Params::new(kdf.m_cost, kdf.t_cost, kdf.p_cost, Some(32))
        .map_err(|e| e.to_string())?;
    let argon = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut out = [0u8; 32];
    argon.hash_password_into(pin.as_bytes(), &salt, &mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

/// HKDF-SHA256(oprf_output) -> 32-byte wrapping key.
fn wrapping_key(oprf_output: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(b"ainm-escrow-v1"), oprf_output);
    let mut key = [0u8; 32];
    hk.expand(b"wrapping-key", &mut key).expect("hkdf expand");
    key
}

pub struct Enrolled {
    pub oprf_key: Vec<u8>,      // serialized OprfServer (store in key_escrow.oprf_key)
    pub ciphertext: Vec<u8>,    // nonce(24) || aead ciphertext
    pub kdf_params: KdfParams,
}

/// Client-side enrollment: derive the wrapping key via a local OPRF round (the client also
/// owns the server object at enrollment time), seal the secret, and emit what the server stores.
pub fn enroll(secret: &[u8; 32], pin: &str) -> Result<Enrolled, String> {
    let kdf = KdfParams::fresh();
    let input = pin_to_input(pin, &kdf)?;

    let server = OprfServer::<Ristretto255>::new(&mut rand::thread_rng()).map_err(|e| e.to_string())?;
    let blind = OprfClient::<Ristretto255>::blind(&input, &mut rand::thread_rng()).map_err(|e| e.to_string())?;
    let eval = server.blind_evaluate(&blind.message);
    let output = blind.state.finalize(&input, &eval).map_err(|e| e.to_string())?;

    let wk = wrapping_key(&output);
    let cipher = XChaCha20Poly1305::new((&wk).into());
    let mut nonce_bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);
    let ct = cipher.encrypt(nonce, secret.as_ref()).map_err(|e| e.to_string())?;

    let mut ciphertext = nonce_bytes.to_vec();
    ciphertext.extend_from_slice(&ct);

    Ok(Enrolled { oprf_key: server.serialize().to_vec(), ciphertext, kdf_params: kdf })
}

pub struct BlindState {
    pub blinded_b64: String,
    client: OprfClient<Ristretto255>,
}

/// Client step 1 of recovery: blind the PIN-derived input.
pub fn client_blind(pin: &str, kdf: &KdfParams) -> Result<BlindState, String> {
    let input = pin_to_input(pin, kdf)?;
    let blind = OprfClient::<Ristretto255>::blind(&input, &mut rand::thread_rng()).map_err(|e| e.to_string())?;
    Ok(BlindState {
        blinded_b64: URL_SAFE_NO_PAD.encode(blind.message.serialize()),
        client: blind.state,
    })
}

/// Server step: evaluate a blinded element with the stored OPRF key. Returns base64url evaluated point.
pub fn server_evaluate(oprf_key: &[u8], blinded_b64: &str) -> Result<String, String> {
    let server = OprfServer::<Ristretto255>::deserialize(oprf_key).map_err(|e| e.to_string())?;
    let blinded_bytes = URL_SAFE_NO_PAD.decode(blinded_b64).map_err(|e| e.to_string())?;
    let blinded = BlindedElement::<Ristretto255>::deserialize(&blinded_bytes).map_err(|e| e.to_string())?;
    let eval = server.blind_evaluate(&blinded);
    Ok(URL_SAFE_NO_PAD.encode(eval.serialize()))
}

/// Client step 2 of recovery: finalize the OPRF, derive the wrapping key, open the ciphertext.
pub fn client_finalize_and_open(
    pin: &str,
    kdf: &KdfParams,
    blind: &BlindState,
    evaluated_b64: &str,
    ciphertext: &[u8],
) -> Result<[u8; 32], String> {
    let input = pin_to_input(pin, kdf)?;
    let eval_bytes = URL_SAFE_NO_PAD.decode(evaluated_b64).map_err(|e| e.to_string())?;
    let eval = EvaluationElement::<Ristretto255>::deserialize(&eval_bytes).map_err(|e| e.to_string())?;
    let output = blind.client.clone().finalize(&input, &eval).map_err(|e| e.to_string())?;
    let wk = wrapping_key(&output);

    if ciphertext.len() < 24 { return Err("ciphertext too short".into()); }
    let (nonce_bytes, ct) = ciphertext.split_at(24);
    let cipher = XChaCha20Poly1305::new((&wk).into());
    let plain = cipher
        .decrypt(XNonce::from_slice(nonce_bytes), ct)
        .map_err(|_| "decryption failed (wrong PIN)".to_string())?;
    let mut out = [0u8; 32];
    if plain.len() != 32 { return Err("unexpected plaintext length".into()); }
    out.copy_from_slice(&plain);
    Ok(out)
}
```

Notes for the implementer: if `OprfClient` is not `Clone`, restructure `client_finalize_and_open` to take `BlindState` by value (the test calls it once per blind, so by-value is fine — adjust the test's borrow to a move if needed). If `blind_evaluate` requires a slice, wrap as `&[blinded]` and the result is the single `EvaluationElement`.

- [ ] **Step 3: Register the module** (`mod escrow;`) and run the test

Run: `cd demo && cargo test --lib escrow::tests -- --nocapture`
Expected: PASS — correct PIN recovers `[7u8;32]`, wrong PIN errors.

- [ ] **Step 4: Commit**

```bash
rtk proxy git add demo/src/escrow.rs demo/src/main.rs demo/src/lib.rs demo/Cargo.lock
rtk proxy git commit -m "feat(escrow): OPRF + Argon2id + HKDF + XChaCha20Poly1305 round-trip"
```

---

### Task 12: Escrow repository + escrow/recovery endpoints

**Files:**
- Modify: `demo/src/escrow.rs` (DB queries)
- Modify: `demo/src/server.rs` (handlers + routes)
- Test: `demo/tests/integration.rs` (`test_escrow_store_and_fetch`)

- [ ] **Step 1: Add escrow DB queries to `demo/src/escrow.rs`**

```rust
use crate::error::StorageError;
use sqlx::PgPool;

/// Finalize enrollment: persist ciphertext + kdf_params + oprf server state, then activate.
pub async fn store_escrow(
    pool: &PgPool,
    account_id: uuid::Uuid,
    ciphertext: &[u8],
    kdf_params: &KdfParams,
    oprf_key: &[u8],
) -> Result<(), StorageError> {
    let kdf_json = serde_json::to_value(kdf_params).map_err(|e| StorageError::Duplicate(e.to_string()))?;
    sqlx::query(
        "INSERT INTO key_escrow (account_id, ciphertext, kdf_params, oprf_key) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (account_id) DO UPDATE SET \
           ciphertext = $2, kdf_params = $3, oprf_key = $4, eval_attempts = 0, destroyed = false, updated_at = NOW()",
    )
    .bind(account_id)
    .bind(ciphertext)
    .bind(kdf_json)
    .bind(oprf_key)
    .execute(pool)
    .await?;
    Ok(())
}

pub struct EscrowFetch {
    pub evaluated_b64: String,
    pub ciphertext: Vec<u8>,
    pub kdf_params: KdfParams,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FetchError {
    Destroyed,
    NotFound,
    Crypto(String),
    Db(String),
}

/// One recovery guess = one server OPRF evaluation. Increments eval_attempts BEFORE evaluating;
/// on reaching max_attempts, deletes oprf_key + sets destroyed and returns Destroyed.
pub async fn fetch_and_evaluate(
    pool: &PgPool,
    account_id: uuid::Uuid,
    blinded_b64: &str,
) -> Result<EscrowFetch, FetchError> {
    let row: Option<(Option<Vec<u8>>, Vec<u8>, serde_json::Value, i32, i32, bool)> = sqlx::query_as(
        "SELECT oprf_key, ciphertext, kdf_params, eval_attempts, max_attempts, destroyed \
         FROM key_escrow WHERE account_id = $1",
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| FetchError::Db(e.to_string()))?;

    let Some((oprf_key_opt, ciphertext, kdf_json, attempts, max, destroyed)) = row else {
        return Err(FetchError::NotFound);
    };
    if destroyed || oprf_key_opt.is_none() {
        return Err(FetchError::Destroyed);
    }
    // This guess pushes us to/over the cap: destroy and refuse.
    if attempts + 1 >= max {
        sqlx::query("UPDATE key_escrow SET oprf_key = NULL, destroyed = true, eval_attempts = eval_attempts + 1, updated_at = NOW() WHERE account_id = $1")
            .bind(account_id)
            .execute(pool)
            .await
            .map_err(|e| FetchError::Db(e.to_string()))?;
        return Err(FetchError::Destroyed);
    }
    // Count this evaluation.
    sqlx::query("UPDATE key_escrow SET eval_attempts = eval_attempts + 1, updated_at = NOW() WHERE account_id = $1")
        .bind(account_id)
        .execute(pool)
        .await
        .map_err(|e| FetchError::Db(e.to_string()))?;

    let oprf_key = oprf_key_opt.unwrap();
    let evaluated_b64 = server_evaluate(&oprf_key, blinded_b64).map_err(FetchError::Crypto)?;
    let kdf_params: KdfParams = serde_json::from_value(kdf_json).map_err(|e| FetchError::Crypto(e.to_string()))?;
    Ok(EscrowFetch { evaluated_b64, ciphertext, kdf_params })
}
```

Note: the `attempts + 1 >= max` rule means with `max_attempts = 5` the server performs at most 4 real evaluations and destroys on the 5th request. Adjust the test's loop bound to match.

- [ ] **Step 2: Add escrow + recovery handlers to `demo/src/server.rs`**

```rust
#[derive(serde::Deserialize)]
pub struct StoreEscrowReq {
    pub enrollment_token: String,
    pub ciphertext_b64: String,
    pub kdf_params: crate::escrow::KdfParams,
    pub oprf_key_b64: String,
}

pub async fn store_escrow_handler(
    State(state): State<ServerState>,
    Json(req): Json<StoreEscrowReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let Some(pool) = state.db.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "database not configured".into()));
    };
    let account_id_str = state.token_signer
        .verify(&req.enrollment_token, "enrollment", crate::protocol::now_unix())
        .map_err(|e| (StatusCode::UNAUTHORIZED, format!("{e:?}")))?;
    let account_id = uuid::Uuid::parse_str(&account_id_str)
        .map_err(|_| (StatusCode::BAD_REQUEST, "bad token subject".into()))?;
    let ciphertext = b64.decode(&req.ciphertext_b64).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let oprf_key = b64.decode(&req.oprf_key_b64).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    crate::escrow::store_escrow(pool, account_id, &ciphertext, &req.kdf_params, &oprf_key)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    crate::accounts::set_status(pool, account_id, "active")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(serde::Deserialize)]
pub struct RecoverStartReq { pub email: String }

pub async fn recover_start_handler(
    State(state): State<ServerState>,
    Json(req): Json<RecoverStartReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    let Some(pool) = state.db.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "database not configured".into()));
    };
    let email_hash = crate::email::hash_email(&req.email);
    // Always send a code if the account exists; respond 204 either way (don't leak existence).
    if crate::accounts::find_account_by_email_hash(pool, &email_hash).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?.is_some()
    {
        let code = crate::email::generate_code();
        crate::email::store_verification(pool, &email_hash, &crate::email::hash_code(&code), "recovery")
            .await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        state.email_sender.send_code(&req.email, &code, "recovery").await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(serde::Deserialize)]
pub struct RecoverVerifyReq { pub email: String, pub code: String }
#[derive(serde::Serialize)]
pub struct RecoverVerifyResp { pub recovery_token: String }

pub async fn recover_verify_handler(
    State(state): State<ServerState>,
    Json(req): Json<RecoverVerifyReq>,
) -> Result<Json<RecoverVerifyResp>, (StatusCode, String)> {
    let Some(pool) = state.db.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "database not configured".into()));
    };
    let email_hash = crate::email::hash_email(&req.email);
    let outcome = crate::email::verify_code(pool, &email_hash, "recovery", &crate::email::hash_code(&req.code))
        .await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if outcome != crate::email::VerifyOutcome::Ok {
        return Err((StatusCode::UNAUTHORIZED, format!("{outcome:?}")));
    }
    let acct = crate::accounts::find_account_by_email_hash(pool, &email_hash).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "account not found".into()))?;
    let token = state.token_signer.issue(&acct.id.to_string(), "recovery", crate::protocol::now_unix(), 900);
    Ok(Json(RecoverVerifyResp { recovery_token: token }))
}

#[derive(serde::Deserialize)]
pub struct EscrowFetchReq { pub recovery_token: String, pub blinded_point: String }
#[derive(serde::Serialize)]
pub struct EscrowFetchResp { pub evaluated_point: String, pub ciphertext_b64: String, pub kdf_params: crate::escrow::KdfParams }

pub async fn escrow_fetch_handler(
    State(state): State<ServerState>,
    Json(req): Json<EscrowFetchReq>,
) -> Result<Json<EscrowFetchResp>, (StatusCode, String)> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let Some(pool) = state.db.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "database not configured".into()));
    };
    let account_id_str = state.token_signer
        .verify(&req.recovery_token, "recovery", crate::protocol::now_unix())
        .map_err(|e| (StatusCode::UNAUTHORIZED, format!("{e:?}")))?;
    let account_id = uuid::Uuid::parse_str(&account_id_str)
        .map_err(|_| (StatusCode::BAD_REQUEST, "bad token subject".into()))?;
    match crate::escrow::fetch_and_evaluate(pool, account_id, &req.blinded_point).await {
        Ok(f) => Ok(Json(EscrowFetchResp {
            evaluated_point: f.evaluated_b64,
            ciphertext_b64: b64.encode(&f.ciphertext),
            kdf_params: f.kdf_params,
        })),
        Err(crate::escrow::FetchError::Destroyed) =>
            Err((StatusCode::GONE, "recovery blob destroyed".into())),
        Err(crate::escrow::FetchError::NotFound) =>
            Err((StatusCode::NOT_FOUND, "no escrow".into())),
        Err(crate::escrow::FetchError::Crypto(e)) =>
            Err((StatusCode::BAD_REQUEST, e)),
        Err(crate::escrow::FetchError::Db(e)) =>
            Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}
```

- [ ] **Step 3: Register the routes** in both router builders:

```rust
        .route("/accounts/escrow", axum::routing::post(store_escrow_handler))
        .route("/accounts/recover/start", axum::routing::post(recover_start_handler))
        .route("/accounts/recover/verify", axum::routing::post(recover_verify_handler))
        .route("/accounts/escrow/fetch", axum::routing::post(escrow_fetch_handler))
```

- [ ] **Step 4: Write the failing integration test (store + correct fetch + exhaustion)**

```rust
#[tokio::test]
async fn test_escrow_store_and_fetch() {
    let Some(pool) = test_pool().await else { return };
    use ainm_demo::{accounts, escrow};

    let uniq = format!("{:x}", uuid::Uuid::new_v4().as_fields().0);
    let username = format!("rec{uniq}");
    let acct = accounts::create_pending_account(&pool, &username, "test.ainm.local", &format!("eh_{uniq}")).await.unwrap();

    // Enroll a known secret with a PIN, store it server-side.
    let secret = [9u8; 32];
    let pin = "135790";
    let enrolled = escrow::enroll(&secret, pin).unwrap();
    escrow::store_escrow(&pool, acct.id, &enrolled.ciphertext, &enrolled.kdf_params, &enrolled.oprf_key).await.unwrap();

    // Correct-PIN recovery round-trip via the server functions.
    let blind = escrow::client_blind(pin, &enrolled.kdf_params).unwrap();
    let fetched = escrow::fetch_and_evaluate(&pool, acct.id, &blind.blinded_b64).await.expect("fetch");
    let recovered = escrow::client_finalize_and_open(pin, &fetched.kdf_params, &blind, &fetched.evaluated_b64, &fetched.ciphertext).unwrap();
    assert_eq!(recovered, secret, "correct PIN recovers the escrowed secret");

    // Exhaustion: with max_attempts=5 the server destroys on the 5th request (after 4 evals).
    // We've already used 1; keep guessing until Destroyed.
    let mut destroyed = false;
    for _ in 0..10 {
        let b = escrow::client_blind("000000", &enrolled.kdf_params).unwrap();
        match escrow::fetch_and_evaluate(&pool, acct.id, &b.blinded_b64).await {
            Ok(_) => continue,
            Err(escrow::FetchError::Destroyed) => { destroyed = true; break; }
            Err(e) => panic!("unexpected error: {e:?}"),
        }
    }
    assert!(destroyed, "escrow must be destroyed after exhausting attempts");

    // Post-destruction fetch fails closed.
    let b = escrow::client_blind(pin, &enrolled.kdf_params).unwrap();
    assert!(matches!(
        escrow::fetch_and_evaluate(&pool, acct.id, &b.blinded_b64).await,
        Err(escrow::FetchError::Destroyed)
    ));
}
```

- [ ] **Step 5: Run build + tests**

Run: `cd demo && cargo build && cargo test --test integration test_escrow_store_and_fetch -- --nocapture`
Expected: build OK, test PASSES.

- [ ] **Step 6: Commit**

```bash
rtk proxy git add demo/src/escrow.rs demo/src/server.rs demo/tests/integration.rs
rtk proxy git commit -m "feat(escrow): store + rate-limited fetch/evaluate + recovery endpoints"
```

---

### Task 13: End-to-end account lifecycle integration test

**Files:**
- Test: `demo/tests/integration.rs` (`test_account_lifecycle_e2e`)

- [ ] **Step 1: Write the full-lifecycle test**

Drives signup → verify → bind key → store escrow → active, then asserts the handshake gate accepts the bound key, then recovers the escrowed key.

```rust
#[tokio::test]
async fn test_account_lifecycle_e2e() {
    let Some(pool) = test_pool().await else { return };
    use ainm_demo::{accounts, email, escrow, signing, identity, auth_token, protocol};
    use base64::Engine;
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;

    let uniq = format!("{:x}", uuid::Uuid::new_v4().as_fields().0);
    let username = format!("e2e{uniq}");
    let address = format!("{username}@test.ainm.local");
    let user_email = format!("{username}@mail.test");
    let email_hash = email::hash_email(&user_email);
    let signer = auth_token::TokenSigner::new(b"e2e-secret");

    // signup + verify
    let acct = accounts::create_pending_account(&pool, &username, "test.ainm.local", &email_hash).await.unwrap();
    let code = email::generate_code();
    email::store_verification(&pool, &email_hash, &email::hash_code(&code), "signup").await.unwrap();
    assert_eq!(email::verify_code(&pool, &email_hash, "signup", &email::hash_code(&code)).await.unwrap(), email::VerifyOutcome::Ok);
    accounts::set_email_verified(&pool, acct.id).await.unwrap();

    // bind key (gated by enrollment token)
    let key = identity::generate_keypair(&address).unwrap();
    let etok = signer.issue(&acct.id.to_string(), "enrollment", protocol::now_unix(), 900);
    let aid = uuid::Uuid::parse_str(&signer.verify(&etok, "enrollment", protocol::now_unix()).unwrap()).unwrap();
    accounts::insert_account_key(&pool, aid, &b64.encode(&key.public_key_bytes), Some("e2e")).await.unwrap();

    // store escrow + activate
    let secret = [3u8; 32];
    let pin = "112358";
    let enrolled = escrow::enroll(&secret, pin).unwrap();
    escrow::store_escrow(&pool, acct.id, &enrolled.ciphertext, &enrolled.kdf_params, &enrolled.oprf_key).await.unwrap();
    accounts::set_status(&pool, acct.id, "active").await.unwrap();

    // handshake gate accepts the bound key
    let found = accounts::find_account_by_address(&pool, &address).await.unwrap().unwrap();
    assert_eq!(found.status, "active");
    let reg = accounts::find_active_key(&pool, found.id).await.unwrap().unwrap();
    let (sig, ts) = signing::sign_init(&key.key_pair, &address).unwrap();
    assert!(signing::verify_init(&b64.decode(&reg.public_key).unwrap(), &address, &ts, &sig, chrono::Utc::now(), 300));

    // recovery restores the same secret
    let blind = escrow::client_blind(pin, &enrolled.kdf_params).unwrap();
    let f = escrow::fetch_and_evaluate(&pool, acct.id, &blind.blinded_b64).await.unwrap();
    let recovered = escrow::client_finalize_and_open(pin, &f.kdf_params, &blind, &f.evaluated_b64, &f.ciphertext).unwrap();
    assert_eq!(recovered, secret);
}
```

- [ ] **Step 2: Run the test**

Run: `cd demo && cargo test --test integration test_account_lifecycle_e2e -- --nocapture`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
rtk proxy git add demo/tests/integration.rs
rtk proxy git commit -m "test(accounts): end-to-end lifecycle (signup->active->connect->recover)"
```

---

## WAVE 4 — `.well-known` identity + final verification

### Task 14: `.well-known/ainm-identity/<username>` attestation

**Files:**
- Modify: `demo/src/server.rs` (handler + route)
- Test: `demo/tests/integration.rs` (`test_well_known_identity`)

- [ ] **Step 1: Add the handler**

Returns the account's address + active public keys. If the server has a federation/server keypair available in `ServerState` (check `state.federation` for a server `Identity`/keypair — the federation context built in `main.rs` holds the `server_keypair`), include a `server_signed_attestation` = base64url Ed25519 signature over the canonical JSON of `{address, public_keys}`. If no server key is available (legacy mode), omit the attestation field.

```rust
use axum::extract::Path;

pub async fn well_known_identity_handler(
    State(state): State<ServerState>,
    Path(username): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let Some(pool) = state.db.as_ref() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "database not configured".into()));
    };
    let address = format!("{username}@{}", state.server_domain);
    let acct = crate::accounts::find_account_by_address(pool, &address).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "unknown account".into()))?;
    let key = crate::accounts::find_active_key(pool, acct.id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let public_keys: Vec<String> = key.map(|k| vec![k.public_key]).unwrap_or_default();

    let mut body = serde_json::json!({
        "address": address,
        "public_keys": public_keys,
    });

    // Optional server attestation if a server keypair is available.
    if let Some(kp) = state.server_signing_keypair() {
        let canonical = serde_json_canonicalizer::to_string(&body)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let sig = kp.sign(canonical.as_bytes());
        body["server_signed_attestation"] = serde_json::json!(
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sig.as_ref())
        );
    }
    Ok(Json(body))
}
```

Add a small accessor `ServerState::server_signing_keypair(&self) -> Option<&ring::signature::Ed25519KeyPair>` that returns the federation server keypair if present, else `None`. Check how the federation context stores the keypair (`FederationContext`) and expose it; if it is wrapped in `Arc<Identity>`, return `Some(&identity.key_pair)`. If wiring the keypair through is non-trivial, it is acceptable for v1 to omit the attestation (return the body without it) and leave a `// TODO(B): sign attestation once server key is in ServerState` — the spec marks cross-server resolution as untested in v1.

- [ ] **Step 2: Register the route** in both router builders:

```rust
        .route("/.well-known/ainm-identity/{username}", axum::routing::get(well_known_identity_handler))
```

- [ ] **Step 3: Write the test**

```rust
#[tokio::test]
async fn test_well_known_identity() {
    let Some(pool) = test_pool().await else { return };
    use ainm_demo::{accounts, identity};
    use base64::Engine;
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;

    let uniq = format!("{:x}", uuid::Uuid::new_v4().as_fields().0);
    let username = format!("wk{uniq}");
    let address = format!("{username}@test.ainm.local");
    let key = identity::generate_keypair(&address).unwrap();
    let acct = accounts::create_pending_account(&pool, &username, "test.ainm.local", &format!("eh_{uniq}")).await.unwrap();
    accounts::insert_account_key(&pool, acct.id, &b64.encode(&key.public_key_bytes), None).await.unwrap();
    accounts::set_status(&pool, acct.id, "active").await.unwrap();

    // Assert the data the handler returns is resolvable: the active key for the address.
    let found = accounts::find_account_by_address(&pool, &address).await.unwrap().unwrap();
    let active = accounts::find_active_key(&pool, found.id).await.unwrap().unwrap();
    assert_eq!(active.public_key, b64.encode(&key.public_key_bytes));
}
```

(The HTTP handler itself is thin glue over these functions; the assertion validates the data path. If a live-server test harness already exists in `integration.rs`, prefer hitting the route over HTTP.)

- [ ] **Step 4: Run build + test**

Run: `cd demo && cargo build && cargo test --test integration test_well_known_identity -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
rtk proxy git add demo/src/server.rs demo/tests/integration.rs
rtk proxy git commit -m "feat(accounts): .well-known/ainm-identity attestation endpoint"
```

---

### Task 15: Full suite green + clippy

**Files:** none (verification only)

- [ ] **Step 1: Run the full test suite**

Run: `cd demo && cargo test 2>&1 | tail -40`
Expected: all tests pass **except** the two known pre-existing failures `test_graph_traversal` and `test_compilation_dedup_updates_confidence`. If any OTHER test fails, it is a regression (most likely an old handshake test not updated in Task 10) — fix it before proceeding.

- [ ] **Step 2: Run clippy and fix warnings in new code**

Run: `cd demo && cargo clippy --all-targets 2>&1 | tail -40`
Expected: no new warnings in `accounts.rs`/`email.rs`/`auth_token.rs`/`escrow.rs`/added `server.rs` code. Fix any.

- [ ] **Step 3: Commit any fixups**

```bash
rtk proxy git add -A demo
rtk proxy git commit -m "chore(accounts): clippy cleanup + suite green"
```

---

## Definition of Done (this plan)

Maps to spec §14 items 1–5, 7 (item 6, iOS, is the follow-up plan):

1. ✅ `003_accounts.sql` creates `accounts`, `account_keys`, `email_verifications`, `key_escrow` (Task 1).
2. ✅ Signup → email-code → verify → key enrollment → escrow → `active` works end-to-end (Tasks 7, 12, 13).
3. ✅ Handshake rejects unknown addresses and key/address mismatches, accepts a correctly bound key (Tasks 9, 10, 13).
4. ✅ Recovery restores the original key on the correct PIN; wrong-PIN exhaustion destroys the escrow and fails closed (Tasks 11, 12, 13).
5. ✅ `email.rs` is trait-based with a mock used in tests; no real emails in CI (Task 4).
7. ✅ Full server suite green except the two known pre-existing failures (Task 15).

**Deferred to the iOS follow-up plan (spec §10, DoD #6):** SwiftUI onboarding (email→code→username→PIN→FaceID) + recovery screens; iOS Ristretto255 OPRF + Argon2 client matching the wire contract frozen above; `InitializeParams` gains the `timestamp` field; `AccountService` REST client. The OPRF wire format and `kdf_params` shape defined in this plan are the iOS client's contract.
