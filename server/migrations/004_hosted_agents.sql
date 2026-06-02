-- Hosted first-party agent: server-held identity + config.
-- v1 holds exactly one row (assistant@<server-domain>).
CREATE TABLE hosted_agents (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    address       TEXT NOT NULL UNIQUE,
    public_key    TEXT NOT NULL,              -- base64url(no-pad) Ed25519 public key
    private_key   BYTEA NOT NULL,             -- PKCS#8 v2 document bytes (server-held secret)
    system_prompt TEXT NOT NULL,
    enabled       BOOLEAN NOT NULL DEFAULT true,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
