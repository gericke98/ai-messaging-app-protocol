# Per-User Agent Credentials & Quota — Design (sub-project C, v1)

**Date:** 2026-06-02
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)
**Milestone context:** Sub-project C of the hosted multi-tenant runtime initiative. Decomposition:
A = Accounts & identity binding (shipped) → B = Hosted agent runtime (shipped, the moat) →
**C = Per-user agent credentials & quota (this doc)** → D = Isolation/sandboxing → E = Deployment/HA → F = Onboarding UX.

## 1. Goal

After sub-project B, the hosted agent (`assistant@<server-domain>`) runs an in-process tool-calling loop on **one
global server LLM key** (`AINM_LLM__*`) with **no usage cap** — every user who messages the agent spends the
company's budget without limit. Sub-project C closes that by making LLM credentials and usage **per-account**:

- **Credentials (BYO):** a user may store their own OpenAI-compatible API key (encrypted at rest); the hosted agent
  then runs on *their* key, shifting cost to them.
- **Fallback:** if a user has no BYO key, the agent uses the server's default key (as today).
- **Quota:** on the **server-key path**, per-account monthly token usage is metered and capped (fair-use / cost /
  abuse control). The **BYO-key path is metered for visibility but never capped** (the user is paying).

This is the urgent gap B left open (uncapped server spend) plus the cost-shifting lever (BYO keys), as one subsystem.

## 2. What exists today / the gap

(From a codebase survey, file:line evidence.)

- **One global LLM key, loaded once at startup.** `main.rs` reads `AINM_LLM__API_KEY` / `AINM_LLM__MODEL`
  (default `gpt-4o-mini`) / `AINM_LLM__BASE_URL` and builds **two** independent `OpenAiClient`s: one for the memory
  compiler worker, one for the hosted agent (`state.llm_client`). `OpenAiClient::new(api_key, model, base_url)`
  (`llm.rs`) stores the key plaintext in-memory.
- **The agent's LLM call happens at exactly one spot:** `runtime.rs::run_loop` calls
  `llm.chat_with_tools(&messages, &offered)`. `handle_turn` (`runtime_server.rs`) is where the requester identity
  (`requester`), `thread_id`, and resolved consent `scopes` are all in hand — the natural place to select a
  per-user credential and wrap quota enforcement.
- **`chat_with_tools` discards the provider's token `usage`.** It returns only `AssistantTurn`. To meter actual
  spend we must surface the response `usage` block.
- **Accounts exist** (`accounts` table, `AccountRow`, `find_account_by_address`) from sub-project A — the per-user
  identity to attach credentials/quota to. Account REST endpoints already exist and are authenticated via the
  stateless HMAC tokens in `auth_token.rs`.
- **No usage tracking, quota, rate limiting, or billing of any kind exists.** Precedent for an atomic per-account
  counter: the escrow guess-counter in `escrow.rs` (single `UPDATE … RETURNING` with a `CASE`), which C's debit
  reuses in shape.
- **Crypto helpers already present:** `escrow.rs` uses XChaCha20-Poly1305 + HKDF-SHA256 (via the shared
  `ainm-escrow` crate's deps) — C reuses these for encrypting BYO keys at rest. There is otherwise **no
  encryption-at-rest helper**; existing secrets (hosted-agent private key, OPRF key) are plaintext BYTEA.
- **Config** loads via figment: `config/default.toml` then `AINM_*` env with `__` nesting (`config.rs`). `AppConfig`
  currently has `database` + `mcp_servers`.
- **Latest migration is `004_hosted_agents.sql`**; the runner is `sqlx::migrate::Migrator::new("migrations")` in
  `db.rs`, auto-run at pool init. Next file: `005_user_llm_credentials.sql`.

**The gap C closes:** uncapped server spend → metered + capped per account; single shared key → optional per-user
BYO key (encrypted) with server fallback.

## 3. Decisions locked (brainstorming)

- **Scope = both:** BYO credentials *and* server-key quota, with server fallback (largest scope of the three options).
- **Quota unit = tokens, cadence = monthly, hard cap** (provider-agnostic; no price tables; `$` is a derived
  estimate only). USD/cost accounting is out of scope (§9).
- **Architecture = a `MeteredLlmClient` decorator** wrapping the inner (`OpenAiClient`) and implementing `LlmClient`;
  built per-turn in `handle_turn`; `run_loop` and the tool layer are unchanged. (Chosen over inline checks in the
  loop, and over HTTP/tower middleware — the agent loop is in-process, so quota must be enforced at LLM-call
  granularity.)
- **Providers = OpenAI-compatible only** (key + model + base_url) — covers OpenAI and Anthropic-via-gateway, matching
  B. No native multi-provider client in v1.
- **Encryption at rest = XChaCha20-Poly1305**, key = HKDF-SHA256 of a dedicated stable `AINM_LLM_ENCRYPTION_KEY`
  (reusing escrow's crypto). **If that env is unset, BYO storage is disabled** (server-key + quota still work).
- **Accounting from the provider's actual `usage.total_tokens`**, debited atomically after each LLM call; missing
  usage → a configurable fallback debit.
- **Cap semantics = admit-if-under-at-turn-start** (the admitting turn may overshoot by its own usage; no
  pre-reservation).
- **BYO path is metered but never capped.** Quota enforcement applies only to the server-key path.
- **v1 meters only the interactive hosted-agent path**, not the background memory-compiler's server-key usage
  (follow-up).

## 4. Data model (migration `005_user_llm_credentials.sql`)

Two new tables. No changes to existing tables.

**`account_llm_settings`** — one optional row per account, holding the BYO key *and* the budget override together
(both all-optional; a user may have a budget override without a BYO key, or vice versa):

```sql
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
```

**`llm_usage`** — the meter, keyed by requester address + month so reset is automatic (a new month is a new row;
no reset job, no reset race):

```sql
CREATE TABLE llm_usage (
    requester_address TEXT   NOT NULL,           -- the human messaging the agent
    period            TEXT   NOT NULL,           -- 'YYYY-MM' (UTC)
    account_id        UUID   REFERENCES accounts(id) ON DELETE SET NULL,  -- linked when requester is local
    used_tokens       BIGINT NOT NULL DEFAULT 0,
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (requester_address, period)
);
```

Debit is a single atomic upsert (TOCTOU-safe, escrow-counter shape):

```sql
INSERT INTO llm_usage (requester_address, period, account_id, used_tokens)
VALUES ($1, $2, $3, $4)
ON CONFLICT (requester_address, period)
DO UPDATE SET used_tokens = llm_usage.used_tokens + EXCLUDED.used_tokens, updated_at = NOW()
RETURNING used_tokens;
```

## 5. Module boundaries

- **`credentials.rs`** (new) — owns `account_llm_settings`:
  - `CredCipher` — XChaCha20-Poly1305 AEAD; key derived once via HKDF-SHA256 from `AINM_LLM_ENCRYPTION_KEY`.
    `encrypt(plaintext) -> (ciphertext, nonce)` (random 24-byte nonce per call) and `decrypt(ciphertext, nonce) ->
    Result<String>`.
  - Repo: `get_settings(pool, account_id)`, `upsert_byo_key(pool, account_id, ciphertext, nonce, model, base_url)`,
    `clear_byo_key(pool, account_id)`, `get_budget_override(pool, account_id) -> Option<i64>`.
  - `enum Credential { ByoKey { api_key: String, model: Option<String>, base_url: Option<String> }, ServerDefault }`
    and `resolve_effective_credential(pool, cipher: Option<&CredCipher>, account_id: Option<Uuid>) -> Credential`
    (BYO if a decryptable key is present, else `ServerDefault`; a decrypt failure logs a warning and falls back to
    `ServerDefault`).
- **`quota.rs`** (new) — owns `llm_usage`:
  - `current_usage(pool, requester, period) -> i64`.
  - `debit(pool, requester, account_id, period, tokens) -> i64` (the atomic upsert above; returns the new total).
  - `resolve_budget(override_opt: Option<i64>, cfg_default: i64) -> i64`.
  - `current_period(now) -> String` ('YYYY-MM', UTC) — pure, testable (caller passes the timestamp; no hidden clock).
- **`llm.rs`** (extend):
  - `struct TokenUsage { prompt_tokens: u32, completion_tokens: u32, total_tokens: u32 }` (Deserialize).
  - `struct ChatResponse { turn: AssistantTurn, usage: Option<TokenUsage> }`.
  - `LlmClient::chat_with_tools(...) -> Result<ChatResponse, AppError>` (return type changes from `AssistantTurn`).
    `OpenAiClient` parses `json["usage"]` into `TokenUsage`; `MockLlmClient::with_turns` returns `usage: None`
    (a new `with_turns_and_usage` constructor scripts usage for metering tests).
  - `MeteredLlmClient { inner: Arc<dyn LlmClient>, pool, requester, account_id, period, budget, enforce: bool,
    missing_usage_fallback: u32 }` implementing `LlmClient` (see §7).
- **`runtime_server.rs`** (extend) — `handle_turn` resolves the requester's account + credential, builds the inner
  `OpenAiClient` (BYO or server default), wraps it in `MeteredLlmClient`, and passes that `&dyn LlmClient` into
  `run_loop`.
- **`runtime.rs`** (extend) — the single `chat_with_tools` call site now binds `ChatResponse` and uses `.turn`
  (otherwise unchanged; the cap/error logic is untouched).
- **`server.rs`** (extend) — three REST endpoints (§6) + `ServerState` fields: `cred_cipher: Option<Arc<CredCipher>>`,
  `quota: QuotaConfig` (the db pool + default `llm_client` are already present).
- **`config.rs`** (extend) — `QuotaConfig`; the encryption key is read directly from env in `main.rs` (a secret, not a
  figment config value).
- **`main.rs`** (extend) — build `CredCipher` from `AINM_LLM_ENCRYPTION_KEY` (None if unset), load `QuotaConfig`,
  inject both into `ServerState`.

## 6. Credential & usage REST endpoints

Authenticated as the account using the **same mechanism the existing `/accounts/*` endpoints use** (the
`auth_token.rs` HMAC bearer that resolves to the account); plan-phase confirms the exact extraction helper.

- **`PUT /accounts/llm-credential`** — body `{ "api_key": String, "model"?: String, "base_url"?: String }`.
  Encrypts `api_key` with `CredCipher`, upserts `account_llm_settings`. Returns `200 {ok:true}`. Returns
  **`503 {error:"byo_not_configured"}`** if `cred_cipher` is `None` (encryption key unset). The plaintext key is
  never logged and never returned by any endpoint.
- **`DELETE /accounts/llm-credential`** — sets `api_key_ciphertext`/`api_key_nonce`/`model`/`base_url` to NULL
  (keeps any budget override). Returns `200 {ok:true}`.
- **`GET /accounts/usage`** — returns `{ "period": "YYYY-MM", "used_tokens": i64, "budget": i64, "remaining": i64,
  "byo_key_set": bool }`. `budget`/`remaining` reflect the server-key cap; for a BYO user `byo_key_set:true` signals
  the cap does not apply to them.

Per-account budget overrides (`account_llm_settings.monthly_token_budget`) are set administratively (DB) in v1 — no
public endpoint.

## 7. Quota enforcement (`MeteredLlmClient`)

Built per-turn in `handle_turn`. `enforce = quota.enabled && path == ServerDefault` (BYO path: `enforce=false`).
`chat_with_tools(messages, tools)`:

1. **If `enforce`:** read `used = quota::current_usage(pool, requester, period)`. If `used >= budget`, **short-circuit**:
   return `Ok(ChatResponse { turn: AssistantTurn::Text("You've reached your monthly usage limit. It resets on
   <first of next month>."), usage: None })`. No provider call is made; `run_loop` finalizes with this text. (Using a
   `Text` turn rather than an `Err` keeps the limit message precise without threading a new error type through the
   loop, which otherwise maps all `Err`s to a generic "error contacting my model" message.)
2. Call `inner.chat_with_tools(messages, tools).await`.
3. On `Ok(resp)`: `let spent = resp.usage.map(|u| u.total_tokens).unwrap_or(missing_usage_fallback);`
   `quota::debit(pool, requester, account_id, period, spent)` (best-effort; a debit DB error is logged, not fatal).
   Debiting happens on **both** paths (BYO meters for visibility). Return `resp`.
4. On `Err(e)`: propagate (no debit).

Cap semantics: a turn is admitted iff `used < budget` at its start; the admitting turn's own usage is debited after,
so the recorded total may exceed `budget` by up to one turn — acceptable (no pre-reservation). The loop may make
several `chat_with_tools` calls per user turn (tool rounds); each is individually pre-checked and debited, so a
runaway multi-round turn stops as soon as a round starts over budget.

## 8. Config (`AINM_*`)

| Key | Default | Meaning |
|-----|---------|---------|
| `AINM_QUOTA__ENABLED` | `true` | Master switch for server-key quota enforcement. |
| `AINM_QUOTA__MONTHLY_TOKEN_BUDGET` | `200000` | Default monthly token budget per account (server-key path). |
| `AINM_QUOTA__MISSING_USAGE_FALLBACK_TOKENS` | `1000` | Tokens debited when the provider omits a `usage` block. |
| `AINM_LLM_ENCRYPTION_KEY` | *(unset)* | base64 32-byte secret for BYO-key encryption. Unset → BYO disabled. |

`QuotaConfig { enabled: bool, monthly_token_budget: i64, missing_usage_fallback_tokens: u32 }` is added to
`AppConfig` with serde defaults so existing configs still load.

## 9. Error handling

- **Encryption unconfigured** (`cred_cipher == None`): the two key-mutation endpoints return `503 byo_not_configured`;
  `GET /usage` still works; the agent uses the server key + quota normally.
- **Decrypt failure** (rotated/corrupt key): `resolve_effective_credential` logs a warning and returns
  `ServerDefault` — the turn still succeeds on the server key (it is metered/capped). We do **not** fail the turn.
- **Quota exceeded** (server path): polite limit `Text` turn (§7.1); no provider call.
- **Missing provider `usage`:** debit `missing_usage_fallback_tokens` + warn (so missing usage can't mean free
  unlimited spend).
- **Debit DB error:** logged, non-fatal (the reply is not held hostage to the meter write).
- **No DB / `quota.enabled=false`:** no enforcement; the agent uses the server key as in B. (Metering requires the
  pool; with no DB there is no ledger.)
- **Non-account requester** (no local `accounts` row, e.g. federated): BYO is unavailable; usage is metered/capped
  against the default budget keyed by the raw `requester_address` (`account_id` NULL).
- **BYO path:** never blocks; metering is best-effort.

## 10. Testing

**Unit (Rust, no DB):**
- `CredCipher` encrypt→decrypt round-trip; decrypt with wrong key/corrupt nonce fails cleanly.
- `resolve_effective_credential`: BYO present+decryptable → `ByoKey`; absent → `ServerDefault`; present but
  undecryptable → `ServerDefault` (+ warn).
- `quota::current_period` boundaries (month rollover) and `resolve_budget` (override vs default).
- `MeteredLlmClient` with a mock inner: under-budget → passes through and debits `usage.total_tokens`; over-budget →
  returns the limit `Text` turn and **never calls the inner**; BYO path (`enforce=false`) over "budget" still calls
  inner and debits but does not block; missing usage → debits the fallback.
- `OpenAiClient` parses an OpenAI-shaped `usage` block into `TokenUsage`.

**Integration (DB-backed, `server/tests/`, skip-without-DB pattern):**
1. `PUT /accounts/llm-credential` stores a key → assert the stored `api_key_ciphertext` ≠ the plaintext bytes and is
   decryptable back to the original; `GET /usage` reports `byo_key_set:true`.
2. A full metered hosted turn (mock LLM scripting usage) on the **server key** debits `llm_usage`; `GET /usage`
   reflects the new `used_tokens`.
3. An over-budget requester's turn returns the limit message and the agent makes **no** provider call (assert the mock
   inner was not invoked).
4. Month rollover: a debit in a new `period` creates a fresh row (old period untouched).
5. `DELETE /accounts/llm-credential` clears the BYO key; subsequent resolution returns `ServerDefault`.

## 11. Out of scope (v1)

USD/cost accounting + per-model price tables (token budget only); a public per-account budget-override endpoint
(admin/DB only); multi-provider beyond OpenAI-compatible; pre-reservation / streaming token accounting (meter from
final `usage`); KMS/enclave for `AINM_LLM_ENCRYPTION_KEY` (env-based, aligns with D/E); billing/Stripe; metering the
background **memory-compiler's** server-key usage (interactive hosted-agent path only in v1 — compiler metering is a
follow-up); BYO keys for non-account/federated requesters.

## 12. Definition of done

1. Migration `005_user_llm_credentials.sql` creates `account_llm_settings` + `llm_usage`.
2. `credentials.rs` encrypts/decrypts BYO keys (XChaCha20-Poly1305 + HKDF from `AINM_LLM_ENCRYPTION_KEY`) and resolves
   the effective credential (BYO → server fallback), failing safe when the encryption key is unset or a key won't
   decrypt.
3. `quota.rs` provides atomic monthly token debit + usage/budget reads.
4. `llm.rs` surfaces provider token usage via `ChatResponse`; `MeteredLlmClient` enforces the cap on the server-key
   path (polite limit message, no provider call) and meters both paths; `OpenAiClient`/`MockLlmClient` updated.
5. `handle_turn` selects the per-user credential and wraps `MeteredLlmClient`; `run_loop` unchanged except the
   `ChatResponse` call site.
6. REST endpoints `PUT`/`DELETE /accounts/llm-credential` and `GET /accounts/usage` work, authenticated as the
   account; the plaintext key is never logged or returned; `503` when encryption is unconfigured.
7. Unit + DB integration tests (§10) pass; full server suite green except the two known pre-existing failures
   (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`).
