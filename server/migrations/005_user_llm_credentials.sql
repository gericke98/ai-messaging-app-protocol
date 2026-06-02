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
    period            TEXT   NOT NULL CHECK (period ~ '^\d{4}-\d{2}$'),   -- 'YYYY-MM' (UTC)
    account_id        UUID   REFERENCES accounts(id) ON DELETE SET NULL,
    used_tokens       BIGINT NOT NULL DEFAULT 0,
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (requester_address, period)
);
