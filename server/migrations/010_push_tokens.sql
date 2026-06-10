-- APNs device tokens (M4 push notifications). One row per device token; a
-- token re-registered by another account moves to it (device changed owner).
-- 008 remains reserved for the agent-grant migration (M3, gated).
CREATE TABLE push_tokens (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id   UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    device_token TEXT NOT NULL UNIQUE,
    platform     TEXT NOT NULL DEFAULT 'ios' CHECK (platform IN ('ios')),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_push_tokens_account ON push_tokens(account_id);
