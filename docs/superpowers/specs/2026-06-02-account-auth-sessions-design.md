# Account Auth Sessions — Design (v1)

**Date:** 2026-06-02
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)
**Milestone context:** Follow-up that closes the load-bearing gap left by sub-project C
(Per-User Agent Credentials & Quota): the BYO LLM-credential endpoints reused the
single-use, 15-minute **enrollment** token for auth, so they were unreachable after
onboarding. This adds a real, long-lived, revocable account session credential.

## 1. Goal & threat model

Provide a **per-device account session token** that authenticates the account-management
REST endpoints — `PUT`/`DELETE /accounts/llm-credential` and `GET /accounts/usage` — so a
user can manage their BYO LLM key and view usage *after* onboarding. The token is minted by
proving possession of the account's **bound Ed25519 key** (the only non-spoofable credential
a client holds post-onboarding), and is **DB-backed and individually revocable** (Telegram-style
"Active Sessions"), with a **sliding expiry capped at an absolute ceiling**.

**Threat model (v1):** a leaked/stolen session token, a lost device, and replay of a captured
signature. Mitigations: tokens are opaque high-entropy secrets stored only as a hash; every
session is individually revocable; the mint challenge is domain-separated to prevent cross-protocol
signature replay; time-based expiry bounds an unnoticed leak.

**Non-goals (v1):** the iOS "Active Sessions" UI and onboarding auto-mint wiring (→ sub-project F;
this spec defines the server contract only); rate-limiting the mint endpoint (note for E/hardening);
refresh tokens (re-minting via the bound key replaces them); device fingerprinting beyond the
optional client-supplied label.

**Decisions locked (brainstorming):** DB-backed revocable model (not stateless HMAC) to match the
Telegram per-device-revoke north star; full session-management surface now (mint + list + revoke-one
+ revoke-all-others); sliding expiry + absolute cap.

## 2. Data model — migration `006_account_auth_tokens.sql`

```sql
CREATE TABLE account_auth_tokens (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id   UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    token_hash   BYTEA NOT NULL UNIQUE,   -- SHA-256 of the opaque token; plaintext NEVER stored
    device_label TEXT,                    -- optional, client-supplied ("iPhone 15")
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at   TIMESTAMPTZ NOT NULL,    -- sliding; capped at created_at + 1 year
    revoked_at   TIMESTAMPTZ              -- NULL = active
);
CREATE INDEX idx_aat_account ON account_auth_tokens(account_id);
-- the UNIQUE on token_hash already indexes the per-request lookup path
```

A session is **active** iff `revoked_at IS NULL AND expires_at > now()`.

## 3. Token format & hashing

- Opaque token = **32 bytes** from a CSPRNG (`rand::rngs::OsRng`), encoded URL-safe-base64-no-pad
  (~256 bits of entropy).
- The server stores only `SHA-256(token_bytes)` in `token_hash` (`ring::digest::SHA256`). The
  plaintext token is returned to the client **exactly once** (at mint), never persisted, never logged.
- Per-request lookup hashes the presented bearer string and `SELECT`s by `token_hash`.

## 4. Mint — `POST /accounts/auth/token`

Request body:
```json
{ "identity": "alice@ainm.local", "timestamp": "2026-06-02T12:00:00Z",
  "signature": "<base64url>", "device_label": "iPhone 15" }
```

- **Auth = signed challenge** over a **domain-separated** payload:
  `"ainm-auth-token|{identity}|{timestamp}"`. The distinct prefix ensures a WS-handshake signature
  (`"{identity}|{timestamp}"`, see `verify_init`) cannot be replayed to mint a session, and a mint
  signature cannot be replayed as a handshake. Freshness window: ±300s (matches `verify_init`).
- Server flow: `find_account_by_address(identity)` → `find_active_key(account_id)` → verify the
  signature against that public key via the new `signing::verify_auth_challenge` → on success,
  generate the opaque token, insert a row with `expires_at = now + 90 days`, return:
```json
{ "token": "<opaque>", "expires_at": "2026-09-02T12:00:00Z", "session_id": "<uuid>" }
```
- New pure function `signing::verify_auth_challenge(public_key_bytes, identity, timestamp,
  signature_b64, now, max_skew_secs) -> bool` — identical structure to `verify_init` but signs/verifies
  the `"ainm-auth-token|…"` payload. `verify_init` is left untouched. A `sign_auth_challenge` test
  helper (mirroring `sign_init`) is added for tests / future iOS reference.

## 5. Verification path — rewrite `account_from_bearer`

`account_from_bearer(state, headers) -> Result<Uuid, (StatusCode, String)>` is rewritten to:

1. Read `Authorization: Bearer <token>` (missing/garbled → 401).
2. `SHA-256` the token, `find_by_hash`.
3. Reject with a **uniform** 401 `"invalid session"` if: not found, `revoked_at IS NOT NULL`, or
   `expires_at <= now`. (No distinction leaked between the cases.)
4. **Sliding bump** (one UPDATE): `last_used_at = now`, `expires_at = LEAST(now + 90 days,
   created_at + 1 year)`. The absolute cap means an actively-used session stays alive but nothing
   survives past one year without a re-mint.
5. Return a small `Caller { account_id, token_hash }` (the hash is needed by the session-management
   endpoints to flag the *current* session and to *keep* it during revoke-others). A thin wrapper
   `account_from_bearer(...) -> Result<Uuid, _>` returning just `Caller.account_id` is kept for the
   three C endpoints that only need the account.

The three C endpoints (`llm_credential_put_handler`, `llm_credential_delete_handler`,
`usage_get_handler`) authenticate via this and **no longer accept enrollment tokens**. (Enrollment
tokens remain valid only for `bind_key_handler`, their real purpose.)

## 6. Session-management endpoints (bearer-gated by a valid session)

All four resolve the caller via `account_from_bearer` *except* mint (signature-gated). Managing
sessions requires already holding a valid session.

- `GET /accounts/auth/sessions` → the caller's **active** sessions:
  `[{ "id", "device_label", "created_at", "last_used_at", "expires_at" }]`. Never returns the token
  or hash. The session used to make the request is flagged (`"current": true`) so the UI can mark it.
- `DELETE /accounts/auth/sessions/{id}` → revoke one. Sets `revoked_at = now` **only if the row's
  `account_id` matches the caller**; otherwise 404 (don't reveal other accounts' session ids).
  Revoking an already-revoked/expired/nonexistent owned id is idempotent (204).
- `DELETE /accounts/auth/sessions/others` → revoke all the caller's active sessions **except the
  current one** (identified by the presented bearer's hash). Returns `{ "revoked": <count> }`.

**Recovery from a lost device:** mint a fresh session via the bound key, then call revoke-all-others.

## 7. Module boundaries

- **`server/src/auth_session.rs`** (new) — the repository + pure helpers:
  - `generate_token() -> (plaintext: String, hash: Vec<u8>)` (CSPRNG + SHA-256) — pure, testable.
  - `hash_token(token: &str) -> Vec<u8>` — pure.
  - `insert(pool, account_id, &hash, device_label, expires_at) -> Result<Uuid, StorageError>`
  - `find_active_by_hash(pool, &hash, now) -> Result<Option<AuthSession>, StorageError>`
  - `slide(pool, id, now, new_expires) -> Result<(), StorageError>` (the per-request bump)
  - `list_active(pool, account_id, now) -> Result<Vec<AuthSession>, StorageError>`
  - `revoke_one(pool, account_id, id, now) -> Result<bool, StorageError>` (false → not owned/none)
  - `revoke_others(pool, account_id, &keep_hash, now) -> Result<u64, StorageError>`
  - `AuthSession { id, account_id, device_label, created_at, last_used_at, expires_at, revoked_at }`
    — NO `Debug` exposure of `token_hash` (the struct holds no plaintext; hash is internal).
  - Constants: `SESSION_TTL_DAYS = 90`, `SESSION_ABSOLUTE_CAP_DAYS = 365`.
- **`server/src/signing.rs`** — add `verify_auth_challenge` + `sign_auth_challenge` (test helper).
- **`server/src/server.rs`** — add `auth_token_mint_handler`, `sessions_list_handler`,
  `session_revoke_handler`, `sessions_revoke_others_handler`; rewrite `account_from_bearer` to call
  the new repo (it needs the presented token's hash to flag `current` / `keep` — so it returns
  `(account_id, token_hash)` internally, or a small `Caller { account_id, token_hash }`); register
  the 4 routes on all 3 routers (the same place the C endpoints are mounted).
- **`server/src/auth_token.rs`** — unchanged (HMAC enrollment/recovery stays single-purpose,
  short-lived; still used by `bind_key_handler` and the recovery flow).
- Register `pub mod auth_session;` in `lib.rs` and `mod auth_session;` in `main.rs`.

## 8. Error handling

- **Mint:** bad/stale/wrong-domain signature, unknown identity, or no active bound key → uniform
  `401 "authentication failed"` (no account enumeration). DB unavailable → 503. Malformed body → 400.
- **Verify (`account_from_bearer`):** missing/garbled header, unknown/revoked/expired token → uniform
  `401 "invalid session"`.
- **Revoke:** non-owned or nonexistent session id → 404. Owned id (any state) → 204 (idempotent).
- **Logging:** the plaintext token and `token_hash` are **never** logged. Log only `session_id` +
  `account_id` at debug. (Consistent with the project rule that secrets/keys are never logged.)

## 9. Testing

**Unit (no DB):**
- `generate_token`: two calls differ; plaintext decodes to 32 bytes; `hash_token(plaintext)` equals
  the returned hash; hashing is deterministic.
- `verify_auth_challenge`: accepts a valid `sign_auth_challenge` signature; rejects tampered identity,
  wrong key, stale timestamp (outside ±300s), and a **`verify_init`-style payload without the
  `ainm-auth-token` prefix** (proves domain separation).
- sliding-cap math: a pure helper `next_expiry(created_at, now) = min(now+90d, created_at+365d)`
  returns `now+90d` early in life and clamps to `created_at+365d` near the ceiling.

**Integration (live Postgres, `server/tests/`):**
- Mint with a valid signature → 200 + token; `GET sessions` shows exactly 1 (flagged current).
- Use the token on `GET /accounts/usage` → 200 (closes the C gap: an account-management endpoint is
  reachable post-onboarding).
- Full BYO round-trip: mint → `PUT /accounts/llm-credential` → 204 (previously impossible).
- `DELETE /accounts/auth/sessions/{id}` then reuse the token → 401.
- Two mints → `DELETE …/sessions/others` from session A → session B is 401, A still 200.
- An **enrollment** token presented to `GET /accounts/usage` → 401 (the stopgap is closed).
- An expired session (insert with `expires_at` in the past) → 401.
- Mint with a tampered/stale signature → 401.

## 10. Definition of done

1. Migration `006_account_auth_tokens.sql` creates the table + index; existing migrations unaffected.
2. `auth_session.rs` provides the repo + pure token-gen/hash helpers + the sliding-cap helper, with
   `token_hash` never exposed in `Debug` output.
3. `signing::verify_auth_challenge` (+ `sign_auth_challenge` test helper) enforces the domain-separated
   challenge; `verify_init` is unchanged.
4. `POST /accounts/auth/token` mints a session by verifying the bound-key signature; returns the
   opaque token + `expires_at` once.
5. `account_from_bearer` is rewritten to validate the DB-backed session (reject revoked/expired,
   slide on use) and the three C endpoints authenticate via it, no longer accepting enrollment tokens.
6. `GET /accounts/auth/sessions`, `DELETE /accounts/auth/sessions/{id}`, and
   `DELETE /accounts/auth/sessions/others` work and are scoped to the caller's account.
7. Plaintext token + hash are never logged or returned after mint.
8. Unit tests + integration tests (against live Postgres) pass; full server suite green except the two
   known pre-existing failures (`test_graph_traversal`, `test_compilation_dedup_updates_confidence`).
