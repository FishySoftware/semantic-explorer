CREATE TABLE IF NOT EXISTS users (
    username TEXT PRIMARY KEY,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS COLLECTIONS (
    collection_id    SERIAL PRIMARY KEY,
    title            TEXT                     NOT NULL,
    details          TEXT                     NULL,
    owner            TEXT                     NOT NULL,
    bucket           TEXT                     NOT NULL,
    tags             TEXT[]                   NOT NULL,
    is_public        BOOLEAN                  NOT NULL DEFAULT FALSE,
    created_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_collections_owner ON collections(owner);
CREATE INDEX IF NOT EXISTS idx_collections_is_public
    ON collections(is_public)
    WHERE is_public = TRUE;
CREATE INDEX IF NOT EXISTS idx_collections_owner_created
    ON collections(owner, created_at DESC);


CREATE TABLE IF NOT EXISTS DATASETS (
    dataset_id       SERIAL PRIMARY KEY,
    title            TEXT                     NOT NULL,
    details          TEXT                     NULL,
    owner            TEXT                     NOT NULL,
    tags             TEXT[]                   NOT NULL,
    is_public        BOOLEAN                  NOT NULL DEFAULT FALSE,
    created_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_datasets_owner
    ON datasets(owner);
CREATE INDEX IF NOT EXISTS idx_datasets_is_public
    ON datasets(is_public)
    WHERE is_public = TRUE;
CREATE INDEX IF NOT EXISTS idx_datasets_owner_created
    ON datasets(owner, created_at DESC);

CREATE TABLE IF NOT EXISTS DATASET_ITEMS (
    item_id          SERIAL PRIMARY KEY,
    dataset_id       INTEGER             NOT NULL,
    title            TEXT                NOT NULL,
    chunks           JSONB               NOT NULL,
    metadata         JSONB               NOT NULL,
    created_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    FOREIGN KEY (dataset_id) REFERENCES DATASETS(dataset_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_dataset_items_dataset_id ON dataset_items(dataset_id);
CREATE INDEX IF NOT EXISTS idx_dataset_items_dataset_created
    ON dataset_items(dataset_id, created_at DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_dataset_items_dataset_title_unique
    ON dataset_items(dataset_id, title);

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

CREATE INDEX IF NOT EXISTS idx_embedders_owner ON embedders(owner);
CREATE INDEX IF NOT EXISTS idx_embedders_provider ON embedders(provider);
CREATE INDEX IF NOT EXISTS idx_embedders_is_public
    ON embedders(is_public) WHERE is_public = TRUE;
CREATE INDEX IF NOT EXISTS idx_embedders_owner_public
    ON embedders(owner, is_public);

CREATE TABLE IF NOT EXISTS COLLECTION_TRANSFORMS (
    collection_transform_id SERIAL PRIMARY KEY,
    title                   TEXT                     NOT NULL,
    collection_id           INTEGER                  NOT NULL,
    dataset_id              INTEGER                  NOT NULL,
    owner                   TEXT                     NOT NULL,
    is_enabled              BOOLEAN                  NOT NULL DEFAULT TRUE,
    chunk_size              INTEGER                  NOT NULL DEFAULT 200,
    job_config              JSONB                    NOT NULL DEFAULT '{}',
    created_at              TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    FOREIGN KEY (collection_id) REFERENCES COLLECTIONS(collection_id) ON DELETE CASCADE,
    FOREIGN KEY (dataset_id) REFERENCES DATASETS(dataset_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_collection_transforms_owner ON collection_transforms(owner);
CREATE INDEX IF NOT EXISTS idx_collection_transforms_collection_id ON collection_transforms(collection_id);
CREATE INDEX IF NOT EXISTS idx_collection_transforms_dataset_id ON collection_transforms(dataset_id);
CREATE INDEX IF NOT EXISTS idx_collection_transforms_owner_enabled
    ON collection_transforms(owner, is_enabled)
    WHERE is_enabled = TRUE;
CREATE INDEX IF NOT EXISTS idx_collection_transforms_owner_created
    ON collection_transforms(owner, created_at DESC);

CREATE TABLE IF NOT EXISTS DATASET_TRANSFORMS (
    dataset_transform_id SERIAL PRIMARY KEY,
    title                TEXT                     NOT NULL,
    source_dataset_id    INTEGER                  NOT NULL,
    embedder_ids         INTEGER[]                NOT NULL,
    owner                TEXT                     NOT NULL,
    is_enabled           BOOLEAN                  NOT NULL DEFAULT TRUE,
    job_config           JSONB                    NOT NULL DEFAULT '{}',
    created_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    FOREIGN KEY (source_dataset_id) REFERENCES DATASETS(dataset_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_dataset_transforms_owner ON dataset_transforms(owner);
CREATE INDEX IF NOT EXISTS idx_dataset_transforms_source_dataset_id ON dataset_transforms(source_dataset_id);
CREATE INDEX IF NOT EXISTS idx_dataset_transforms_owner_enabled
    ON dataset_transforms(owner, is_enabled)
    WHERE is_enabled = TRUE;
CREATE INDEX IF NOT EXISTS idx_dataset_transforms_owner_created
    ON dataset_transforms(owner, created_at DESC);

CREATE TABLE IF NOT EXISTS EMBEDDED_DATASETS (
    embedded_dataset_id  SERIAL PRIMARY KEY,
    title                TEXT                     NOT NULL,
    dataset_transform_id INTEGER                  NOT NULL,
    source_dataset_id    INTEGER                  NOT NULL,
    embedder_id          INTEGER                  NOT NULL,
    owner                TEXT                     NOT NULL,
    collection_name      TEXT                     NOT NULL,
    last_processed_at    TIMESTAMP WITH TIME ZONE NULL,
    created_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    FOREIGN KEY (dataset_transform_id) REFERENCES DATASET_TRANSFORMS(dataset_transform_id) ON DELETE CASCADE,
    FOREIGN KEY (source_dataset_id) REFERENCES DATASETS(dataset_id) ON DELETE CASCADE,
    FOREIGN KEY (embedder_id) REFERENCES EMBEDDERS(embedder_id) ON DELETE CASCADE,
    UNIQUE(dataset_transform_id, embedder_id)
);

CREATE INDEX IF NOT EXISTS idx_embedded_datasets_owner ON embedded_datasets(owner);
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_dataset_transform_id ON embedded_datasets(dataset_transform_id);
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_source_dataset_id ON embedded_datasets(source_dataset_id);
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_embedder_id ON embedded_datasets(embedder_id);
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_collection_name ON embedded_datasets(collection_name);
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_owner_created
    ON embedded_datasets(owner, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_last_processed_at
    ON embedded_datasets(embedded_dataset_id, last_processed_at);

CREATE TABLE IF NOT EXISTS VISUALIZATION_TRANSFORMS (
    visualization_transform_id SERIAL PRIMARY KEY,
    title                      TEXT                     NOT NULL,
    embedded_dataset_id        INTEGER                  NOT NULL,
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
    updated_at                 TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    FOREIGN KEY (embedded_dataset_id) REFERENCES EMBEDDED_DATASETS(embedded_dataset_id) ON DELETE CASCADE
);



-- Audit events table for security and compliance monitoring
CREATE TABLE IF NOT EXISTS audit_events (
    audit_event_id      BIGSERIAL PRIMARY KEY,
    timestamp           TIMESTAMP WITH TIME ZONE NOT NULL,
    event_type          TEXT                     NOT NULL,
    outcome             TEXT                     NOT NULL,
    username            TEXT                     NOT NULL,
    request_id          TEXT                     NULL,
    client_ip           INET                     NULL,
    resource_type       TEXT                     NULL,
    resource_id         TEXT                     NULL,
    details             TEXT                     NULL,
    created_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Indexes for common audit queries
CREATE INDEX IF NOT EXISTS idx_audit_events_timestamp
    ON audit_events(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_events_username
    ON audit_events(username);
CREATE INDEX IF NOT EXISTS idx_audit_events_event_type
    ON audit_events(event_type);
CREATE INDEX IF NOT EXISTS idx_audit_events_outcome
    ON audit_events(outcome);
CREATE INDEX IF NOT EXISTS idx_audit_events_resource_type
    ON audit_events(resource_type);
CREATE INDEX IF NOT EXISTS idx_audit_events_username_timestamp
    ON audit_events(username, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_events_event_type_timestamp
    ON audit_events(event_type, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_events_username_event_type_timestamp
    ON audit_events(username, event_type, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_events_resource
    ON audit_events(resource_type, resource_id);

-- Create dataset_transform_batches table for tracking batch processing operations
CREATE TABLE IF NOT EXISTS dataset_transform_batches (
    id SERIAL PRIMARY KEY,
    dataset_transform_id INTEGER NOT NULL REFERENCES dataset_transforms(dataset_transform_id) ON DELETE CASCADE,
    batch_key VARCHAR(255) NOT NULL,
    processed_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    chunk_count INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    processing_duration_ms BIGINT,
    
    -- Audit fields
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_dataset_transform_batches_transform_id
    ON dataset_transform_batches(dataset_transform_id);
CREATE INDEX IF NOT EXISTS idx_dataset_transform_batches_status
    ON dataset_transform_batches(status);
CREATE INDEX IF NOT EXISTS idx_dataset_transform_batches_composite
    ON dataset_transform_batches(dataset_transform_id, status, processed_at DESC);

-- Add comment for documentation
COMMENT ON TABLE dataset_transform_batches IS 
    'Tracks individual batch processing operations for dataset transforms. Each batch record corresponds to a processing event from the dataset transform worker.';
COMMENT ON COLUMN dataset_transform_batches.batch_key IS 
    'Unique identifier for the batch, typically filename or chunk group ID from the source dataset.';
COMMENT ON COLUMN dataset_transform_batches.status IS 
    'Processing status: pending, processing, success, failed, skipped.';
COMMENT ON COLUMN dataset_transform_batches.chunk_count IS 
    'Number of chunks in this batch.';
COMMENT ON COLUMN dataset_transform_batches.processing_duration_ms IS 
    'Time taken to process this batch in milliseconds.';

CREATE INDEX IF NOT EXISTS idx_visualization_transforms_owner ON visualization_transforms(owner);
CREATE INDEX IF NOT EXISTS idx_visualization_transforms_embedded_dataset_id ON visualization_transforms(embedded_dataset_id);
CREATE INDEX IF NOT EXISTS idx_visualization_transforms_owner_enabled
    ON visualization_transforms(owner, is_enabled)
    WHERE is_enabled = TRUE;
CREATE INDEX IF NOT EXISTS idx_visualization_transforms_owner_created
    ON visualization_transforms(owner, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_visualization_transforms_last_run_status
    ON visualization_transforms(last_run_status);

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

CREATE INDEX IF NOT EXISTS idx_transform_processed_files_type_id ON transform_processed_files(transform_type, transform_id);
CREATE INDEX IF NOT EXISTS idx_transform_processed_files_status ON transform_processed_files(process_status);
CREATE INDEX IF NOT EXISTS idx_transform_processed_files_processed_at ON transform_processed_files(processed_at DESC);

CREATE TABLE IF NOT EXISTS LLMS (
    llm_id               SERIAL PRIMARY KEY,
    name                 TEXT                     NOT NULL,
    owner                TEXT                     NOT NULL,
    provider             TEXT                     NOT NULL,
    base_url             TEXT                     NOT NULL,
    api_key              TEXT                     NULL,
    config               JSONB                    NOT NULL DEFAULT '{}',
    is_public            BOOLEAN                  NOT NULL DEFAULT FALSE,
    created_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_llms_owner
    ON llms(owner);
CREATE INDEX IF NOT EXISTS idx_llms_provider
    ON llms(provider);
CREATE INDEX IF NOT EXISTS idx_llms_is_public
    ON llms(is_public) WHERE is_public = TRUE;

CREATE TABLE IF NOT EXISTS chat_sessions (
    session_id TEXT PRIMARY KEY,
    owner TEXT NOT NULL REFERENCES users(username) ON DELETE CASCADE,
    embedded_dataset_id INTEGER NOT NULL REFERENCES embedded_datasets(embedded_dataset_id) ON DELETE CASCADE,
    llm_id INTEGER NOT NULL REFERENCES llms(llm_id) ON DELETE CASCADE,
    title TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_chat_sessions_owner ON chat_sessions(owner);
CREATE INDEX IF NOT EXISTS idx_chat_sessions_embedded_dataset ON chat_sessions(embedded_dataset_id);
CREATE INDEX IF NOT EXISTS idx_chat_sessions_updated_at ON chat_sessions(updated_at DESC);

CREATE TABLE IF NOT EXISTS chat_messages (
    message_id SERIAL PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES chat_sessions(session_id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    content TEXT NOT NULL,
    documents_retrieved INTEGER,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id);
CREATE INDEX IF NOT EXISTS idx_chat_messages_session_created
    ON chat_messages(session_id, created_at DESC);

CREATE TABLE IF NOT EXISTS chat_message_retrieved_documents (
    id SERIAL PRIMARY KEY,
    message_id INTEGER NOT NULL REFERENCES chat_messages(message_id) ON DELETE CASCADE,
    document_id TEXT,
    text TEXT NOT NULL,
    similarity_score REAL NOT NULL,
    item_title TEXT,
    source TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_chat_message_retrieved_documents_message_id 
    ON chat_message_retrieved_documents(message_id);
CREATE INDEX IF NOT EXISTS idx_chat_message_retrieved_documents_score 
    ON chat_message_retrieved_documents(message_id, similarity_score DESC);




CREATE TABLE IF NOT EXISTS VISUALIZATIONS (
    visualization_id        SERIAL PRIMARY KEY,
    visualization_transform_id INTEGER NOT NULL,
    status                  TEXT NOT NULL CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    started_at              TIMESTAMP WITH TIME ZONE NULL,
    completed_at            TIMESTAMP WITH TIME ZONE NULL,
    html_s3_key             TEXT NULL,
    point_count             INTEGER NULL,
    cluster_count           INTEGER NULL,
    error_message           TEXT NULL,
    stats_json              JSONB NULL,
    created_at              TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    FOREIGN KEY (visualization_transform_id) REFERENCES VISUALIZATION_TRANSFORMS(visualization_transform_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_visualizations_transform_id 
    ON visualizations(visualization_transform_id);
CREATE INDEX IF NOT EXISTS idx_visualizations_status 
    ON visualizations(status);
CREATE INDEX IF NOT EXISTS idx_visualizations_transform_created
    ON visualizations(visualization_transform_id, created_at DESC);
