-- ============================================================================
-- Collections & Datasets
-- ============================================================================

CREATE TABLE IF NOT EXISTS collections (
    collection_id       SERIAL PRIMARY KEY,
    title               TEXT                     NOT NULL,
    details             TEXT                     NULL,
    owner_id            TEXT                     NOT NULL,
    owner_display_name  TEXT                     NOT NULL,
    tags                TEXT[]                   NOT NULL DEFAULT '{}',
    is_public           BOOLEAN                  NOT NULL DEFAULT FALSE,
    file_count          BIGINT                   NOT NULL DEFAULT 0,
    created_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Optimized indexes for collections
CREATE INDEX IF NOT EXISTS idx_collections_owner_created ON collections(owner_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_collections_is_public ON collections(is_public) WHERE is_public = TRUE;
CREATE INDEX IF NOT EXISTS idx_collections_title_tsvector 
  ON collections USING GIN (to_tsvector('english', title));
CREATE INDEX IF NOT EXISTS idx_collections_tags 
  ON collections USING GIN (tags);
CREATE INDEX IF NOT EXISTS idx_collections_title ON collections(title);
CREATE INDEX IF NOT EXISTS idx_collections_owner_title ON collections(owner_id, title);

CREATE TABLE IF NOT EXISTS datasets (
    dataset_id          SERIAL PRIMARY KEY,
    title               TEXT                     NOT NULL,
    details             TEXT                     NULL,
    owner_id            TEXT                     NOT NULL,
    owner_display_name  TEXT                     NOT NULL,
    tags                TEXT[]                   NOT NULL DEFAULT '{}',
    is_public           BOOLEAN                  NOT NULL DEFAULT FALSE,
    item_count          INTEGER                  NOT NULL DEFAULT 0,
    total_chunks        BIGINT                   NOT NULL DEFAULT 0,
    created_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_datasets_owner_created ON datasets(owner_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_datasets_is_public ON datasets(is_public) WHERE is_public = TRUE;
CREATE INDEX IF NOT EXISTS idx_datasets_title_tsvector 
  ON datasets USING GIN (to_tsvector('english', title));
CREATE INDEX IF NOT EXISTS idx_datasets_tags 
  ON datasets USING GIN (tags);
CREATE INDEX IF NOT EXISTS idx_datasets_title ON datasets(title);
CREATE INDEX IF NOT EXISTS idx_datasets_owner_title ON datasets(owner_id, title);

CREATE TABLE IF NOT EXISTS dataset_items (
    item_id          SERIAL PRIMARY KEY,
    dataset_id       INTEGER                  NOT NULL REFERENCES datasets(dataset_id) ON DELETE CASCADE,
    title            TEXT                     NOT NULL,
    chunks           JSONB                    NOT NULL,
    metadata         JSONB                    NOT NULL DEFAULT '{}',
    created_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_dataset_items_dataset_created ON dataset_items(dataset_id, created_at DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_dataset_items_dataset_title_unique ON dataset_items(dataset_id, title);
CREATE INDEX IF NOT EXISTS idx_dataset_items_name 
  ON dataset_items(title) 
  WHERE title IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_dataset_items_dataset_item_desc 
  ON dataset_items(dataset_id, item_id DESC);

-- ============================================================================
-- Dataset Stats Triggers
-- ============================================================================

-- Trigger function to update dataset stats on INSERT/DELETE/UPDATE of dataset_items
CREATE OR REPLACE FUNCTION update_dataset_stats() RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        -- Update stats for the new/updated dataset
        UPDATE datasets
        SET
            item_count = (SELECT COUNT(*) FROM dataset_items WHERE dataset_id = NEW.dataset_id),
            total_chunks = (SELECT COALESCE(SUM(jsonb_array_length(chunks)), 0) FROM dataset_items WHERE dataset_id = NEW.dataset_id),
            updated_at = NOW()
        WHERE dataset_id = NEW.dataset_id;
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        -- Update stats after deletion
        UPDATE datasets
        SET
            item_count = (SELECT COUNT(*) FROM dataset_items WHERE dataset_id = OLD.dataset_id),
            total_chunks = (SELECT COALESCE(SUM(jsonb_array_length(chunks)), 0) FROM dataset_items WHERE dataset_id = OLD.dataset_id),
            updated_at = NOW()
        WHERE dataset_id = OLD.dataset_id;
        RETURN OLD;
    ELSIF TG_OP = 'UPDATE' THEN
        -- Update stats if chunks changed
        IF OLD.chunks IS DISTINCT FROM NEW.chunks THEN
            UPDATE datasets
            SET
                total_chunks = (SELECT COALESCE(SUM(jsonb_array_length(chunks)), 0) FROM dataset_items WHERE dataset_id = NEW.dataset_id),
                updated_at = NOW()
            WHERE dataset_id = NEW.dataset_id;
        END IF;
        RETURN NEW;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Create trigger on dataset_items to maintain stats
DROP TRIGGER IF EXISTS trg_update_dataset_stats ON dataset_items;
CREATE TRIGGER trg_update_dataset_stats
AFTER INSERT OR DELETE OR UPDATE OF chunks ON dataset_items
FOR EACH ROW
EXECUTE FUNCTION update_dataset_stats();

-- ============================================================================
-- Embedders & LLMs
-- ============================================================================

CREATE TABLE IF NOT EXISTS embedders (
    embedder_id          SERIAL PRIMARY KEY,
    name                 TEXT                     NOT NULL,
    owner_id             TEXT                     NOT NULL,
    owner_display_name   TEXT                     NOT NULL,
    provider             TEXT                     NOT NULL,
    base_url             TEXT                     NOT NULL,
    api_key_encrypted    TEXT                     NULL,
    config               JSONB                    NOT NULL DEFAULT '{}',
    batch_size           INTEGER                  NOT NULL DEFAULT 96,
    dimensions           INTEGER                  NOT NULL DEFAULT 1536,
    max_input_tokens     INTEGER                  NOT NULL DEFAULT 8191,
    truncate_strategy    VARCHAR(50)              NOT NULL DEFAULT 'NONE',
    collection_name      TEXT                     NULL,
    is_public            BOOLEAN                  NOT NULL DEFAULT FALSE,
    created_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_embedders_owner_public ON embedders(owner_id, is_public);
CREATE INDEX IF NOT EXISTS idx_embedders_provider ON embedders(provider);
CREATE INDEX IF NOT EXISTS idx_embedders_is_public ON embedders(is_public) WHERE is_public = TRUE;
CREATE INDEX IF NOT EXISTS idx_embedders_owner_created 
  ON embedders(owner_id, created_at DESC);

CREATE TABLE IF NOT EXISTS llms (
    llm_id               SERIAL PRIMARY KEY,
    name                 TEXT                     NOT NULL,
    owner_id             TEXT                     NOT NULL,
    owner_display_name   TEXT                     NOT NULL,
    provider             TEXT                     NOT NULL,
    base_url             TEXT                     NOT NULL,
    api_key_encrypted    TEXT                     NULL,
    model                TEXT                     NOT NULL,
    config               JSONB                    NOT NULL DEFAULT '{}',
    is_public            BOOLEAN                  NOT NULL DEFAULT FALSE,
    created_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_llms_owner_public ON llms(owner_id, is_public);
CREATE INDEX IF NOT EXISTS idx_llms_provider ON llms(provider);
CREATE INDEX IF NOT EXISTS idx_llms_is_public ON llms(is_public) WHERE is_public = TRUE;
CREATE INDEX IF NOT EXISTS idx_llms_owner_created 
  ON llms(owner_id, created_at DESC);

-- ============================================================================
-- Transform Pipeline
-- ============================================================================

CREATE TABLE IF NOT EXISTS collection_transforms (
    collection_transform_id SERIAL PRIMARY KEY,
    title                   TEXT                     NOT NULL,
    collection_id           INTEGER                  NOT NULL REFERENCES collections(collection_id) ON DELETE CASCADE,
    dataset_id              INTEGER                  NOT NULL REFERENCES datasets(dataset_id) ON DELETE CASCADE,
    owner_id                TEXT                     NOT NULL,
    owner_display_name      TEXT                     NOT NULL,
    is_enabled              BOOLEAN                  NOT NULL DEFAULT TRUE,
    chunk_size              INTEGER                  NOT NULL DEFAULT 200,
    job_config              JSONB                    NOT NULL DEFAULT '{}',
    created_at              TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_collection_transforms_owner_created ON collection_transforms(owner_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_collection_transforms_collection ON collection_transforms(collection_id, is_enabled);
CREATE INDEX IF NOT EXISTS idx_collection_transforms_dataset ON collection_transforms(dataset_id);
CREATE INDEX IF NOT EXISTS idx_collection_transforms_enabled ON collection_transforms(owner_id, is_enabled) WHERE is_enabled = TRUE;
CREATE INDEX IF NOT EXISTS idx_collection_transforms_enabled_created 
  ON collection_transforms(is_enabled, created_at DESC) 
  WHERE is_enabled = TRUE;

CREATE TABLE IF NOT EXISTS dataset_transforms (
    dataset_transform_id SERIAL PRIMARY KEY,
    title                TEXT                     NOT NULL,
    source_dataset_id    INTEGER                  NOT NULL REFERENCES datasets(dataset_id) ON DELETE CASCADE,
    embedder_ids         INTEGER[]                NOT NULL DEFAULT '{}',
    owner_id             TEXT                     NOT NULL,
    owner_display_name   TEXT                     NOT NULL,
    is_enabled           BOOLEAN                  NOT NULL DEFAULT TRUE,
    job_config           JSONB                    NOT NULL DEFAULT '{}',
    created_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_dataset_transforms_owner_created ON dataset_transforms(owner_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_dataset_transforms_source_enabled ON dataset_transforms(source_dataset_id, is_enabled);
CREATE INDEX IF NOT EXISTS idx_dataset_transforms_enabled ON dataset_transforms(owner_id, is_enabled) WHERE is_enabled = TRUE;
CREATE INDEX IF NOT EXISTS idx_dataset_transforms_enabled_created 
  ON dataset_transforms(is_enabled, created_at DESC) 
  WHERE is_enabled = TRUE;

CREATE TABLE IF NOT EXISTS embedded_datasets (
    embedded_dataset_id  SERIAL PRIMARY KEY,
    title                TEXT                     NOT NULL,
    dataset_transform_id INTEGER                  NOT NULL REFERENCES dataset_transforms(dataset_transform_id) ON DELETE CASCADE,
    source_dataset_id    INTEGER                  NOT NULL REFERENCES datasets(dataset_id) ON DELETE CASCADE,
    embedder_id          INTEGER                  NOT NULL REFERENCES embedders(embedder_id) ON DELETE CASCADE,
    owner_id             TEXT                     NOT NULL,
    owner_display_name   TEXT                     NOT NULL,
    collection_name      TEXT                     NOT NULL,
    last_processed_at    TIMESTAMP WITH TIME ZONE NULL,
    created_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(dataset_transform_id, embedder_id)
);


CREATE INDEX IF NOT EXISTS idx_embedded_datasets_owner_created ON embedded_datasets(owner_id, created_at DESC);
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
    owner_id                   TEXT                     NOT NULL,
    owner_display_name         TEXT                     NOT NULL,
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
CREATE INDEX IF NOT EXISTS idx_visualization_transforms_owner_created ON visualization_transforms(owner_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_visualization_transforms_embedded_enabled ON visualization_transforms(embedded_dataset_id, is_enabled);
CREATE INDEX IF NOT EXISTS idx_visualization_transforms_enabled ON visualization_transforms(owner_id, is_enabled) WHERE is_enabled = TRUE;
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
    owner_id            TEXT                     NOT NULL,
    owner_display_name  TEXT                     NOT NULL,
    embedded_dataset_id INTEGER                  NOT NULL REFERENCES embedded_datasets(embedded_dataset_id) ON DELETE CASCADE,
    llm_id              INTEGER                  NOT NULL REFERENCES llms(llm_id) ON DELETE CASCADE,
    title               TEXT                     NOT NULL DEFAULT '',
    created_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_chat_sessions_owner_updated ON chat_sessions(owner_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_chat_sessions_embedded_dataset ON chat_sessions(embedded_dataset_id);
CREATE INDEX IF NOT EXISTS idx_chat_sessions_user_created 
  ON chat_sessions(owner_id, created_at DESC);

CREATE TABLE IF NOT EXISTS chat_messages (
    message_id          SERIAL PRIMARY KEY,
    session_id          TEXT                     NOT NULL REFERENCES chat_sessions(session_id) ON DELETE CASCADE,
    role                TEXT                     NOT NULL CHECK (role IN ('user', 'assistant')),
    content             TEXT                     NOT NULL,
    documents_retrieved INTEGER                  NULL,
    status              TEXT                     NOT NULL DEFAULT 'complete' CHECK (status IN ('complete', 'incomplete', 'error')),
    created_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

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

CREATE INDEX IF NOT EXISTS idx_chat_message_retrieved_docs ON chat_message_retrieved_documents(message_id, similarity_score DESC) 
    INCLUDE (document_id, item_title, text);

-- ============================================================================
-- Audit & Compliance
-- ============================================================================

CREATE TABLE IF NOT EXISTS audit_events (
    audit_event_id   BIGSERIAL PRIMARY KEY,
    timestamp        TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    event_type       TEXT                     NOT NULL,
    outcome          TEXT                     NOT NULL,
    user_id          TEXT                     NOT NULL,
    username_display TEXT                     NOT NULL,
    request_id       TEXT                     NULL,
    client_ip        INET                     NULL,
    resource_type    TEXT                     NULL,
    resource_id      TEXT                     NULL,
    details          TEXT                     NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_events_timestamp ON audit_events(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_events_user_timestamp ON audit_events(user_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_events_username_display_timestamp ON audit_events(username_display, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_events_type_timestamp ON audit_events(event_type, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_events_resource ON audit_events(resource_type, resource_id) WHERE resource_type IS NOT NULL;

-- ============================================================================
-- Row-Level Security (RLS) Policies
-- ============================================================================
-- Enable RLS on all user-owned tables
-- FORCE ROW LEVEL SECURITY ensures RLS applies even for table owners/superusers

ALTER TABLE collections ENABLE ROW LEVEL SECURITY;
ALTER TABLE collections FORCE ROW LEVEL SECURITY;

ALTER TABLE datasets ENABLE ROW LEVEL SECURITY;
ALTER TABLE datasets FORCE ROW LEVEL SECURITY;

ALTER TABLE dataset_items ENABLE ROW LEVEL SECURITY;
ALTER TABLE dataset_items FORCE ROW LEVEL SECURITY;

ALTER TABLE embedders ENABLE ROW LEVEL SECURITY;
ALTER TABLE embedders FORCE ROW LEVEL SECURITY;

ALTER TABLE llms ENABLE ROW LEVEL SECURITY;
ALTER TABLE llms FORCE ROW LEVEL SECURITY;

ALTER TABLE collection_transforms ENABLE ROW LEVEL SECURITY;
ALTER TABLE collection_transforms FORCE ROW LEVEL SECURITY;

ALTER TABLE dataset_transforms ENABLE ROW LEVEL SECURITY;
ALTER TABLE dataset_transforms FORCE ROW LEVEL SECURITY;

ALTER TABLE embedded_datasets ENABLE ROW LEVEL SECURITY;
ALTER TABLE embedded_datasets FORCE ROW LEVEL SECURITY;

ALTER TABLE dataset_transform_batches ENABLE ROW LEVEL SECURITY;
ALTER TABLE dataset_transform_batches FORCE ROW LEVEL SECURITY;

ALTER TABLE visualization_transforms ENABLE ROW LEVEL SECURITY;
ALTER TABLE visualization_transforms FORCE ROW LEVEL SECURITY;

ALTER TABLE visualizations ENABLE ROW LEVEL SECURITY;
ALTER TABLE visualizations FORCE ROW LEVEL SECURITY;

ALTER TABLE transform_processed_files ENABLE ROW LEVEL SECURITY;
ALTER TABLE transform_processed_files FORCE ROW LEVEL SECURITY;

ALTER TABLE chat_sessions ENABLE ROW LEVEL SECURITY;
ALTER TABLE chat_sessions FORCE ROW LEVEL SECURITY;

ALTER TABLE chat_messages ENABLE ROW LEVEL SECURITY;
ALTER TABLE chat_messages FORCE ROW LEVEL SECURITY;

ALTER TABLE chat_message_retrieved_documents ENABLE ROW LEVEL SECURITY;
ALTER TABLE chat_message_retrieved_documents FORCE ROW LEVEL SECURITY;

-- Collections RLS Policies
CREATE POLICY collections_access_policy ON collections
    FOR ALL
    USING (
        owner_id = current_setting('app.current_user', TRUE)::text
        OR is_public = TRUE
    );

CREATE POLICY collections_modify_policy ON collections
    FOR INSERT
    WITH CHECK (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY collections_update_policy ON collections
    FOR UPDATE
    USING (owner_id = current_setting('app.current_user', TRUE)::text)
    WITH CHECK (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY collections_delete_policy ON collections
    FOR DELETE
    USING (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY datasets_access_policy ON datasets
    FOR ALL
    USING (
        owner_id = current_setting('app.current_user', TRUE)::text
        OR is_public = TRUE
    );

CREATE POLICY datasets_modify_policy ON datasets
    FOR INSERT
    WITH CHECK (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY datasets_update_policy ON datasets
    FOR UPDATE
    USING (owner_id = current_setting('app.current_user', TRUE)::text)
    WITH CHECK (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY datasets_delete_policy ON datasets
    FOR DELETE
    USING (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY dataset_items_access_policy ON dataset_items
    FOR ALL
    USING (
        EXISTS (
            SELECT 1 FROM datasets d
            WHERE d.dataset_id = dataset_items.dataset_id
            AND (
                d.owner_id = current_setting('app.current_user', TRUE)::text
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
            AND d.owner_id = current_setting('app.current_user', TRUE)::text
        )
    );

CREATE POLICY dataset_items_update_policy ON dataset_items
    FOR UPDATE
    USING (
        EXISTS (
            SELECT 1 FROM datasets d
            WHERE d.dataset_id = dataset_items.dataset_id
            AND d.owner_id = current_setting('app.current_user', TRUE)::text
        )
    );

CREATE POLICY dataset_items_delete_policy ON dataset_items
    FOR DELETE
    USING (
        EXISTS (
            SELECT 1 FROM datasets d
            WHERE d.dataset_id = dataset_items.dataset_id
            AND d.owner_id = current_setting('app.current_user', TRUE)::text
        )
    );

CREATE POLICY embedders_access_policy ON embedders
    FOR ALL
    USING (
        owner_id = current_setting('app.current_user', TRUE)::text
        OR is_public = TRUE
    );

CREATE POLICY embedders_modify_policy ON embedders
    FOR INSERT
    WITH CHECK (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY embedders_update_policy ON embedders
    FOR UPDATE
    USING (owner_id = current_setting('app.current_user', TRUE)::text)
    WITH CHECK (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY embedders_delete_policy ON embedders
    FOR DELETE
    USING (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY llms_access_policy ON llms
    FOR ALL
    USING (
        owner_id = current_setting('app.current_user', TRUE)::text
        OR is_public = TRUE
    );

CREATE POLICY llms_modify_policy ON llms
    FOR INSERT
    WITH CHECK (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY llms_update_policy ON llms
    FOR UPDATE
    USING (owner_id = current_setting('app.current_user', TRUE)::text)
    WITH CHECK (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY llms_delete_policy ON llms
    FOR DELETE
    USING (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY collection_transforms_access_policy ON collection_transforms
    FOR ALL
    USING (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY collection_transforms_modify_policy ON collection_transforms
    FOR INSERT
    WITH CHECK (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY collection_transforms_update_policy ON collection_transforms
    FOR UPDATE
    USING (owner_id = current_setting('app.current_user', TRUE)::text)
    WITH CHECK (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY collection_transforms_delete_policy ON collection_transforms
    FOR DELETE
    USING (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY dataset_transforms_access_policy ON dataset_transforms
    FOR ALL
    USING (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY dataset_transforms_modify_policy ON dataset_transforms
    FOR INSERT
    WITH CHECK (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY dataset_transforms_update_policy ON dataset_transforms
    FOR UPDATE
    USING (owner_id = current_setting('app.current_user', TRUE)::text)
    WITH CHECK (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY dataset_transforms_delete_policy ON dataset_transforms
    FOR DELETE
    USING (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY embedded_datasets_access_policy ON embedded_datasets
    FOR ALL
    USING (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY embedded_datasets_modify_policy ON embedded_datasets
    FOR INSERT
    WITH CHECK (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY embedded_datasets_update_policy ON embedded_datasets
    FOR UPDATE
    USING (owner_id = current_setting('app.current_user', TRUE)::text)
    WITH CHECK (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY embedded_datasets_delete_policy ON embedded_datasets
    FOR DELETE
    USING (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY dataset_transform_batches_access_policy ON dataset_transform_batches
    FOR ALL
    USING (
        EXISTS (
            SELECT 1 FROM dataset_transforms dt
            WHERE dt.dataset_transform_id = dataset_transform_batches.dataset_transform_id
            AND dt.owner_id = current_setting('app.current_user', TRUE)::text
        )
    );

CREATE POLICY visualization_transforms_access_policy ON visualization_transforms
    FOR ALL
    USING (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY visualization_transforms_modify_policy ON visualization_transforms
    FOR INSERT
    WITH CHECK (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY visualization_transforms_update_policy ON visualization_transforms
    FOR UPDATE
    USING (owner_id = current_setting('app.current_user', TRUE)::text)
    WITH CHECK (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY visualization_transforms_delete_policy ON visualization_transforms
    FOR DELETE
    USING (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY visualizations_access_policy ON visualizations
    FOR ALL
    USING (
        EXISTS (
            SELECT 1 FROM visualization_transforms vt
            WHERE vt.visualization_transform_id = visualizations.visualization_transform_id
            AND vt.owner_id = current_setting('app.current_user', TRUE)::text
        )
    );

CREATE POLICY transform_processed_files_collection_transforms ON transform_processed_files
    FOR ALL
    USING (
        transform_type = 'collection_transform'
        AND EXISTS (
            SELECT 1 FROM collection_transforms ct
            WHERE ct.collection_transform_id = transform_processed_files.transform_id
            AND ct.owner_id = current_setting('app.current_user', TRUE)::text
        )
    );

CREATE POLICY transform_processed_files_dataset_transforms ON transform_processed_files
    FOR ALL
    USING (
        transform_type = 'dataset_transform'
        AND EXISTS (
            SELECT 1 FROM dataset_transforms dt
            WHERE dt.dataset_transform_id = transform_processed_files.transform_id
            AND dt.owner_id = current_setting('app.current_user', TRUE)::text
        )
    );

CREATE POLICY transform_processed_files_visualization_transforms ON transform_processed_files
    FOR ALL
    USING (
        transform_type = 'visualization_transform'
        AND EXISTS (
            SELECT 1 FROM visualization_transforms vt
            WHERE vt.visualization_transform_id = transform_processed_files.transform_id
            AND vt.owner_id = current_setting('app.current_user', TRUE)::text
        )
    );

CREATE POLICY chat_sessions_access_policy ON chat_sessions
    FOR ALL
    USING (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY chat_sessions_modify_policy ON chat_sessions
    FOR INSERT
    WITH CHECK (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY chat_sessions_update_policy ON chat_sessions
    FOR UPDATE
    USING (owner_id = current_setting('app.current_user', TRUE)::text)
    WITH CHECK (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY chat_sessions_delete_policy ON chat_sessions
    FOR DELETE
    USING (owner_id = current_setting('app.current_user', TRUE)::text);

CREATE POLICY chat_messages_access_policy ON chat_messages
    FOR ALL
    USING (
        EXISTS (
            SELECT 1 FROM chat_sessions cs
            WHERE cs.session_id = chat_messages.session_id
            AND cs.owner_id = current_setting('app.current_user', TRUE)::text
        )
    );

CREATE POLICY chat_message_retrieved_documents_access_policy ON chat_message_retrieved_documents
    FOR ALL
    USING (
        EXISTS (
            SELECT 1 FROM chat_messages cm
            JOIN chat_sessions cs ON cs.session_id = cm.session_id
            WHERE cm.message_id = chat_message_retrieved_documents.message_id
            AND cs.owner_id = current_setting('app.current_user', TRUE)::text
        )
    );