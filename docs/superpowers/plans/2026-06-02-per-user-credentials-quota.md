# Per-User Agent Credentials & Quota Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the hosted agent's LLM credentials and usage per-account — optional encrypted BYO key with server fallback, and a metered monthly token quota on the server-key path.

**Architecture:** A `MeteredLlmClient` decorator (implements `LlmClient`) wraps the inner `OpenAiClient` (BYO or server default) and is built per-turn in `handle_turn`; `run_loop` is unchanged except for reading `.turn` off a new `ChatResponse`. Two new tables (`account_llm_settings`, `llm_usage`), two new modules (`credentials.rs`, `quota.rs`), and three REST endpoints. BYO keys are encrypted with XChaCha20-Poly1305 (key = HKDF-SHA256 of `AINM_LLM_ENCRYPTION_KEY`), reusing the escrow crate's crypto deps.

**Tech Stack:** Rust 2024 / Axum / sqlx / PostgreSQL / `chacha20poly1305` + `hkdf` + `sha2` (already deps) / `serde_json`. Tests: `cargo test -p ainm-server` + `MockLlmClient` + DB-backed integration with the skip-without-DB pattern.

**Source spec:** `docs/superpowers/specs/2026-06-02-per-user-credentials-quota-design.md`

---

## Conventions for the implementer

- Working dir is the repo root: `/Users/santiagogerickeparga/Proyectos/ai-native-messaging`. Server crate at `server/` (crate `ainm-server` / `ainm_server`). Run tests with `cargo test -p ainm-server`.
- **Branch:** create `feat/credentials-quota` off `main` before Task C-1 (the controller handles this). Never start on `main`.
- **Module visibility:** new modules go in `server/src/lib.rs` with `pub mod` so `server/tests/` can reference them.
- **Two known pre-existing failures** are acceptable: `test_graph_traversal`, `test_compilation_dedup_updates_confidence`. Do not fix them.
- **NEVER log a plaintext API key** (the user's BYO key) or the cipher master key.
- Commit after each task's tests pass, using the message in that task's final step.

### Ground-truth signatures (verified in the codebase — do not re-derive)

```rust
// server/src/auth_token.rs
impl TokenSigner {
    pub fn issue(&self, account_id: &str, purpose: &str, now_unix: i64, ttl_secs: i64) -> String;
    pub fn verify(&self, token: &str, expected_purpose: &str, now_unix: i64) -> Result<String /*account_id*/, TokenError>;
}
// crate::protocol::now_unix() -> i64

// server/src/accounts.rs
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AccountRow { pub id: uuid::Uuid, pub username: String, pub domain: String,
    pub address: String, pub email_hash: String, pub email_verified: bool, pub status: String }
pub async fn find_account_by_address(pool: &PgPool, address: &str) -> Result<Option<AccountRow>, StorageError>;

// server/src/llm.rs (current — chat_with_tools returns AssistantTurn, will become ChatResponse)
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn extract(&self, messages: &[ConversationMessage]) -> Result<ExtractionResult, AppError>;
    async fn chat_with_tools(&self, messages: &[ChatMessage], tools: &[ToolSchema]) -> Result<AssistantTurn, AppError> { /* default Err */ }
}
pub struct OpenAiClient { http, api_key, model, base_url }
impl OpenAiClient { pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self; }
pub struct MockLlmClient { pub results: Mutex<Vec<ExtractionResult>>, pub turns: Mutex<Vec<AssistantTurn>> }
impl MockLlmClient { pub fn new(..); pub fn empty(); pub fn with_turns(turns: Vec<AssistantTurn>) -> Self; }
pub enum AssistantTurn { Text(String), ToolCalls(Vec<ToolCall>) }

// server/src/runtime.rs — the ONE call site in run_loop:
//   let turn = match llm.chat_with_tools(&messages, &offered).await { Ok(t)=>t, Err(e)=>{...graceful...} };
//   match turn { AssistantTurn::Text(text)=>..., AssistantTurn::ToolCalls(calls)=>... }

// server/src/runtime_server.rs
pub async fn handle_turn(state: &ServerState, thread_id: &str, requester: &str, inbound_user_text: &str);
//   pulls (agent, llm, db) = (state.hosted_agent, state.llm_client, state.db); builds registry;
//   calls run_loop(llm.as_ref(), &registry, &agent.system_prompt, &scopes, history, inbound_user_text, 8)

// server/src/server.rs — ServerState (Clone). Relevant existing fields: db: Option<PgPool>,
//   token_signer: TokenSigner, server_domain: String, hosted_agent, llm_client: Option<Arc<dyn LlmClient>>,
//   mcp_providers. Constructed in start, start_with_port, start_federation.
// Existing authed endpoint precedent (bind_key_handler):
//   let acct = state.token_signer.verify(&req.enrollment_token, "enrollment", crate::protocol::now_unix())?;
//   let account_id = uuid::Uuid::parse_str(&acct)?;

// crates/ainm-escrow crypto pattern to mirror:
//   Hkdf::<Sha256>::new(Some(b"ainm-escrow-v1"), ikm).expand(b"wrapping-key", &mut [0u8;32])
//   XChaCha20Poly1305::new((&key).into()); XNonce::from_slice(&nonce_24); cipher.encrypt/.decrypt
```

Deps already in `server/Cargo.toml` (NO new deps needed): `chacha20poly1305 = { version="0.10", features=["alloc"] }`, `hkdf = "0.12"`, `sha2 = "0.10"`, `rand = "0.8"`, `base64 = "0.22"`, `uuid`, `chrono`, `sqlx`.

Migrations: `server/migrations/NNN_*.sql`, run by `sqlx::migrate::Migrator` in `db.rs`. Latest is `004_hosted_agents.sql`; next is `005_user_llm_credentials.sql`.

### Auth decision for C endpoints (v1)
The three endpoints authenticate via `Authorization: Bearer <token>`, verified with `state.token_signer.verify(token, "enrollment", protocol::now_unix())` → `account_id` string → `Uuid`. This reuses the **enrollment** token the client already holds post-signup (the same token `bind_key_handler` accepts), read from the header instead of a body. A dedicated long-lived account auth token is a documented follow-up.

---

## Task C-1: Migration — `account_llm_settings` + `llm_usage`

**Files:**
- Create: `server/migrations/005_user_llm_credentials.sql`

- [ ] **Step 1: Write the migration**

```sql
-- Per-user LLM credentials (BYO key, encrypted) + budget override.
CREATE TABLE account_llm_settings (
    account_id           UUID PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    api_key_ciphertext   BYTEA,            -- NULL = no BYO key (use server key)
    api_key_nonce        BYTEA,            -- XChaCha20-Poly1305 24-byte nonce (NULL iff ciphertext NULL)
    model                TEXT,             -- optional model override for the BYO key
    base_url             TEXT,             -- optional base_url override for the BYO key
    monthly_token_budget BIGINT,           -- NULL = use config default; per-account override otherwise
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Per-requester monthly token usage meter. New month = new row (automatic reset).
CREATE TABLE llm_usage (
    requester_address TEXT   NOT NULL,
    period            TEXT   NOT NULL,           -- 'YYYY-MM' (UTC)
    account_id        UUID   REFERENCES accounts(id) ON DELETE SET NULL,
    used_tokens       BIGINT NOT NULL DEFAULT 0,
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (requester_address, period)
);
```

- [ ] **Step 2: Verify it applies (if a DB is available) + commit**

If a Postgres is configured locally, run the server once (or `cargo test -p ainm-server` which runs migrations on pool init) to confirm the migration applies. If no DB, just confirm SQL syntax by eye.

```bash
git add server/migrations/005_user_llm_credentials.sql
git commit -m "feat(c): migration for account_llm_settings + llm_usage (C-1)"
```

---

## Task C-2: `credentials.rs` — `CredCipher` (encrypt/decrypt)

**Files:**
- Create: `server/src/credentials.rs`
- Modify: `server/src/lib.rs` (add `pub mod credentials;`)
- Test: unit tests inline.

- [ ] **Step 1: Write the failing tests**

Create `server/src/credentials.rs` with this test module at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cipher_roundtrips() {
        let c = CredCipher::new(b"a-32-byte-or-any-length-master-key");
        let (ct, nonce) = c.encrypt("sk-secret-api-key").unwrap();
        assert_ne!(ct, b"sk-secret-api-key"); // ciphertext is not the plaintext
        let pt = c.decrypt(&ct, &nonce).unwrap();
        assert_eq!(pt, "sk-secret-api-key");
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let c1 = CredCipher::new(b"master-key-one");
        let c2 = CredCipher::new(b"master-key-two");
        let (ct, nonce) = c1.encrypt("sk-secret").unwrap();
        assert!(c2.decrypt(&ct, &nonce).is_err());
    }

    #[test]
    fn each_encrypt_uses_fresh_nonce() {
        let c = CredCipher::new(b"k");
        let (_, n1) = c.encrypt("x").unwrap();
        let (_, n2) = c.encrypt("x").unwrap();
        assert_ne!(n1, n2);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p ainm-server -- credentials::tests::cipher`
Expected: FAIL — module/type absent.

- [ ] **Step 3: Implement `CredCipher`**

Top of `server/src/credentials.rs`:

```rust
//! Per-account LLM credentials: encryption-at-rest for BYO API keys + the settings repo.
//!
//! BYO keys are encrypted with XChaCha20-Poly1305; the AEAD key is derived once via
//! HKDF-SHA256 from a stable server secret (AINM_LLM_ENCRYPTION_KEY). Never log plaintext keys.

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;

/// Encrypts/decrypts BYO API keys. Built once at startup from the server master secret.
pub struct CredCipher {
    cipher: XChaCha20Poly1305,
}

impl CredCipher {
    /// Derive the AEAD key from an arbitrary-length master secret via HKDF-SHA256.
    pub fn new(master_secret: &[u8]) -> Self {
        let hk = Hkdf::<Sha256>::new(Some(b"ainm-llm-cred-v1"), master_secret);
        let mut key = [0u8; 32];
        hk.expand(b"cred-key", &mut key).expect("hkdf expand");
        Self { cipher: XChaCha20Poly1305::new((&key).into()) }
    }

    /// Encrypt plaintext → (ciphertext, 24-byte nonce). Fresh random nonce per call.
    pub fn encrypt(&self, plaintext: &str) -> Result<(Vec<u8>, Vec<u8>), String> {
        let mut nonce = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut nonce);
        let ct = self
            .cipher
            .encrypt(XNonce::from_slice(&nonce), plaintext.as_bytes())
            .map_err(|e| format!("encrypt failed: {e}"))?;
        Ok((ct, nonce.to_vec()))
    }

    /// Decrypt (ciphertext, nonce) → plaintext. Errs on wrong key / tampering.
    pub fn decrypt(&self, ciphertext: &[u8], nonce: &[u8]) -> Result<String, String> {
        if nonce.len() != 24 {
            return Err("bad nonce length".into());
        }
        let pt = self
            .cipher
            .decrypt(XNonce::from_slice(nonce), ciphertext)
            .map_err(|_| "decrypt failed".to_string())?;
        String::from_utf8(pt).map_err(|_| "decrypted bytes not utf8".to_string())
    }
}
```

Add to `server/src/lib.rs`: `pub mod credentials;`

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p ainm-server -- credentials::tests`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add server/src/credentials.rs server/src/lib.rs
git commit -m "feat(c): CredCipher BYO-key encryption (C-2)"
```

---

## Task C-3: `credentials.rs` — settings repo + credential resolution

**Files:**
- Modify: `server/src/credentials.rs`
- Test: unit tests inline (the pure resolution logic); DB repo covered by integration C-10.

- [ ] **Step 1: Write the failing test (pure resolution)**

Append to the `tests` module in `server/src/credentials.rs`:

```rust
    #[test]
    fn resolve_returns_server_default_when_no_settings() {
        let cred = resolve_credential(None, None);
        assert!(matches!(cred, Credential::ServerDefault));
    }

    #[test]
    fn resolve_returns_byo_when_key_present_and_decrypts() {
        let cipher = CredCipher::new(b"k");
        let (ct, nonce) = cipher.encrypt("sk-user").unwrap();
        let settings = AccountLlmSettings {
            api_key_ciphertext: Some(ct),
            api_key_nonce: Some(nonce),
            model: Some("gpt-4o".into()),
            base_url: None,
            monthly_token_budget: None,
        };
        let cred = resolve_credential(Some(&cipher), Some(&settings));
        match cred {
            Credential::ByoKey { api_key, model, .. } => {
                assert_eq!(api_key, "sk-user");
                assert_eq!(model.as_deref(), Some("gpt-4o"));
            }
            _ => panic!("expected ByoKey"),
        }
    }

    #[test]
    fn resolve_falls_back_when_key_undecryptable() {
        let good = CredCipher::new(b"good");
        let bad = CredCipher::new(b"bad");
        let (ct, nonce) = good.encrypt("sk-user").unwrap();
        let settings = AccountLlmSettings {
            api_key_ciphertext: Some(ct), api_key_nonce: Some(nonce),
            model: None, base_url: None, monthly_token_budget: None,
        };
        // Wrong cipher → cannot decrypt → ServerDefault (does NOT panic / leak).
        assert!(matches!(resolve_credential(Some(&bad), Some(&settings)), Credential::ServerDefault));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p ainm-server -- credentials::tests::resolve`
Expected: FAIL — types/fn absent.

- [ ] **Step 3: Implement the settings struct, repo, and resolution**

Add to `server/src/credentials.rs` (after `CredCipher`). Add imports `use crate::error::StorageError; use sqlx::PgPool; use uuid::Uuid;` at the top.

```rust
/// One row of account_llm_settings (all fields optional).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AccountLlmSettings {
    pub api_key_ciphertext: Option<Vec<u8>>,
    pub api_key_nonce: Option<Vec<u8>>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub monthly_token_budget: Option<i64>,
}

/// The effective LLM credential for a turn.
pub enum Credential {
    ByoKey { api_key: String, model: Option<String>, base_url: Option<String> },
    ServerDefault,
}

/// Pure: pick the effective credential. BYO if a decryptable key is present, else ServerDefault.
/// A decrypt failure logs a warning and falls back (never panics, never leaks).
pub fn resolve_credential(cipher: Option<&CredCipher>, settings: Option<&AccountLlmSettings>) -> Credential {
    let (Some(cipher), Some(s)) = (cipher, settings) else { return Credential::ServerDefault };
    let (Some(ct), Some(nonce)) = (s.api_key_ciphertext.as_ref(), s.api_key_nonce.as_ref()) else {
        return Credential::ServerDefault;
    };
    match cipher.decrypt(ct, nonce) {
        Ok(api_key) => Credential::ByoKey {
            api_key,
            model: s.model.clone(),
            base_url: s.base_url.clone(),
        },
        Err(_) => {
            tracing::warn!("[cred] stored BYO key failed to decrypt; falling back to server key");
            Credential::ServerDefault
        }
    }
}

/// Fetch an account's settings row, if present.
pub async fn get_settings(pool: &PgPool, account_id: Uuid) -> Result<Option<AccountLlmSettings>, StorageError> {
    let row = sqlx::query_as::<_, AccountLlmSettings>(
        "SELECT api_key_ciphertext, api_key_nonce, model, base_url, monthly_token_budget \
         FROM account_llm_settings WHERE account_id = $1",
    )
    .bind(account_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Upsert the BYO key (and optional model/base_url), preserving any existing budget override.
pub async fn upsert_byo_key(
    pool: &PgPool, account_id: Uuid, ciphertext: &[u8], nonce: &[u8],
    model: Option<&str>, base_url: Option<&str>,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO account_llm_settings (account_id, api_key_ciphertext, api_key_nonce, model, base_url) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (account_id) DO UPDATE SET \
            api_key_ciphertext = EXCLUDED.api_key_ciphertext, \
            api_key_nonce = EXCLUDED.api_key_nonce, \
            model = EXCLUDED.model, base_url = EXCLUDED.base_url, updated_at = NOW()",
    )
    .bind(account_id).bind(ciphertext).bind(nonce).bind(model).bind(base_url)
    .execute(pool).await?;
    Ok(())
}

/// Clear the BYO key (NULL the cipher columns), keeping any budget override.
pub async fn clear_byo_key(pool: &PgPool, account_id: Uuid) -> Result<(), StorageError> {
    sqlx::query(
        "UPDATE account_llm_settings \
         SET api_key_ciphertext = NULL, api_key_nonce = NULL, model = NULL, base_url = NULL, updated_at = NOW() \
         WHERE account_id = $1",
    )
    .bind(account_id).execute(pool).await?;
    Ok(())
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p ainm-server -- credentials::tests`
Expected: PASS (all 6 tests).

- [ ] **Step 5: Commit**

```bash
git add server/src/credentials.rs
git commit -m "feat(c): account_llm_settings repo + credential resolution (C-3)"
```

---

## Task C-4: `quota.rs` — period, budget, atomic ledger

**Files:**
- Create: `server/src/quota.rs`
- Modify: `server/src/lib.rs` (add `pub mod quota;`)
- Test: unit tests inline (pure period/budget); atomic debit covered by integration C-10.

- [ ] **Step 1: Write the failing tests**

Create `server/src/quota.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_is_year_month_utc() {
        let t = chrono::DateTime::parse_from_rfc3339("2026-06-02T13:00:00Z").unwrap().with_timezone(&chrono::Utc);
        assert_eq!(current_period(t), "2026-06");
        let t2 = chrono::DateTime::parse_from_rfc3339("2026-12-31T23:59:59Z").unwrap().with_timezone(&chrono::Utc);
        assert_eq!(current_period(t2), "2026-12");
    }

    #[test]
    fn budget_prefers_override() {
        assert_eq!(resolve_budget(Some(50_000), 200_000), 50_000);
        assert_eq!(resolve_budget(None, 200_000), 200_000);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p ainm-server -- quota::tests`
Expected: FAIL — module/fns absent.

- [ ] **Step 3: Implement**

Top of `server/src/quota.rs`:

```rust
//! Per-requester monthly token usage meter (the llm_usage ledger).

use crate::error::StorageError;
use sqlx::PgPool;
use uuid::Uuid;

/// The 'YYYY-MM' period bucket for a UTC timestamp (pure; caller supplies the clock).
pub fn current_period(now: chrono::DateTime<chrono::Utc>) -> String {
    now.format("%Y-%m").to_string()
}

/// The effective budget: per-account override if set, else the config default.
pub fn resolve_budget(override_opt: Option<i64>, config_default: i64) -> i64 {
    override_opt.unwrap_or(config_default)
}

/// Current tokens used by a requester this period (0 if no row yet).
pub async fn current_usage(pool: &PgPool, requester: &str, period: &str) -> Result<i64, StorageError> {
    let used: Option<i64> = sqlx::query_scalar(
        "SELECT used_tokens FROM llm_usage WHERE requester_address = $1 AND period = $2",
    )
    .bind(requester).bind(period)
    .fetch_optional(pool).await?;
    Ok(used.unwrap_or(0))
}

/// Atomically add `tokens` to the requester's usage for `period`; returns the new total.
/// TOCTOU-safe upsert (escrow-counter shape).
pub async fn debit(
    pool: &PgPool, requester: &str, account_id: Option<Uuid>, period: &str, tokens: i64,
) -> Result<i64, StorageError> {
    let total: i64 = sqlx::query_scalar(
        "INSERT INTO llm_usage (requester_address, period, account_id, used_tokens) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (requester_address, period) \
         DO UPDATE SET used_tokens = llm_usage.used_tokens + EXCLUDED.used_tokens, updated_at = NOW() \
         RETURNING used_tokens",
    )
    .bind(requester).bind(period).bind(account_id).bind(tokens)
    .fetch_one(pool).await?;
    Ok(total)
}
```

Add to `server/src/lib.rs`: `pub mod quota;`

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p ainm-server -- quota::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add server/src/quota.rs server/src/lib.rs
git commit -m "feat(c): quota ledger (period, budget, atomic debit) (C-4)"
```

---

## Task C-5: `llm.rs` — `ChatResponse`/`TokenUsage` + return-type change (keystone refactor)

**Files:**
- Modify: `server/src/llm.rs`
- Modify: `server/src/runtime.rs` (the one call site)
- Test: update the existing `mock_scripts_multi_turn_tool_use` test; everything must compile.

This changes `chat_with_tools` to return `ChatResponse { turn, usage }`. It touches the trait, `OpenAiClient`, `MockLlmClient`, the existing llm.rs tests, and `run_loop`'s call site — all together so the crate compiles.

- [ ] **Step 1: Add the new types**

In `server/src/llm.rs`, add near the other tool-use types:

```rust
/// Token usage reported by the provider for one call.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TokenUsage {
    #[serde(default)] pub prompt_tokens: u32,
    #[serde(default)] pub completion_tokens: u32,
    #[serde(default)] pub total_tokens: u32,
}

/// The result of one chat turn: the assistant turn plus optional usage.
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub turn: AssistantTurn,
    pub usage: Option<TokenUsage>,
}
```

- [ ] **Step 2: Change the trait method signature**

Change the `LlmClient::chat_with_tools` default method return type from `Result<AssistantTurn, AppError>` to `Result<ChatResponse, AppError>`, and the default body's `Err(...)` stays (it's still an `Err`, type still unifies). The default body:

```rust
    async fn chat_with_tools(
        &self,
        _messages: &[ChatMessage],
        _tools: &[ToolSchema],
    ) -> Result<ChatResponse, AppError> {
        Err(AppError::Protocol { code: -32601, message: "chat_with_tools not supported".into() })
    }
```

- [ ] **Step 3: Update `OpenAiClient::chat_with_tools`**

Change its return type to `Result<ChatResponse, AppError>`. After parsing `let json: serde_json::Value = ...`, extract usage and wrap the returned turns. Replace the two `return Ok(AssistantTurn::ToolCalls(calls));` / final `Ok(AssistantTurn::Text(text))` with `ChatResponse`. Concretely, parse usage once:

```rust
        let usage: Option<TokenUsage> = serde_json::from_value(json["usage"].clone()).ok();
```

then the tool-calls branch becomes:

```rust
                if !calls.is_empty() {
                    return Ok(ChatResponse { turn: AssistantTurn::ToolCalls(calls), usage });
                }
```

and the final line becomes:

```rust
        let text = message["content"].as_str().unwrap_or("").to_string();
        Ok(ChatResponse { turn: AssistantTurn::Text(text), usage })
```

(`usage` is `Clone` only if needed twice — it's used in two return paths; add `usage.clone()` in the tool-calls return and keep `usage` for the final, or compute it just before each return. Simplest: compute `let usage = ...;` once and use `usage.clone()` in the early return.)

- [ ] **Step 4: Update `MockLlmClient::chat_with_tools`**

Change its return type to `Result<ChatResponse, AppError>`; wrap the popped turn:

```rust
    async fn chat_with_tools(
        &self,
        _messages: &[ChatMessage],
        _tools: &[ToolSchema],
    ) -> Result<ChatResponse, AppError> {
        let mut turns = self.turns.lock().unwrap();
        if turns.is_empty() {
            Err(AppError::Protocol {
                code: -32099,
                message: "MockLlmClient: chat_with_tools called but turns queue is exhausted".into(),
            })
        } else {
            Ok(ChatResponse { turn: turns.remove(0), usage: None })
        }
    }
```

- [ ] **Step 5: Update the existing llm.rs test**

In `mock_scripts_multi_turn_tool_use`, the results are now `ChatResponse`. Change the assertions to read `.turn`:

```rust
        let t1 = mock.chat_with_tools(&[ChatMessage::user("hi")], &[]).await.unwrap();
        assert!(matches!(t1.turn, AssistantTurn::ToolCalls(_)));
        let t2 = mock.chat_with_tools(&[ChatMessage::user("hi")], &[]).await.unwrap();
        match t2.turn { AssistantTurn::Text(s) => assert!(s.contains("done")), _ => panic!("expected text") }
        let exhausted = mock.chat_with_tools(&[ChatMessage::user("hi")], &[]).await;
        assert!(exhausted.is_err(), "exhausted turns queue should error");
```

- [ ] **Step 6: Update the `run_loop` call site in `runtime.rs`**

Change the call to bind a `ChatResponse` and use `.turn`:

```rust
        let response = match llm.chat_with_tools(&messages, &offered).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("[runtime] LLM error: {e}");
                return TurnOutcome {
                    final_text: "Sorry — I hit an error contacting my model. Please try again.".into(),
                    tool_rounds: rounds,
                    stopped_early: true,
                };
            }
        };
        let turn = response.turn;
        match turn {
            // ... existing AssistantTurn::Text / AssistantTurn::ToolCalls arms unchanged ...
        }
```

- [ ] **Step 7: Build + run the unit suite**

Run: `cargo build -p ainm-server && cargo test -p ainm-server --lib`
Expected: compiles; `mock_scripts_multi_turn_tool_use`, the runtime tests, and the tools/credentials/quota tests all pass.

- [ ] **Step 8: Commit**

```bash
git add server/src/llm.rs server/src/runtime.rs
git commit -m "feat(c): chat_with_tools returns ChatResponse with token usage (C-5)"
```

---

## Task C-6: `MeteredLlmClient` decorator

**Files:**
- Modify: `server/src/llm.rs` (add `MeteredLlmClient`)
- Test: unit tests inline using a mock inner client + an injectable usage store. To avoid a DB in unit tests, the meter is tested against a tiny in-memory `QuotaStore` trait so the decorator logic is DB-free; the real DB path is exercised in C-10.

To keep the decorator unit-testable without Postgres, introduce a minimal trait the decorator depends on, with a DB impl (used in production) and a mock impl (used in tests).

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `server/src/llm.rs`:

```rust
    use std::sync::Mutex as StdMutex;

    struct FakeStore { used: StdMutex<i64>, debited: StdMutex<i64> }
    #[async_trait::async_trait]
    impl QuotaStore for FakeStore {
        async fn current_used(&self) -> i64 { *self.used.lock().unwrap() }
        async fn debit(&self, tokens: i64) { *self.debited.lock().unwrap() += tokens; }
    }

    struct OneTurnInner(StdMutex<u32>); // counts how many times inner was called
    #[async_trait::async_trait]
    impl LlmClient for OneTurnInner {
        async fn extract(&self, _m: &[ConversationMessage]) -> Result<ExtractionResult, AppError> {
            Ok(ExtractionResult { nodes: vec![], edges: vec![] })
        }
        async fn chat_with_tools(&self, _m: &[ChatMessage], _t: &[ToolSchema]) -> Result<ChatResponse, AppError> {
            *self.0.lock().unwrap() += 1;
            Ok(ChatResponse { turn: AssistantTurn::Text("ok".into()), usage: Some(TokenUsage { prompt_tokens: 10, completion_tokens: 5, total_tokens: 15 }) })
        }
    }

    #[tokio::test]
    async fn under_budget_passes_through_and_debits() {
        let inner = std::sync::Arc::new(OneTurnInner(StdMutex::new(0)));
        let store = std::sync::Arc::new(FakeStore { used: StdMutex::new(0), debited: StdMutex::new(0) });
        let m = MeteredLlmClient { inner: inner.clone(), store: store.clone(), budget: 1000, enforce: true, missing_usage_fallback: 1000 };
        let r = m.chat_with_tools(&[], &[]).await.unwrap();
        assert!(matches!(r.turn, AssistantTurn::Text(_)));
        assert_eq!(*inner.0.lock().unwrap(), 1);            // inner WAS called
        assert_eq!(*store.debited.lock().unwrap(), 15);     // actual usage debited
    }

    #[tokio::test]
    async fn over_budget_returns_limit_text_and_skips_inner() {
        let inner = std::sync::Arc::new(OneTurnInner(StdMutex::new(0)));
        let store = std::sync::Arc::new(FakeStore { used: StdMutex::new(5000), debited: StdMutex::new(0) });
        let m = MeteredLlmClient { inner: inner.clone(), store: store.clone(), budget: 1000, enforce: true, missing_usage_fallback: 1000 };
        let r = m.chat_with_tools(&[], &[]).await.unwrap();
        match r.turn { AssistantTurn::Text(s) => assert!(s.to_lowercase().contains("limit")), _ => panic!("expected limit text") }
        assert_eq!(*inner.0.lock().unwrap(), 0);            // inner was NOT called
        assert_eq!(*store.debited.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn byo_path_never_blocks_but_meters() {
        let inner = std::sync::Arc::new(OneTurnInner(StdMutex::new(0)));
        let store = std::sync::Arc::new(FakeStore { used: StdMutex::new(999_999), debited: StdMutex::new(0) });
        let m = MeteredLlmClient { inner: inner.clone(), store: store.clone(), budget: 1000, enforce: false, missing_usage_fallback: 1000 };
        let r = m.chat_with_tools(&[], &[]).await.unwrap();
        assert!(matches!(r.turn, AssistantTurn::Text(_)));  // not blocked despite "over budget"
        assert_eq!(*inner.0.lock().unwrap(), 1);
        assert_eq!(*store.debited.lock().unwrap(), 15);     // still metered
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p ainm-server -- llm::tests::under_budget llm::tests::over_budget llm::tests::byo_path`
Expected: FAIL — `QuotaStore`/`MeteredLlmClient` absent.

- [ ] **Step 3: Implement `QuotaStore` + `MeteredLlmClient`**

Add to `server/src/llm.rs`:

```rust
/// Minimal usage-store seam so MeteredLlmClient is unit-testable without a DB.
#[async_trait::async_trait]
pub trait QuotaStore: Send + Sync {
    async fn current_used(&self) -> i64;
    async fn debit(&self, tokens: i64);
}

/// Decorator that enforces a token budget (server-key path) and meters usage.
pub struct MeteredLlmClient {
    pub inner: std::sync::Arc<dyn LlmClient>,
    pub store: std::sync::Arc<dyn QuotaStore>,
    pub budget: i64,
    pub enforce: bool,
    pub missing_usage_fallback: u32,
}

#[async_trait::async_trait]
impl LlmClient for MeteredLlmClient {
    async fn extract(&self, messages: &[ConversationMessage]) -> Result<ExtractionResult, AppError> {
        self.inner.extract(messages).await
    }

    async fn chat_with_tools(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolSchema],
    ) -> Result<ChatResponse, AppError> {
        if self.enforce && self.store.current_used().await >= self.budget {
            return Ok(ChatResponse {
                turn: AssistantTurn::Text(
                    "You've reached your monthly usage limit. It resets at the start of next month.".into(),
                ),
                usage: None,
            });
        }
        let resp = self.inner.chat_with_tools(messages, tools).await?;
        let spent = resp.usage.as_ref().map(|u| u.total_tokens).unwrap_or(self.missing_usage_fallback);
        self.store.debit(spent as i64).await;
        Ok(resp)
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p ainm-server -- llm::tests`
Expected: PASS (the 3 new + existing mock test).

- [ ] **Step 5: Commit**

```bash
git add server/src/llm.rs
git commit -m "feat(c): MeteredLlmClient decorator + QuotaStore seam (C-6)"
```

---

## Task C-7: Plumb `cred_cipher` + `quota` config through state & startup

**Files:**
- Modify: `server/src/config.rs` (add `QuotaConfig`)
- Modify: `server/src/server.rs` (ServerState fields + `start_federation` params + 3 construction sites)
- Modify: `server/src/main.rs` (build cipher + quota config, inject)
- Test: build green; behavior covered later.

- [ ] **Step 1: Add `QuotaConfig` to config.rs**

```rust
#[derive(Debug, Deserialize, Clone)]
pub struct QuotaConfig {
    #[serde(default = "default_quota_enabled")]
    pub enabled: bool,
    #[serde(default = "default_monthly_token_budget")]
    pub monthly_token_budget: i64,
    #[serde(default = "default_missing_usage_fallback")]
    pub missing_usage_fallback_tokens: u32,
}
impl Default for QuotaConfig {
    fn default() -> Self {
        Self {
            enabled: default_quota_enabled(),
            monthly_token_budget: default_monthly_token_budget(),
            missing_usage_fallback_tokens: default_missing_usage_fallback(),
        }
    }
}
fn default_quota_enabled() -> bool { true }
fn default_monthly_token_budget() -> i64 { 200_000 }
fn default_missing_usage_fallback() -> u32 { 1000 }
```

Add to `AppConfig`:

```rust
    #[serde(default)]
    pub quota: QuotaConfig,
```

If `main.rs` (or anywhere) constructs an `AppConfig { ... }` struct literal (B-9 added one fallback literal), add `quota: QuotaConfig::default(),` to it.

- [ ] **Step 2: Add ServerState fields + thread through start_federation**

In `server/src/server.rs`, add to `ServerState` (after `mcp_providers`):

```rust
    pub cred_cipher: Option<std::sync::Arc<crate::credentials::CredCipher>>,
    pub quota: crate::config::QuotaConfig,
```

In `start` and `start_with_port` construction sites, default them:

```rust
    cred_cipher: None,
    quota: crate::config::QuotaConfig::default(),
```

In `start_federation`, add two parameters at the end of the signature:

```rust
    cred_cipher: Option<std::sync::Arc<crate::credentials::CredCipher>>,
    quota: crate::config::QuotaConfig,
```

and set the fields in its `ServerState { ... }` from those params.

- [ ] **Step 3: Build the cipher + quota in main.rs and pass them**

In `server/src/main.rs`, after the hosted-agent/llm/mcp build block and before the `start_federation(...)` call:

```rust
    // C: BYO-key cipher (None if AINM_LLM_ENCRYPTION_KEY unset → BYO disabled, fail-safe) + quota config.
    let cred_cipher: Option<std::sync::Arc<credentials::CredCipher>> =
        match std::env::var("AINM_LLM_ENCRYPTION_KEY") {
            Ok(b64) => match base64::engine::general_purpose::STANDARD.decode(b64.trim()) {
                Ok(key) => Some(std::sync::Arc::new(credentials::CredCipher::new(&key))),
                Err(e) => {
                    tracing::warn!("[cred] AINM_LLM_ENCRYPTION_KEY not valid base64 ({e}); BYO keys disabled");
                    None
                }
            },
            Err(_) => {
                tracing::info!("[cred] AINM_LLM_ENCRYPTION_KEY unset; BYO LLM keys disabled");
                None
            }
        };
    let quota_config = app_config.quota.clone();
```

Add `mod credentials; mod quota;` to main.rs's module list if it uses unqualified `mod` declarations (it does — match the B-8 pattern that added `mod hosted_agent; mod runtime_server;` etc). Ensure `use base64::Engine;` is in scope in main.rs (add if needed).

Pass `cred_cipher, quota_config` as the two new trailing args to `server::start_federation(...)`.

- [ ] **Step 4: Build**

Run: `cargo build -p ainm-server`
Expected: compiles (fields unused-but-present is fine; C-8 consumes them).

- [ ] **Step 5: Commit**

```bash
git add server/src/config.rs server/src/server.rs server/src/main.rs
git commit -m "feat(c): plumb cred_cipher + quota config through ServerState/startup (C-7)"
```

---

## Task C-8: Wire credential resolution + metering into `handle_turn`

**Files:**
- Modify: `server/src/runtime_server.rs`
- Test: build green; behavior covered by C-10.

`handle_turn` must: resolve the requester's account → load settings → pick credential (BYO/server) → build the inner client → wrap a `MeteredLlmClient` whose `QuotaStore` is a DB-backed adapter → run the loop.

- [ ] **Step 1: Add a DB-backed `QuotaStore` adapter**

In `server/src/runtime_server.rs`, add a small adapter that implements `crate::llm::QuotaStore` over the `quota` module + pool:

```rust
struct DbQuotaStore {
    pool: sqlx::PgPool,
    requester: String,
    account_id: Option<uuid::Uuid>,
    period: String,
}
#[async_trait::async_trait]
impl crate::llm::QuotaStore for DbQuotaStore {
    async fn current_used(&self) -> i64 {
        crate::quota::current_usage(&self.pool, &self.requester, &self.period).await.unwrap_or(0)
    }
    async fn debit(&self, tokens: i64) {
        if let Err(e) = crate::quota::debit(&self.pool, &self.requester, self.account_id, &self.period, tokens).await {
            tracing::warn!("[quota] debit failed for {}: {e}", self.requester);
        }
    }
}
```

- [ ] **Step 2: Resolve credential + build the metered client in `handle_turn`**

Just before the `run_loop(...)` call in `handle_turn`, replace the direct `llm.as_ref()` usage. Insert (using the already-bound `agent`, `llm`, `db`, `requester`, `registry`, `history`, `scopes`):

```rust
    // C: resolve the requester's account, credential, and budget.
    let account = crate::accounts::find_account_by_address(db, requester).await.ok().flatten();
    let account_id = account.as_ref().map(|a| a.id);
    let settings = match account_id {
        Some(id) => crate::credentials::get_settings(db, id).await.ok().flatten(),
        None => None,
    };
    let credential = crate::credentials::resolve_credential(state.cred_cipher.as_deref(), settings.as_ref());

    // Build the inner client: BYO key (fresh OpenAiClient) or the server default.
    let default_model = std::env::var("AINM_LLM__MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
    let (inner, is_byo): (std::sync::Arc<dyn crate::llm::LlmClient>, bool) = match credential {
        crate::credentials::Credential::ByoKey { api_key, model, base_url } => (
            std::sync::Arc::new(crate::llm::OpenAiClient::new(
                api_key, model.unwrap_or(default_model), base_url,
            )),
            true,
        ),
        crate::credentials::Credential::ServerDefault => (llm.clone(), false),
    };

    let now = chrono::Utc::now();
    let period = crate::quota::current_period(now);
    let budget = crate::quota::resolve_budget(
        settings.as_ref().and_then(|s| s.monthly_token_budget),
        state.quota.monthly_token_budget,
    );
    let store = std::sync::Arc::new(DbQuotaStore {
        pool: db.clone(), requester: requester.to_string(), account_id, period,
    });
    let metered = crate::llm::MeteredLlmClient {
        inner,
        store,
        budget,
        enforce: state.quota.enabled && !is_byo, // BYO path meters but never blocks
        missing_usage_fallback: state.quota.missing_usage_fallback_tokens,
    };

    let outcome = run_loop(
        &metered, &registry, &agent.system_prompt, &scopes, history,
        inbound_user_text, 8,
    ).await;
```

Note: `llm` here is `&Arc<dyn LlmClient>` (from `state.llm_client.as_ref()`), so `llm.clone()` clones the `Arc`. Confirm the binding: the destructure is `let (Some(agent), Some(llm), Some(db)) = (state.hosted_agent.as_ref(), state.llm_client.as_ref(), state.db.as_ref())` — so `llm: &Arc<dyn LlmClient>` and `llm.clone()` yields `Arc<dyn LlmClient>`; `db: &PgPool` so `db.clone()` is a pool handle clone. Adjust `.clone()`/deref as the compiler requires.

- [ ] **Step 3: Build + unit suite**

Run: `cargo build -p ainm-server && cargo test -p ainm-server --lib`
Expected: compiles; all unit tests pass.

- [ ] **Step 4: Commit**

```bash
git add server/src/runtime_server.rs
git commit -m "feat(c): per-user credential resolution + metering in handle_turn (C-8)"
```

---

## Task C-9: REST endpoints — set/clear key + usage

**Files:**
- Modify: `server/src/server.rs` (3 handlers + auth helper + routes)
- Modify: `server/src/accounts.rs` (add `find_account_by_id`)
- Test: build green; behavior covered by C-10.

- [ ] **Step 1: Add `find_account_by_id` to accounts.rs**

```rust
pub async fn find_account_by_id(pool: &PgPool, id: uuid::Uuid) -> Result<Option<AccountRow>, StorageError> {
    let row = sqlx::query_as::<_, AccountRow>(
        "SELECT id, username, domain, address, email_hash, email_verified, status \
         FROM accounts WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}
```

- [ ] **Step 2: Add the auth helper + handlers in server.rs**

Add `use axum::http::HeaderMap;` if not present. Add a module-level helper + handlers:

```rust
/// Resolve the calling account from the `Authorization: Bearer <enrollment-token>` header.
fn account_from_bearer(state: &ServerState, headers: &HeaderMap) -> Result<uuid::Uuid, (StatusCode, String)> {
    let raw = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::UNAUTHORIZED, "missing Authorization header".into()))?;
    let token = raw.strip_prefix("Bearer ").ok_or((StatusCode::UNAUTHORIZED, "expected Bearer token".into()))?;
    let acct = state
        .token_signer
        .verify(token, "enrollment", crate::protocol::now_unix())
        .map_err(|e| (StatusCode::UNAUTHORIZED, format!("{e:?}")))?;
    uuid::Uuid::parse_str(&acct).map_err(|_| (StatusCode::BAD_REQUEST, "bad token subject".into()))
}

#[derive(serde::Deserialize)]
pub struct LlmCredentialReq {
    pub api_key: String,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

pub async fn llm_credential_put_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(req): Json<LlmCredentialReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    let account_id = account_from_bearer(&state, &headers)?;
    let pool = state.db.clone().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    let cipher = state
        .cred_cipher
        .clone()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "byo_not_configured".into()))?;
    let (ct, nonce) = cipher.encrypt(&req.api_key).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    crate::credentials::upsert_byo_key(&pool, account_id, &ct, &nonce, req.model.as_deref(), req.base_url.as_deref())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn llm_credential_delete_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, String)> {
    let account_id = account_from_bearer(&state, &headers)?;
    let pool = state.db.clone().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    crate::credentials::clear_byo_key(&pool, account_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn usage_get_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let account_id = account_from_bearer(&state, &headers)?;
    let pool = state.db.clone().ok_or((StatusCode::SERVICE_UNAVAILABLE, "no database".into()))?;
    let account = crate::accounts::find_account_by_id(&pool, account_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "account not found".into()))?;
    let settings = crate::credentials::get_settings(&pool, account_id).await.ok().flatten();
    let period = crate::quota::current_period(chrono::Utc::now());
    let used = crate::quota::current_usage(&pool, &account.address, &period).await.unwrap_or(0);
    let budget = crate::quota::resolve_budget(
        settings.as_ref().and_then(|s| s.monthly_token_budget),
        state.quota.monthly_token_budget,
    );
    let byo_key_set = settings.as_ref().map(|s| s.api_key_ciphertext.is_some()).unwrap_or(false);
    Ok(Json(serde_json::json!({
        "period": period,
        "used_tokens": used,
        "budget": budget,
        "remaining": (budget - used).max(0),
        "byo_key_set": byo_key_set,
    })))
}
```

- [ ] **Step 3: Register the routes**

Find where existing `/accounts/*` routes are registered on the axum `Router` (near where `bind_key_handler` is wired) and add:

```rust
        .route("/accounts/llm-credential", axum::routing::put(llm_credential_put_handler).delete(llm_credential_delete_handler))
        .route("/accounts/usage", axum::routing::get(usage_get_handler))
```

(Match the existing import style for `put`/`get`/`delete` — they may already be imported from `axum::routing`.)

- [ ] **Step 4: Build**

Run: `cargo build -p ainm-server`
Expected: compiles.

- [ ] **Step 5: Commit**

```bash
git add server/src/server.rs server/src/accounts.rs
git commit -m "feat(c): llm-credential + usage REST endpoints (C-9)"
```

---

## Task C-10: Integration tests (DB-backed)

**Files:**
- Create: `server/tests/credentials_quota_test.rs`
- Test: this IS the test file.

Follow the existing `server/tests/` skip-without-DB pattern (reuse the `pool_or_skip` helper shape from `hosted_agent_test.rs`: read `TEST_DATABASE_URL` → `AINM_DATABASE__URL` → `DATABASE_URL`, connect, run `Migrator` from `CARGO_MANIFEST_DIR/migrations`, return `None` to skip).

- [ ] **Step 1: Write the tests**

Create `server/tests/credentials_quota_test.rs`:

```rust
//! Integration tests for per-user credentials + quota (sub-project C).

use ainm_server::{credentials, quota};

async fn pool_or_skip() -> Option<sqlx::PgPool> {
    let url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("AINM_DATABASE__URL"))
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()?;
    let pool = sqlx::PgPool::connect(&url).await.ok()?;
    let migrations = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    sqlx::migrate::Migrator::new(migrations).await.ok()?.run(&pool).await.ok()?;
    Some(pool)
}

/// Create a throwaway active account, return (id, address).
async fn make_account(pool: &sqlx::PgPool, tag: &str) -> (uuid::Uuid, String) {
    let address = format!("c-{tag}-{:x}@ainm.local", uuid::Uuid::new_v4().as_u128());
    let id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO accounts (username, domain, address, email_hash, email_verified, status) \
         VALUES ($1, 'ainm.local', $2, $3, true, 'active') RETURNING id",
    )
    .bind(format!("c-{tag}"))
    .bind(&address)
    .bind(format!("hash-{address}"))
    .fetch_one(pool).await.unwrap();
    (id, address)
}

#[tokio::test]
async fn byo_key_stored_encrypted_and_decryptable() {
    let Some(pool) = pool_or_skip().await else { eprintln!("no DB; skipping"); return; };
    let (account_id, _addr) = make_account(&pool, "byo").await;
    let cipher = credentials::CredCipher::new(b"test-master-secret");

    let (ct, nonce) = cipher.encrypt("sk-user-secret").unwrap();
    credentials::upsert_byo_key(&pool, account_id, &ct, &nonce, Some("gpt-4o"), None).await.unwrap();

    let settings = credentials::get_settings(&pool, account_id).await.unwrap().unwrap();
    // Stored ciphertext is NOT the plaintext.
    assert_ne!(settings.api_key_ciphertext.as_deref().unwrap(), b"sk-user-secret");
    // And it round-trips back to the original via resolve_credential.
    match credentials::resolve_credential(Some(&cipher), Some(&settings)) {
        credentials::Credential::ByoKey { api_key, model, .. } => {
            assert_eq!(api_key, "sk-user-secret");
            assert_eq!(model.as_deref(), Some("gpt-4o"));
        }
        _ => panic!("expected ByoKey"),
    }

    // Clearing removes the key → ServerDefault.
    credentials::clear_byo_key(&pool, account_id).await.unwrap();
    let after = credentials::get_settings(&pool, account_id).await.unwrap().unwrap();
    assert!(after.api_key_ciphertext.is_none());
    assert!(matches!(credentials::resolve_credential(Some(&cipher), Some(&after)), credentials::Credential::ServerDefault));
}

#[tokio::test]
async fn debit_accumulates_and_rolls_over_monthly() {
    let Some(pool) = pool_or_skip().await else { eprintln!("no DB; skipping"); return; };
    let (account_id, addr) = make_account(&pool, "debit").await;

    let t1 = quota::debit(&pool, &addr, Some(account_id), "2026-06", 100).await.unwrap();
    assert_eq!(t1, 100);
    let t2 = quota::debit(&pool, &addr, Some(account_id), "2026-06", 50).await.unwrap();
    assert_eq!(t2, 150);
    assert_eq!(quota::current_usage(&pool, &addr, "2026-06").await.unwrap(), 150);

    // New month = fresh row.
    let t3 = quota::debit(&pool, &addr, Some(account_id), "2026-07", 10).await.unwrap();
    assert_eq!(t3, 10);
    assert_eq!(quota::current_usage(&pool, &addr, "2026-06").await.unwrap(), 150); // untouched
}

#[tokio::test]
async fn metered_client_blocks_over_budget_against_real_ledger() {
    use ainm_server::llm::{AssistantTurn, ChatResponse, ChatMessage, LlmClient, MeteredLlmClient, QuotaStore, ToolSchema, TokenUsage};
    use ainm_server::llm::{ConversationMessage, ExtractionResult};

    let Some(pool) = pool_or_skip().await else { eprintln!("no DB; skipping"); return; };
    let (account_id, addr) = make_account(&pool, "meter").await;

    // Pre-load usage at the budget.
    quota::debit(&pool, &addr, Some(account_id), "2026-06", 1000).await.unwrap();

    struct CallCountInner(std::sync::Mutex<u32>);
    #[async_trait::async_trait]
    impl LlmClient for CallCountInner {
        async fn extract(&self, _m: &[ConversationMessage]) -> Result<ExtractionResult, ainm_server::error::AppError> {
            Ok(ExtractionResult { nodes: vec![], edges: vec![] })
        }
        async fn chat_with_tools(&self, _m: &[ChatMessage], _t: &[ToolSchema]) -> Result<ChatResponse, ainm_server::error::AppError> {
            *self.0.lock().unwrap() += 1;
            Ok(ChatResponse { turn: AssistantTurn::Text("hi".into()), usage: Some(TokenUsage { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 }) })
        }
    }
    struct Store { pool: sqlx::PgPool, addr: String, account_id: uuid::Uuid }
    #[async_trait::async_trait]
    impl QuotaStore for Store {
        async fn current_used(&self) -> i64 { quota::current_usage(&self.pool, &self.addr, "2026-06").await.unwrap_or(0) }
        async fn debit(&self, t: i64) { let _ = quota::debit(&self.pool, &self.addr, Some(self.account_id), "2026-06", t).await; }
    }

    let inner = std::sync::Arc::new(CallCountInner(std::sync::Mutex::new(0)));
    let m = MeteredLlmClient {
        inner: inner.clone(),
        store: std::sync::Arc::new(Store { pool: pool.clone(), addr: addr.clone(), account_id }),
        budget: 1000,
        enforce: true,
        missing_usage_fallback: 1000,
    };
    let r = m.chat_with_tools(&[], &[]).await.unwrap();
    match r.turn { AssistantTurn::Text(s) => assert!(s.to_lowercase().contains("limit")), _ => panic!("expected limit") }
    assert_eq!(*inner.0.lock().unwrap(), 0, "inner must not be called when over budget");
}
```

If `ConversationMessage`/`ExtractionResult`/`error::AppError` are not all `pub` at those paths, adjust the imports to the real public paths (they are used by the existing `MockLlmClient` so they are reachable; confirm exact module paths).

- [ ] **Step 2: Run**

Run: `cargo test -p ainm-server --test credentials_quota_test`
Expected: compiles; tests PASS if a DB is configured, else print "no DB; skipping" and pass.

- [ ] **Step 3: Commit**

```bash
git add server/tests/credentials_quota_test.rs
git commit -m "test(c): credentials + quota integration tests (C-10)"
```

---

## Task C-11: Full suite green + clippy + secret-in-logs audit + DoD

**Files:** any needing a fix surfaced here.

- [ ] **Step 1: Full server suite**

Run: `cargo test -p ainm-server`
Expected: green EXCEPT the two known pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`). Fix any NEW failure.

- [ ] **Step 2: Clippy**

Run: `cargo clippy -p ainm-server --all-targets`
Fix any warnings in the new C files (`credentials.rs`, `quota.rs`, the `llm.rs` additions, `runtime_server.rs` additions, the endpoints). Do not touch unrelated pre-existing warnings.

- [ ] **Step 3: Secret-in-logs audit**

Run: `grep -rn "api_key" server/src/ | grep -iE "tracing|println|eprintln|dbg!|format!\("`
Confirm NO match logs/prints a plaintext BYO `api_key` or the cipher master key. The legitimate uses are: `OpenAiClient` building the `Authorization` header, struct fields, the encrypt call. If any logs the key, remove it. Also confirm `Credential`/`AccountLlmSettings`/`CredCipher` are not `Debug`-printed with the key (`CredCipher` holds no plaintext; `Credential::ByoKey` holds the key — ensure it is never `{:?}`-logged).

- [ ] **Step 4: Confirm DoD (spec §12) by inspection**

Map each of the 7 DoD items to shipped code:
1. Migration → C-1. 2. `credentials.rs` cipher + resolution + fail-safe → C-2/C-3 + C-7 (cipher None when env unset). 3. `quota.rs` atomic debit → C-4. 4. `ChatResponse` + `MeteredLlmClient` + updated clients → C-5/C-6. 5. `handle_turn` credential selection + metering, `run_loop` only the call-site change → C-8/C-5. 6. Endpoints authed, key never logged/returned, 503 when unconfigured → C-9. 7. Suite green minus the two known failures → Step 1.

- [ ] **Step 5: Commit any fixes**

```bash
git add -A
git commit -m "chore(c): full suite green + clippy + secret-in-logs audit (C-11)"
```

---

## Self-Review (completed by plan author)

**Spec coverage:** §4 schema → C-1. §5 `credentials.rs`/`quota.rs`/`MeteredLlmClient`/`runtime_server`/`runtime`/`server`/`config`/`main` → C-2..C-9. §6 endpoints → C-9. §7 enforcement (short-circuit limit text, debit both paths, BYO never blocks, missing-usage fallback) → C-6 + C-8. §8 config → C-7. §9 error handling (503 unconfigured, decrypt-fallback, non-account requester via address-keyed ledger, no-DB) → C-3/C-7/C-8/C-9. §10 testing → C-2/C-3/C-4/C-6/C-10. §11 out-of-scope respected (no USD, no override endpoint, OpenAI-compatible only, meter from final usage, env key, agent path only). §12 DoD → C-11.

**Type consistency:** `ChatResponse { turn, usage }` / `TokenUsage` (C-5) used in `OpenAiClient`/`MockLlmClient`/`MeteredLlmClient`/`run_loop`. `QuotaStore` (C-6) impl'd by `FakeStore`(test)/`DbQuotaStore`(C-8)/test `Store`(C-10). `Credential`/`AccountLlmSettings`/`resolve_credential`/`get_settings`/`upsert_byo_key`/`clear_byo_key` (C-3) used in C-8/C-9/C-10. `current_period`/`resolve_budget`/`current_usage`/`debit` (C-4) used in C-8/C-9/C-10. `cred_cipher`/`quota` ServerState fields (C-7) used in C-8/C-9.

**Known adaptation points (flagged inline, not placeholders):** exact route-registration block in `server.rs` (C-9 Step 3); `main.rs` module-declaration style + any `AppConfig` struct literal (C-7); the precise `.clone()`/deref on `llm`/`db` bindings in `handle_turn` (C-8 Step 2); exact public paths of `ConversationMessage`/`ExtractionResult`/`AppError` for the C-10 test imports. Each is called out at its site.

**Deliberate decision:** C endpoints reuse the **enrollment** bearer token for account auth (zero new token infra; documented v1 limitation — a dedicated long-lived auth token is a follow-up).
