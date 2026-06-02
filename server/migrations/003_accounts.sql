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
