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
