-- ============================================================================
-- Collections & Datasets
-- ============================================================================

CREATE TABLE IF NOT EXISTS COLLECTIONS (
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
CREATE INDEX idx_collections_owner_created ON collections(owner, created_at DESC);
CREATE INDEX idx_collections_is_public ON collections(is_public) WHERE is_public = TRUE;

CREATE TABLE IF NOT EXISTS DATASETS (
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
CREATE INDEX idx_datasets_owner_created ON datasets(owner, created_at DESC);
CREATE INDEX idx_datasets_is_public ON datasets(is_public) WHERE is_public = TRUE;

CREATE TABLE IF NOT EXISTS DATASET_ITEMS (
    item_id          SERIAL PRIMARY KEY,
    dataset_id       INTEGER                  NOT NULL REFERENCES DATASETS(dataset_id) ON DELETE CASCADE,
    title            TEXT                     NOT NULL,
    chunks           JSONB                    NOT NULL,
    metadata         JSONB                    NOT NULL DEFAULT '{}',
    created_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Optimized indexes for dataset_items
CREATE INDEX idx_dataset_items_dataset_created ON dataset_items(dataset_id, created_at DESC);
CREATE UNIQUE INDEX idx_dataset_items_dataset_title_unique ON dataset_items(dataset_id, title);

-- ============================================================================
-- Embedders & LLMs
-- ============================================================================

CREATE TABLE IF NOT EXISTS EMBEDDERS (
    embedder_id          SERIAL PRIMARY KEY,
    name                 TEXT                     NOT NULL,
    owner                TEXT                     NOT NULL,
    provider             TEXT                     NOT NULL,
    base_url             TEXT                     NOT NULL,
    api_key              TEXT                     NULL,
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
CREATE INDEX idx_embedders_owner_public ON embedders(owner, is_public);
CREATE INDEX idx_embedders_provider ON embedders(provider);
CREATE INDEX idx_embedders_is_public ON embedders(is_public) WHERE is_public = TRUE;

CREATE TABLE IF NOT EXISTS LLMS (
    llm_id               SERIAL PRIMARY KEY,
    name                 TEXT                     NOT NULL,
    owner                TEXT                     NOT NULL,
    provider             TEXT                     NOT NULL,
    base_url             TEXT                     NOT NULL,
    api_key              TEXT                     NULL,
    model                TEXT                     NOT NULL DEFAULT 'gpt-4',
    config               JSONB                    NOT NULL DEFAULT '{}',
    is_public            BOOLEAN                  NOT NULL DEFAULT FALSE,
    created_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Optimized indexes for LLMs
CREATE INDEX idx_llms_owner_public ON llms(owner, is_public);
CREATE INDEX idx_llms_provider ON llms(provider);
CREATE INDEX idx_llms_is_public ON llms(is_public) WHERE is_public = TRUE;

-- ============================================================================
-- Transform Pipeline
-- ============================================================================

CREATE TABLE IF NOT EXISTS COLLECTION_TRANSFORMS (
    collection_transform_id SERIAL PRIMARY KEY,
    title                   TEXT                     NOT NULL,
    collection_id           INTEGER                  NOT NULL REFERENCES COLLECTIONS(collection_id) ON DELETE CASCADE,
    dataset_id              INTEGER                  NOT NULL REFERENCES DATASETS(dataset_id) ON DELETE CASCADE,
    owner                   TEXT                     NOT NULL,
    is_enabled              BOOLEAN                  NOT NULL DEFAULT TRUE,
    chunk_size              INTEGER                  NOT NULL DEFAULT 200,
    job_config              JSONB                    NOT NULL DEFAULT '{}',
    created_at              TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Optimized indexes for collection_transforms
CREATE INDEX idx_collection_transforms_owner_created ON collection_transforms(owner, created_at DESC);
CREATE INDEX idx_collection_transforms_collection ON collection_transforms(collection_id, is_enabled);
CREATE INDEX idx_collection_transforms_dataset ON collection_transforms(dataset_id);
CREATE INDEX idx_collection_transforms_enabled ON collection_transforms(owner, is_enabled) WHERE is_enabled = TRUE;

CREATE TABLE IF NOT EXISTS DATASET_TRANSFORMS (
    dataset_transform_id SERIAL PRIMARY KEY,
    title                TEXT                     NOT NULL,
    source_dataset_id    INTEGER                  NOT NULL REFERENCES DATASETS(dataset_id) ON DELETE CASCADE,
    embedder_ids         INTEGER[]                NOT NULL DEFAULT '{}',
    owner                TEXT                     NOT NULL,
    is_enabled           BOOLEAN                  NOT NULL DEFAULT TRUE,
    job_config           JSONB                    NOT NULL DEFAULT '{}',
    created_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Optimized indexes for dataset_transforms
CREATE INDEX idx_dataset_transforms_owner_created ON dataset_transforms(owner, created_at DESC);
CREATE INDEX idx_dataset_transforms_source_enabled ON dataset_transforms(source_dataset_id, is_enabled);
CREATE INDEX idx_dataset_transforms_enabled ON dataset_transforms(owner, is_enabled) WHERE is_enabled = TRUE;

CREATE TABLE IF NOT EXISTS EMBEDDED_DATASETS (
    embedded_dataset_id  SERIAL PRIMARY KEY,
    title                TEXT                     NOT NULL,
    dataset_transform_id INTEGER                  NOT NULL REFERENCES DATASET_TRANSFORMS(dataset_transform_id) ON DELETE CASCADE,
    source_dataset_id    INTEGER                  NOT NULL REFERENCES DATASETS(dataset_id) ON DELETE CASCADE,
    embedder_id          INTEGER                  NOT NULL REFERENCES EMBEDDERS(embedder_id) ON DELETE CASCADE,
    owner                TEXT                     NOT NULL,
    collection_name      TEXT                     NOT NULL,
    last_processed_at    TIMESTAMP WITH TIME ZONE NULL,
    created_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(dataset_transform_id, embedder_id)
);

-- Optimized indexes for embedded_datasets
CREATE INDEX idx_embedded_datasets_owner_created ON embedded_datasets(owner, created_at DESC);
CREATE INDEX idx_embedded_datasets_transform ON embedded_datasets(dataset_transform_id);
CREATE INDEX idx_embedded_datasets_collection ON embedded_datasets(collection_name);
CREATE INDEX idx_embedded_datasets_last_processed ON embedded_datasets(embedded_dataset_id, last_processed_at);

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
CREATE INDEX idx_dataset_transform_batches_composite ON dataset_transform_batches(dataset_transform_id, status, processed_at DESC);
CREATE INDEX idx_dataset_transform_batches_status ON dataset_transform_batches(status) WHERE status IN ('pending', 'processing');

CREATE TABLE IF NOT EXISTS VISUALIZATION_TRANSFORMS (
    visualization_transform_id SERIAL PRIMARY KEY,
    title                      TEXT                     NOT NULL,
    embedded_dataset_id        INTEGER                  NOT NULL REFERENCES EMBEDDED_DATASETS(embedded_dataset_id) ON DELETE CASCADE,
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
CREATE INDEX idx_visualization_transforms_owner_created ON visualization_transforms(owner, created_at DESC);
CREATE INDEX idx_visualization_transforms_embedded_enabled ON visualization_transforms(embedded_dataset_id, is_enabled);
CREATE INDEX idx_visualization_transforms_enabled ON visualization_transforms(owner, is_enabled) WHERE is_enabled = TRUE;
CREATE INDEX idx_visualization_transforms_status ON visualization_transforms(last_run_status) WHERE last_run_status IS NOT NULL;

CREATE TABLE IF NOT EXISTS VISUALIZATIONS (
    visualization_id           SERIAL PRIMARY KEY,
    visualization_transform_id INTEGER                  NOT NULL REFERENCES VISUALIZATION_TRANSFORMS(visualization_transform_id) ON DELETE CASCADE,
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
CREATE INDEX idx_visualizations_transform_created ON visualizations(visualization_transform_id, created_at DESC);
CREATE INDEX idx_visualizations_status ON visualizations(status) WHERE status IN ('pending', 'processing');

CREATE TABLE IF NOT EXISTS TRANSFORM_PROCESSED_FILES (
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
CREATE INDEX idx_transform_processed_files_type_id_status ON transform_processed_files(transform_type, transform_id, process_status);
CREATE INDEX idx_transform_processed_files_processed_at ON transform_processed_files(processed_at DESC);

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
CREATE INDEX idx_chat_sessions_owner_updated ON chat_sessions(owner, updated_at DESC);
CREATE INDEX idx_chat_sessions_embedded_dataset ON chat_sessions(embedded_dataset_id);

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
CREATE INDEX idx_chat_messages_session_created ON chat_messages(session_id, created_at DESC) INCLUDE (role, status);
CREATE INDEX idx_chat_messages_session_status ON chat_messages(session_id, status) WHERE status != 'complete';

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
CREATE INDEX idx_chat_message_retrieved_docs ON chat_message_retrieved_documents(message_id, similarity_score DESC) 
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
CREATE INDEX idx_audit_events_timestamp ON audit_events(timestamp DESC);
CREATE INDEX idx_audit_events_username_timestamp ON audit_events(username, timestamp DESC);
CREATE INDEX idx_audit_events_type_timestamp ON audit_events(event_type, timestamp DESC);
CREATE INDEX idx_audit_events_resource ON audit_events(resource_type, resource_id) WHERE resource_type IS NOT NULL;

-- ============================================================================
-- Table Comments for Documentation
-- ============================================================================

COMMENT ON TABLE dataset_transform_batches IS 
    'Tracks individual batch processing operations for dataset transforms. Status: pending, processing, success, failed, skipped.';
COMMENT ON TABLE chat_messages IS 
    'Chat messages with streaming support. Status: complete (default), incomplete (streaming), error (failed).';
COMMENT ON TABLE audit_events IS 
    'Security and compliance audit log for all system events.';

