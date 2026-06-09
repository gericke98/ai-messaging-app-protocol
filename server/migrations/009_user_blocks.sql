-- UGC safety controls (App Store guideline 1.2): block users, report content.
-- 008 is reserved for the agent-grant migration (docs/superpowers/plans/
-- 2026-06-09-agent-grant-server.md); sqlx does not require contiguous versions.

CREATE TABLE user_blocks (
    blocker_addr TEXT NOT NULL,
    blocked_addr TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (blocker_addr, blocked_addr)
);

-- Delivery-time lookup: "who blocks this sender?"
CREATE INDEX user_blocks_blocked_idx ON user_blocks (blocked_addr);

CREATE TABLE content_reports (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    reporter_addr TEXT NOT NULL,
    subject_addr  TEXT NOT NULL,
    thread_id     TEXT,
    message_id    TEXT,
    reason        TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'open',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX content_reports_status_idx ON content_reports (status, created_at);
