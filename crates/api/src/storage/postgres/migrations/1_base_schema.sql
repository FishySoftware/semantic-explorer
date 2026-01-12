-- ============================================================================
-- CONSOLIDATED MIGRATION: Complete Database Schema
-- ============================================================================
-- This single migration file consolidates all schema changes including:
-- 1. Base schema (collections, datasets, transforms, chat, audit)
-- 2. Row-level security policies
-- 3. OIDC session management
-- 4. Performance optimization indexes
--
-- Purpose: Fresh database setup from scratch
-- Idempotent: All CREATE TABLE/INDEX use IF NOT EXISTS
-- ============================================================================

-- ============================================================================
-- Collections & Datasets
-- ============================================================================

CREATE TABLE IF NOT EXISTS collections (
    collection_id    SERIAL PRIMARY KEY,
    title            TEXT                     NOT NULL,
    details          TEXT                     NULL,
    owner            TEXT                     NOT NULL,
    bucket           TEXT                     NOT NULL,
    tags             TEXT[]                   NOT NULL DEFAULT '{}',
    is_public        BOOLEAN                  NOT NULL DEFAULT FALSE,
    created_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Optimized indexes for collections
CREATE INDEX IF NOT EXISTS idx_collections_owner_created ON collections(owner, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_collections_is_public ON collections(is_public) WHERE is_public = TRUE;
CREATE INDEX IF NOT EXISTS idx_collections_title_tsvector 
  ON collections USING GIN (to_tsvector('english', title));
CREATE INDEX IF NOT EXISTS idx_collections_tags 
  ON collections USING GIN (tags);

CREATE TABLE IF NOT EXISTS datasets (
    dataset_id       SERIAL PRIMARY KEY,
    title            TEXT                     NOT NULL,
    details          TEXT                     NULL,
    owner            TEXT                     NOT NULL,
    tags             TEXT[]                   NOT NULL DEFAULT '{}',
    is_public        BOOLEAN                  NOT NULL DEFAULT FALSE,
    created_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Optimized indexes for datasets
CREATE INDEX IF NOT EXISTS idx_datasets_owner_created ON datasets(owner, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_datasets_is_public ON datasets(is_public) WHERE is_public = TRUE;
CREATE INDEX IF NOT EXISTS idx_datasets_title_tsvector 
  ON datasets USING GIN (to_tsvector('english', title));
CREATE INDEX IF NOT EXISTS idx_datasets_tags 
  ON datasets USING GIN (tags);

CREATE TABLE IF NOT EXISTS dataset_items (
    item_id          SERIAL PRIMARY KEY,
    dataset_id       INTEGER                  NOT NULL REFERENCES datasets(dataset_id) ON DELETE CASCADE,
    title            TEXT                     NOT NULL,
    chunks           JSONB                    NOT NULL,
    metadata         JSONB                    NOT NULL DEFAULT '{}',
    created_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Optimized indexes for dataset_items
CREATE INDEX IF NOT EXISTS idx_dataset_items_dataset_created ON dataset_items(dataset_id, created_at DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_dataset_items_dataset_title_unique ON dataset_items(dataset_id, title);
CREATE INDEX IF NOT EXISTS idx_dataset_items_name 
  ON dataset_items(title) 
  WHERE title IS NOT NULL;

-- ============================================================================
-- Embedders & LLMs
-- ============================================================================

CREATE TABLE IF NOT EXISTS embedders (
    embedder_id          SERIAL PRIMARY KEY,
    name                 TEXT                     NOT NULL,
    owner                TEXT                     NOT NULL,
    provider             TEXT                     NOT NULL,
    base_url             TEXT                     NOT NULL,
    api_key_encrypted    TEXT                     NULL,
    config               JSONB                    NOT NULL DEFAULT '{}',
    batch_size           INTEGER                  NOT NULL DEFAULT 100,
    max_batch_size       INTEGER                  NOT NULL DEFAULT 96,
    dimensions           INTEGER                  NOT NULL DEFAULT 1536,
    max_input_tokens     INTEGER                  NOT NULL DEFAULT 8191,
    truncate_strategy    VARCHAR(50)              NOT NULL DEFAULT 'NONE',
    collection_name      TEXT                     NULL,
    is_public            BOOLEAN                  NOT NULL DEFAULT FALSE,
    created_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Optimized indexes for embedders
CREATE INDEX IF NOT EXISTS idx_embedders_owner_public ON embedders(owner, is_public);
CREATE INDEX IF NOT EXISTS idx_embedders_provider ON embedders(provider);
CREATE INDEX IF NOT EXISTS idx_embedders_is_public ON embedders(is_public) WHERE is_public = TRUE;
CREATE INDEX IF NOT EXISTS idx_embedders_owner_created 
  ON embedders(owner, created_at DESC);

CREATE TABLE IF NOT EXISTS llms (
    llm_id               SERIAL PRIMARY KEY,
    name                 TEXT                     NOT NULL,
    owner                TEXT                     NOT NULL,
    provider             TEXT                     NOT NULL,
    base_url             TEXT                     NOT NULL,
    api_key_encrypted    TEXT                     NULL,
    model                TEXT                     NOT NULL DEFAULT 'gpt-4',
    config               JSONB                    NOT NULL DEFAULT '{}',
    is_public            BOOLEAN                  NOT NULL DEFAULT FALSE,
    created_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Optimized indexes for LLMs
CREATE INDEX IF NOT EXISTS idx_llms_owner_public ON llms(owner, is_public);
CREATE INDEX IF NOT EXISTS idx_llms_provider ON llms(provider);
CREATE INDEX IF NOT EXISTS idx_llms_is_public ON llms(is_public) WHERE is_public = TRUE;
CREATE INDEX IF NOT EXISTS idx_llms_owner_created 
  ON llms(owner, created_at DESC);

-- ============================================================================
-- Transform Pipeline
-- ============================================================================

CREATE TABLE IF NOT EXISTS collection_transforms (
    collection_transform_id SERIAL PRIMARY KEY,
    title                   TEXT                     NOT NULL,
    collection_id           INTEGER                  NOT NULL REFERENCES collections(collection_id) ON DELETE CASCADE,
    dataset_id              INTEGER                  NOT NULL REFERENCES datasets(dataset_id) ON DELETE CASCADE,
    owner                   TEXT                     NOT NULL,
    is_enabled              BOOLEAN                  NOT NULL DEFAULT TRUE,
    chunk_size              INTEGER                  NOT NULL DEFAULT 200,
    job_config              JSONB                    NOT NULL DEFAULT '{}',
    created_at              TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Optimized indexes for collection_transforms
CREATE INDEX IF NOT EXISTS idx_collection_transforms_owner_created ON collection_transforms(owner, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_collection_transforms_collection ON collection_transforms(collection_id, is_enabled);
CREATE INDEX IF NOT EXISTS idx_collection_transforms_dataset ON collection_transforms(dataset_id);
CREATE INDEX IF NOT EXISTS idx_collection_transforms_enabled ON collection_transforms(owner, is_enabled) WHERE is_enabled = TRUE;
CREATE INDEX IF NOT EXISTS idx_collection_transforms_enabled_created 
  ON collection_transforms(is_enabled, created_at DESC) 
  WHERE is_enabled = TRUE;

CREATE TABLE IF NOT EXISTS dataset_transforms (
    dataset_transform_id SERIAL PRIMARY KEY,
    title                TEXT                     NOT NULL,
    source_dataset_id    INTEGER                  NOT NULL REFERENCES datasets(dataset_id) ON DELETE CASCADE,
    embedder_ids         INTEGER[]                NOT NULL DEFAULT '{}',
    owner                TEXT                     NOT NULL,
    is_enabled           BOOLEAN                  NOT NULL DEFAULT TRUE,
    job_config           JSONB                    NOT NULL DEFAULT '{}',
    created_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Optimized indexes for dataset_transforms
CREATE INDEX IF NOT EXISTS idx_dataset_transforms_owner_created ON dataset_transforms(owner, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_dataset_transforms_source_enabled ON dataset_transforms(source_dataset_id, is_enabled);
CREATE INDEX IF NOT EXISTS idx_dataset_transforms_enabled ON dataset_transforms(owner, is_enabled) WHERE is_enabled = TRUE;
CREATE INDEX IF NOT EXISTS idx_dataset_transforms_enabled_created 
  ON dataset_transforms(is_enabled, created_at DESC) 
  WHERE is_enabled = TRUE;

CREATE TABLE IF NOT EXISTS embedded_datasets (
    embedded_dataset_id  SERIAL PRIMARY KEY,
    title                TEXT                     NOT NULL,
    dataset_transform_id INTEGER                  NOT NULL REFERENCES dataset_transforms(dataset_transform_id) ON DELETE CASCADE,
    source_dataset_id    INTEGER                  NOT NULL REFERENCES datasets(dataset_id) ON DELETE CASCADE,
    embedder_id          INTEGER                  NOT NULL REFERENCES embedders(embedder_id) ON DELETE CASCADE,
    owner                TEXT                     NOT NULL,
    collection_name      TEXT                     NOT NULL,
    last_processed_at    TIMESTAMP WITH TIME ZONE NULL,
    created_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(dataset_transform_id, embedder_id)
);

-- Optimized indexes for embedded_datasets
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_owner_created ON embedded_datasets(owner, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_transform ON embedded_datasets(dataset_transform_id);
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_collection ON embedded_datasets(collection_name);
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_last_processed ON embedded_datasets(embedded_dataset_id, last_processed_at);

CREATE TABLE IF NOT EXISTS dataset_transform_batches (
    id                      SERIAL PRIMARY KEY,
    dataset_transform_id    INTEGER                  NOT NULL REFERENCES dataset_transforms(dataset_transform_id) ON DELETE CASCADE,
    batch_key               VARCHAR(255)             NOT NULL,
    processed_at            TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status                  VARCHAR(50)              NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'success', 'failed', 'skipped')),
    chunk_count             INTEGER                  NOT NULL DEFAULT 0,
    error_message           TEXT                     NULL,
    processing_duration_ms  BIGINT                   NULL,
    created_at              TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at              TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Optimized composite index for batch queries
CREATE INDEX IF NOT EXISTS idx_dataset_transform_batches_composite ON dataset_transform_batches(dataset_transform_id, status, processed_at DESC);
CREATE INDEX IF NOT EXISTS idx_dataset_transform_batches_status ON dataset_transform_batches(status) WHERE status IN ('pending', 'processing');

CREATE TABLE IF NOT EXISTS visualization_transforms (
    visualization_transform_id SERIAL PRIMARY KEY,
    title                      TEXT                     NOT NULL,
    embedded_dataset_id        INTEGER                  NOT NULL REFERENCES embedded_datasets(embedded_dataset_id) ON DELETE CASCADE,
    owner                      TEXT                     NOT NULL,
    is_enabled                 BOOLEAN                  NOT NULL DEFAULT TRUE,
    reduced_collection_name    TEXT                     NULL,
    topics_collection_name     TEXT                     NULL,
    visualization_config       JSONB                    NOT NULL DEFAULT '{}',
    last_run_status            TEXT                     NULL,
    last_run_at                TIMESTAMP WITH TIME ZONE NULL,
    last_error                 TEXT                     NULL,
    last_run_stats             JSONB                    NULL,
    created_at                 TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at                 TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Optimized indexes for visualization_transforms
CREATE INDEX IF NOT EXISTS idx_visualization_transforms_owner_created ON visualization_transforms(owner, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_visualization_transforms_embedded_enabled ON visualization_transforms(embedded_dataset_id, is_enabled);
CREATE INDEX IF NOT EXISTS idx_visualization_transforms_enabled ON visualization_transforms(owner, is_enabled) WHERE is_enabled = TRUE;
CREATE INDEX IF NOT EXISTS idx_visualization_transforms_status ON visualization_transforms(last_run_status) WHERE last_run_status IS NOT NULL;

CREATE TABLE IF NOT EXISTS visualizations (
    visualization_id           SERIAL PRIMARY KEY,
    visualization_transform_id INTEGER                  NOT NULL REFERENCES visualization_transforms(visualization_transform_id) ON DELETE CASCADE,
    status                     TEXT                     NOT NULL CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    started_at                 TIMESTAMP WITH TIME ZONE NULL,
    completed_at               TIMESTAMP WITH TIME ZONE NULL,
    html_s3_key                TEXT                     NULL,
    point_count                INTEGER                  NULL,
    cluster_count              INTEGER                  NULL,
    error_message              TEXT                     NULL,
    stats_json                 JSONB                    NULL,
    created_at                 TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Optimized indexes for visualizations
CREATE INDEX IF NOT EXISTS idx_visualizations_transform_created ON visualizations(visualization_transform_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_visualizations_status ON visualizations(status) WHERE status IN ('pending', 'processing');

CREATE TABLE IF NOT EXISTS transform_processed_files (
    id                      SERIAL PRIMARY KEY,
    transform_type          TEXT                     NOT NULL,
    transform_id            INTEGER                  NOT NULL,
    file_key                TEXT                     NOT NULL,
    processed_at            TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    item_count              INTEGER                  NOT NULL DEFAULT 0,
    process_status          TEXT                     NOT NULL DEFAULT 'completed',
    process_error           TEXT                     NULL,
    processing_duration_ms  BIGINT                   NULL,
    UNIQUE(transform_type, transform_id, file_key)
);

-- Optimized indexes for transform_processed_files
CREATE INDEX IF NOT EXISTS idx_transform_processed_files_type_id_status ON transform_processed_files(transform_type, transform_id, process_status);
CREATE INDEX IF NOT EXISTS idx_transform_processed_files_processed_at ON transform_processed_files(processed_at DESC);

-- ============================================================================
-- Chat System
-- ============================================================================

CREATE TABLE IF NOT EXISTS chat_sessions (
    session_id          TEXT PRIMARY KEY,
    owner               TEXT                     NOT NULL,
    embedded_dataset_id INTEGER                  NOT NULL REFERENCES embedded_datasets(embedded_dataset_id) ON DELETE CASCADE,
    llm_id              INTEGER                  NOT NULL REFERENCES llms(llm_id) ON DELETE CASCADE,
    title               TEXT                     NOT NULL DEFAULT '',
    created_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Optimized indexes for chat_sessions  
CREATE INDEX IF NOT EXISTS idx_chat_sessions_owner_updated ON chat_sessions(owner, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_chat_sessions_embedded_dataset ON chat_sessions(embedded_dataset_id);
CREATE INDEX IF NOT EXISTS idx_chat_sessions_user_created 
  ON chat_sessions(owner, created_at DESC);

CREATE TABLE IF NOT EXISTS chat_messages (
    message_id          SERIAL PRIMARY KEY,
    session_id          TEXT                     NOT NULL REFERENCES chat_sessions(session_id) ON DELETE CASCADE,
    role                TEXT                     NOT NULL CHECK (role IN ('user', 'assistant')),
    content             TEXT                     NOT NULL,
    documents_retrieved INTEGER                  NULL,
    status              TEXT                     NOT NULL DEFAULT 'complete' CHECK (status IN ('complete', 'incomplete', 'error')),
    created_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Optimized indexes for chat_messages (covering index for common query)
CREATE INDEX IF NOT EXISTS idx_chat_messages_session_created ON chat_messages(session_id, created_at DESC) INCLUDE (role, status);
CREATE INDEX IF NOT EXISTS idx_chat_messages_session_status ON chat_messages(session_id, status) WHERE status != 'complete';
CREATE INDEX IF NOT EXISTS idx_chat_messages_session 
  ON chat_messages(session_id, created_at);

CREATE TABLE IF NOT EXISTS chat_message_retrieved_documents (
    id               SERIAL PRIMARY KEY,
    message_id       INTEGER                  NOT NULL REFERENCES chat_messages(message_id) ON DELETE CASCADE,
    document_id      TEXT                     NULL,
    text             TEXT                     NOT NULL,
    similarity_score REAL                     NOT NULL,
    item_title       TEXT                     NULL,
    source           TEXT                     NULL,
    created_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Optimized index for retrieved documents (covering common columns)
CREATE INDEX IF NOT EXISTS idx_chat_message_retrieved_docs ON chat_message_retrieved_documents(message_id, similarity_score DESC) 
    INCLUDE (document_id, item_title, text);

-- ============================================================================
-- Audit & Compliance
-- ============================================================================

CREATE TABLE IF NOT EXISTS audit_events (
    audit_event_id  BIGSERIAL PRIMARY KEY,
    timestamp       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    event_type      TEXT                     NOT NULL,
    outcome         TEXT                     NOT NULL,
    username        TEXT                     NOT NULL,
    request_id      TEXT                     NULL,
    client_ip       INET                     NULL,
    resource_type   TEXT                     NULL,
    resource_id     TEXT                     NULL,
    details         TEXT                     NULL
);

-- Optimized indexes for audit queries (most common patterns)
CREATE INDEX IF NOT EXISTS idx_audit_events_timestamp ON audit_events(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_events_username_timestamp ON audit_events(username, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_events_type_timestamp ON audit_events(event_type, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_events_resource ON audit_events(resource_type, resource_id) WHERE resource_type IS NOT NULL;

-- ============================================================================
-- Row-Level Security (RLS) Policies
-- ============================================================================
-- Enable RLS on all user-owned tables

ALTER TABLE collections ENABLE ROW LEVEL SECURITY;
ALTER TABLE datasets ENABLE ROW LEVEL SECURITY;
ALTER TABLE dataset_items ENABLE ROW LEVEL SECURITY;
ALTER TABLE embedders ENABLE ROW LEVEL SECURITY;
ALTER TABLE llms ENABLE ROW LEVEL SECURITY;
ALTER TABLE collection_transforms ENABLE ROW LEVEL SECURITY;
ALTER TABLE dataset_transforms ENABLE ROW LEVEL SECURITY;
ALTER TABLE embedded_datasets ENABLE ROW LEVEL SECURITY;
ALTER TABLE dataset_transform_batches ENABLE ROW LEVEL SECURITY;
ALTER TABLE visualization_transforms ENABLE ROW LEVEL SECURITY;
ALTER TABLE visualizations ENABLE ROW LEVEL SECURITY;
ALTER TABLE transform_processed_files ENABLE ROW LEVEL SECURITY;
ALTER TABLE chat_sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE chat_messages ENABLE ROW LEVEL SECURITY;
ALTER TABLE chat_message_retrieved_documents ENABLE ROW LEVEL SECURITY;

-- Collections RLS Policies
CREATE POLICY collections_access_policy ON collections
    FOR ALL
    USING (
        owner = current_setting('app.current_user', TRUE)::text
        OR is_public = TRUE
    );

CREATE POLICY collections_modify_policy ON collections
    FOR INSERT
    WITH CHECK (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY collections_update_policy ON collections
    FOR UPDATE
    USING (owner = current_setting('app.current_user', TRUE)::text)
    WITH CHECK (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY collections_delete_policy ON collections
    FOR DELETE
    USING (owner = current_setting('app.current_user', TRUE)::text);

-- Datasets RLS Policies
CREATE POLICY datasets_access_policy ON datasets
    FOR ALL
    USING (
        owner = current_setting('app.current_user', TRUE)::text
        OR is_public = TRUE
    );

CREATE POLICY datasets_modify_policy ON datasets
    FOR INSERT
    WITH CHECK (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY datasets_update_policy ON datasets
    FOR UPDATE
    USING (owner = current_setting('app.current_user', TRUE)::text)
    WITH CHECK (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY datasets_delete_policy ON datasets
    FOR DELETE
    USING (owner = current_setting('app.current_user', TRUE)::text);

-- Dataset Items RLS Policies (inherits access from parent dataset)
CREATE POLICY dataset_items_access_policy ON dataset_items
    FOR ALL
    USING (
        EXISTS (
            SELECT 1 FROM datasets d
            WHERE d.dataset_id = dataset_items.dataset_id
            AND (
                d.owner = current_setting('app.current_user', TRUE)::text
                OR d.is_public = TRUE
            )
        )
    );

CREATE POLICY dataset_items_modify_policy ON dataset_items
    FOR INSERT
    WITH CHECK (
        EXISTS (
            SELECT 1 FROM datasets d
            WHERE d.dataset_id = dataset_items.dataset_id
            AND d.owner = current_setting('app.current_user', TRUE)::text
        )
    );

CREATE POLICY dataset_items_update_policy ON dataset_items
    FOR UPDATE
    USING (
        EXISTS (
            SELECT 1 FROM datasets d
            WHERE d.dataset_id = dataset_items.dataset_id
            AND d.owner = current_setting('app.current_user', TRUE)::text
        )
    );

CREATE POLICY dataset_items_delete_policy ON dataset_items
    FOR DELETE
    USING (
        EXISTS (
            SELECT 1 FROM datasets d
            WHERE d.dataset_id = dataset_items.dataset_id
            AND d.owner = current_setting('app.current_user', TRUE)::text
        )
    );

-- Embedders RLS Policies
CREATE POLICY embedders_access_policy ON embedders
    FOR ALL
    USING (
        owner = current_setting('app.current_user', TRUE)::text
        OR is_public = TRUE
    );

CREATE POLICY embedders_modify_policy ON embedders
    FOR INSERT
    WITH CHECK (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY embedders_update_policy ON embedders
    FOR UPDATE
    USING (owner = current_setting('app.current_user', TRUE)::text)
    WITH CHECK (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY embedders_delete_policy ON embedders
    FOR DELETE
    USING (owner = current_setting('app.current_user', TRUE)::text);

-- LLMs RLS Policies
CREATE POLICY llms_access_policy ON llms
    FOR ALL
    USING (
        owner = current_setting('app.current_user', TRUE)::text
        OR is_public = TRUE
    );

CREATE POLICY llms_modify_policy ON llms
    FOR INSERT
    WITH CHECK (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY llms_update_policy ON llms
    FOR UPDATE
    USING (owner = current_setting('app.current_user', TRUE)::text)
    WITH CHECK (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY llms_delete_policy ON llms
    FOR DELETE
    USING (owner = current_setting('app.current_user', TRUE)::text);

-- Collection Transforms RLS Policies
CREATE POLICY collection_transforms_access_policy ON collection_transforms
    FOR ALL
    USING (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY collection_transforms_modify_policy ON collection_transforms
    FOR INSERT
    WITH CHECK (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY collection_transforms_update_policy ON collection_transforms
    FOR UPDATE
    USING (owner = current_setting('app.current_user', TRUE)::text)
    WITH CHECK (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY collection_transforms_delete_policy ON collection_transforms
    FOR DELETE
    USING (owner = current_setting('app.current_user', TRUE)::text);

-- Dataset Transforms RLS Policies
CREATE POLICY dataset_transforms_access_policy ON dataset_transforms
    FOR ALL
    USING (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY dataset_transforms_modify_policy ON dataset_transforms
    FOR INSERT
    WITH CHECK (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY dataset_transforms_update_policy ON dataset_transforms
    FOR UPDATE
    USING (owner = current_setting('app.current_user', TRUE)::text)
    WITH CHECK (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY dataset_transforms_delete_policy ON dataset_transforms
    FOR DELETE
    USING (owner = current_setting('app.current_user', TRUE)::text);

-- Embedded Datasets RLS Policies
CREATE POLICY embedded_datasets_access_policy ON embedded_datasets
    FOR ALL
    USING (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY embedded_datasets_modify_policy ON embedded_datasets
    FOR INSERT
    WITH CHECK (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY embedded_datasets_update_policy ON embedded_datasets
    FOR UPDATE
    USING (owner = current_setting('app.current_user', TRUE)::text)
    WITH CHECK (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY embedded_datasets_delete_policy ON embedded_datasets
    FOR DELETE
    USING (owner = current_setting('app.current_user', TRUE)::text);

-- Dataset Transform Batches RLS Policies (inherits from dataset_transforms)
CREATE POLICY dataset_transform_batches_access_policy ON dataset_transform_batches
    FOR ALL
    USING (
        EXISTS (
            SELECT 1 FROM dataset_transforms dt
            WHERE dt.dataset_transform_id = dataset_transform_batches.dataset_transform_id
            AND dt.owner = current_setting('app.current_user', TRUE)::text
        )
    );

-- Visualization Transforms RLS Policies
CREATE POLICY visualization_transforms_access_policy ON visualization_transforms
    FOR ALL
    USING (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY visualization_transforms_modify_policy ON visualization_transforms
    FOR INSERT
    WITH CHECK (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY visualization_transforms_update_policy ON visualization_transforms
    FOR UPDATE
    USING (owner = current_setting('app.current_user', TRUE)::text)
    WITH CHECK (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY visualization_transforms_delete_policy ON visualization_transforms
    FOR DELETE
    USING (owner = current_setting('app.current_user', TRUE)::text);

-- Visualizations RLS Policies (inherits from visualization_transforms)
CREATE POLICY visualizations_access_policy ON visualizations
    FOR ALL
    USING (
        EXISTS (
            SELECT 1 FROM visualization_transforms vt
            WHERE vt.visualization_transform_id = visualizations.visualization_transform_id
            AND vt.owner = current_setting('app.current_user', TRUE)::text
        )
    );

-- Transform Processed Files RLS Policies
CREATE POLICY transform_processed_files_collection_transforms ON transform_processed_files
    FOR ALL
    USING (
        transform_type = 'collection_transform'
        AND EXISTS (
            SELECT 1 FROM collection_transforms ct
            WHERE ct.collection_transform_id = transform_processed_files.transform_id
            AND ct.owner = current_setting('app.current_user', TRUE)::text
        )
    );

CREATE POLICY transform_processed_files_dataset_transforms ON transform_processed_files
    FOR ALL
    USING (
        transform_type = 'dataset_transform'
        AND EXISTS (
            SELECT 1 FROM dataset_transforms dt
            WHERE dt.dataset_transform_id = transform_processed_files.transform_id
            AND dt.owner = current_setting('app.current_user', TRUE)::text
        )
    );

CREATE POLICY transform_processed_files_visualization_transforms ON transform_processed_files
    FOR ALL
    USING (
        transform_type = 'visualization_transform'
        AND EXISTS (
            SELECT 1 FROM visualization_transforms vt
            WHERE vt.visualization_transform_id = transform_processed_files.transform_id
            AND vt.owner = current_setting('app.current_user', TRUE)::text
        )
    );

-- Chat Sessions RLS Policies
CREATE POLICY chat_sessions_access_policy ON chat_sessions
    FOR ALL
    USING (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY chat_sessions_modify_policy ON chat_sessions
    FOR INSERT
    WITH CHECK (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY chat_sessions_update_policy ON chat_sessions
    FOR UPDATE
    USING (owner = current_setting('app.current_user', TRUE)::text)
    WITH CHECK (owner = current_setting('app.current_user', TRUE)::text);

CREATE POLICY chat_sessions_delete_policy ON chat_sessions
    FOR DELETE
    USING (owner = current_setting('app.current_user', TRUE)::text);

-- Chat Messages RLS Policies (inherits from chat_sessions)
CREATE POLICY chat_messages_access_policy ON chat_messages
    FOR ALL
    USING (
        EXISTS (
            SELECT 1 FROM chat_sessions cs
            WHERE cs.session_id = chat_messages.session_id
            AND cs.owner = current_setting('app.current_user', TRUE)::text
        )
    );

-- Chat Message Retrieved Documents RLS Policies (inherits from chat_messages)
CREATE POLICY chat_message_retrieved_documents_access_policy ON chat_message_retrieved_documents
    FOR ALL
    USING (
        EXISTS (
            SELECT 1 FROM chat_messages cm
            JOIN chat_sessions cs ON cs.session_id = cm.session_id
            WHERE cm.message_id = chat_message_retrieved_documents.message_id
            AND cs.owner = current_setting('app.current_user', TRUE)::text
        )
    );

-- ============================================================================
-- OIDC Session Management Tables
-- ============================================================================

CREATE TABLE IF NOT EXISTS user_sessions (
    session_id          TEXT PRIMARY KEY,
    username            TEXT                     NOT NULL,
    user_agent          TEXT                     NULL,
    ip_address          INET                     NULL,
    id_token_hash       TEXT                     NOT NULL,
    access_token_hash   TEXT                     NOT NULL,
    refresh_token_hash  TEXT                     NULL,
    created_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    last_activity_at    TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    expires_at          TIMESTAMP WITH TIME ZONE NOT NULL,
    revoked_at          TIMESTAMP WITH TIME ZONE NULL,
    revoked_reason      TEXT                     NULL
);

-- Indexes for session management queries
CREATE INDEX IF NOT EXISTS idx_user_sessions_username_active ON user_sessions(username, last_activity_at DESC) 
    WHERE revoked_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_user_sessions_expires ON user_sessions(expires_at) 
    WHERE revoked_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_user_sessions_token_hash ON user_sessions(access_token_hash) 
    WHERE revoked_at IS NULL;

CREATE TABLE IF NOT EXISTS refresh_token_rotations (
    id                  BIGSERIAL PRIMARY KEY,
    session_id          TEXT                     NOT NULL REFERENCES user_sessions(session_id) ON DELETE CASCADE,
    old_token_hash      TEXT                     NOT NULL,
    new_token_hash      TEXT                     NOT NULL,
    rotated_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    ip_address          INET                     NULL,
    user_agent          TEXT                     NULL
);

-- Index for detecting token reuse attempts
CREATE INDEX IF NOT EXISTS idx_refresh_token_rotations_old_hash ON refresh_token_rotations(old_token_hash, rotated_at DESC);
CREATE INDEX IF NOT EXISTS idx_refresh_token_rotations_session ON refresh_token_rotations(session_id, rotated_at DESC);

CREATE TABLE IF NOT EXISTS session_events (
    id                  BIGSERIAL PRIMARY KEY,
    session_id          TEXT                     NOT NULL,
    username            TEXT                     NOT NULL,
    event_type          TEXT                     NOT NULL CHECK (event_type IN (
        'session_created', 
        'session_refreshed', 
        'session_revoked', 
        'session_expired',
        'token_rotation',
        'concurrent_limit_exceeded',
        'suspicious_activity'
    )),
    ip_address          INET                     NULL,
    user_agent          TEXT                     NULL,
    details             JSONB                    NULL,
    created_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Indexes for session event queries
CREATE INDEX IF NOT EXISTS idx_session_events_session ON session_events(session_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_session_events_username ON session_events(username, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_session_events_type ON session_events(event_type, created_at DESC);

-- ============================================================================
-- Helper Functions for Session Management
-- ============================================================================

CREATE OR REPLACE FUNCTION revoke_expired_sessions()
RETURNS INTEGER AS $$
DECLARE
    revoked_count INTEGER;
BEGIN
    WITH revoked AS (
        UPDATE user_sessions
        SET revoked_at = NOW(),
            revoked_reason = 'expired'
        WHERE expires_at < NOW()
          AND revoked_at IS NULL
        RETURNING session_id
    )
    SELECT COUNT(*) INTO revoked_count FROM revoked;
    
    RETURN revoked_count;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION enforce_session_limit(
    p_username TEXT,
    p_max_sessions INTEGER
) RETURNS INTEGER AS $$
DECLARE
    revoked_count INTEGER;
BEGIN
    WITH oldest_sessions AS (
        SELECT session_id
        FROM user_sessions
        WHERE username = p_username
          AND revoked_at IS NULL
        ORDER BY last_activity_at DESC
        OFFSET p_max_sessions
    ),
    revoked AS (
        UPDATE user_sessions
        SET revoked_at = NOW(),
            revoked_reason = 'concurrent_limit_exceeded'
        WHERE session_id IN (SELECT session_id FROM oldest_sessions)
        RETURNING session_id
    )
    SELECT COUNT(*) INTO revoked_count FROM revoked;
    
    RETURN revoked_count;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION get_active_session_count(p_username TEXT)
RETURNS INTEGER AS $$
DECLARE
    session_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO session_count
    FROM user_sessions
    WHERE username = p_username
      AND revoked_at IS NULL
      AND expires_at > NOW();
    
    RETURN session_count;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- Table Comments for Documentation
-- ============================================================================

COMMENT ON TABLE dataset_transform_batches IS
    'Tracks individual batch processing operations for dataset transforms. Status: pending, processing, success, failed, skipped.';
COMMENT ON TABLE chat_messages IS
    'Chat messages with streaming support. Status: complete (default), incomplete (streaming), error (failed).';
COMMENT ON TABLE audit_events IS
    'Security and compliance audit log for all system events.';
COMMENT ON COLUMN embedders.api_key_encrypted IS
    'AES-256-GCM encrypted API key. Format: base64(nonce || ciphertext) where nonce is 12 bytes. Required for production.';
COMMENT ON COLUMN llms.api_key_encrypted IS
    'AES-256-GCM encrypted API key. Format: base64(nonce || ciphertext) where nonce is 12 bytes. Required for production.';

-- ============================================================================
-- Migration Complete
-- ============================================================================
-- All schema, indexes, RLS policies, and helper functions are now in place.
-- The database is ready for application use.
-- ============================================================================
