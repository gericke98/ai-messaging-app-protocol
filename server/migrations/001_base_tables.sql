-- AINM Production Schema: Base Tables
-- Migrates volatile HashMap state to persistent PostgreSQL storage

CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- Thread metadata (previously in-memory ThreadRegistry)
CREATE TABLE threads (
    id          TEXT PRIMARY KEY,
    origin_domain TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata    JSONB DEFAULT '{}'
);

-- Message history (previously redb THREAD_MESSAGES for backfill only)
CREATE TABLE messages (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    thread_id   TEXT NOT NULL REFERENCES threads(id),
    sequence    BIGINT NOT NULL,
    from_addr   TEXT NOT NULL,
    content     JSONB NOT NULL,
    signature   TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(thread_id, sequence)
);
CREATE INDEX idx_messages_thread_seq ON messages(thread_id, sequence);

-- Archival facts (previously volatile HashMap in ServerState)
CREATE TABLE archival_facts (
    fact_id         TEXT PRIMARY KEY,
    participant_addr TEXT NOT NULL,
    thread_id       TEXT,
    key             TEXT NOT NULL,
    value           JSONB NOT NULL,
    tags            JSONB DEFAULT '[]',
    stored_by       TEXT NOT NULL,
    stored_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(participant_addr, key)
);
CREATE INDEX idx_archival_participant ON archival_facts(participant_addr);
CREATE INDEX idx_archival_key_prefix ON archival_facts(participant_addr, key text_pattern_ops);
CREATE INDEX idx_archival_tags ON archival_facts USING GIN(tags);
