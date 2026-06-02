-- AINM Compound Memory: Knowledge Graph Tables
-- User-owned memory graph with temporal validity, confidence scoring, and provenance

-- Memory nodes: entities in the user's knowledge graph
CREATE TABLE memory_nodes (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_addr       TEXT NOT NULL,
    node_type       VARCHAR(50) NOT NULL CHECK (node_type IN ('semantic', 'episodic', 'procedural')),
    category        VARCHAR(50) NOT NULL,
    label           TEXT NOT NULL,
    properties      JSONB DEFAULT '{}',
    confidence      REAL NOT NULL DEFAULT 1.0 CHECK (confidence >= 0.0 AND confidence <= 1.0),
    valid_from      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    valid_until     TIMESTAMPTZ,
    superseded_by   UUID REFERENCES memory_nodes(id),
    provenance      JSONB NOT NULL DEFAULT '{}',
    tags            JSONB DEFAULT '[]',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    search_vector   tsvector GENERATED ALWAYS AS (
        to_tsvector('english', label || ' ' || coalesce(properties::text, ''))
    ) STORED
);

-- Memory edges: directed relationships between nodes
CREATE TABLE memory_edges (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_addr       TEXT NOT NULL,
    source_id       UUID NOT NULL REFERENCES memory_nodes(id) ON DELETE CASCADE,
    target_id       UUID NOT NULL REFERENCES memory_nodes(id) ON DELETE CASCADE,
    relationship    VARCHAR(64) NOT NULL,
    weight          REAL NOT NULL DEFAULT 1.0 CHECK (weight >= 0.0 AND weight <= 1.0),
    confidence      REAL NOT NULL DEFAULT 1.0 CHECK (confidence >= 0.0 AND confidence <= 1.0),
    valid_from      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    valid_until     TIMESTAMPTZ,
    properties      JSONB DEFAULT '{}',
    provenance      JSONB NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT no_self_loop CHECK (source_id != target_id)
);

-- Node indexes
CREATE INDEX idx_mn_user_type ON memory_nodes(user_addr, node_type) WHERE valid_until IS NULL;
CREATE INDEX idx_mn_user_category ON memory_nodes(user_addr, category) WHERE valid_until IS NULL;
CREATE INDEX idx_mn_search ON memory_nodes USING GIN(search_vector);
CREATE INDEX idx_mn_props ON memory_nodes USING GIN(properties jsonb_path_ops);
CREATE INDEX idx_mn_temporal ON memory_nodes(user_addr, valid_from DESC, valid_until);
CREATE INDEX idx_mn_user_current ON memory_nodes(user_addr) WHERE valid_until IS NULL;

-- Edge indexes
CREATE INDEX idx_me_source ON memory_edges(source_id, relationship) WHERE valid_until IS NULL;
CREATE INDEX idx_me_target ON memory_edges(target_id, relationship) WHERE valid_until IS NULL;
CREATE INDEX idx_me_user ON memory_edges(user_addr) WHERE valid_until IS NULL;

-- Compilation tracking
CREATE TABLE compilation_passes (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_addr       TEXT NOT NULL,
    source_threads  JSONB NOT NULL,
    nodes_created   INT NOT NULL DEFAULT 0,
    nodes_updated   INT NOT NULL DEFAULT 0,
    nodes_superseded INT NOT NULL DEFAULT 0,
    edges_created   INT NOT NULL DEFAULT 0,
    started_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at    TIMESTAMPTZ
);

-- Briefing audit log
CREATE TABLE briefing_log (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_addr       TEXT NOT NULL,
    agent_addr      TEXT NOT NULL,
    thread_id       TEXT NOT NULL,
    scope           VARCHAR(20) NOT NULL,
    consent_level   VARCHAR(20) NOT NULL,
    node_count      INT NOT NULL,
    generated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Memory access grants
CREATE TABLE memory_grants (
    user_addr       TEXT NOT NULL,
    agent_addr      TEXT NOT NULL,
    scopes          JSONB NOT NULL DEFAULT '[]',
    consent_level   VARCHAR(20) NOT NULL DEFAULT 'standard',
    granted_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_addr, agent_addr)
);
