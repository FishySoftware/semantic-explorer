CREATE TABLE collections (
    collection_id       SERIAL PRIMARY KEY,
    title               TEXT                     NOT NULL,
    details             TEXT,
    owner_id            TEXT                     NOT NULL,
    owner_display_name  TEXT                     NOT NULL,
    tags                TEXT[]                   NOT NULL DEFAULT '{}',
    is_public           BOOLEAN                  NOT NULL DEFAULT FALSE,
    file_count          BIGINT                   NOT NULL DEFAULT 0,
    created_at          TIMESTAMPTZ              NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ              NOT NULL DEFAULT NOW()
);

CREATE TABLE datasets (
    dataset_id          SERIAL PRIMARY KEY,
    title               TEXT                     NOT NULL,
    details             TEXT,
    owner_id            TEXT                     NOT NULL,
    owner_display_name  TEXT                     NOT NULL,
    tags                TEXT[]                   NOT NULL DEFAULT '{}',
    is_public           BOOLEAN                  NOT NULL DEFAULT FALSE,
    item_count          INTEGER                  NOT NULL DEFAULT 0,
    total_chunks        BIGINT                   NOT NULL DEFAULT 0,
    created_at          TIMESTAMPTZ              NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ              NOT NULL DEFAULT NOW()
);

CREATE TABLE dataset_items (
    item_id             SERIAL PRIMARY KEY,
    dataset_id          INTEGER                  NOT NULL REFERENCES datasets(dataset_id) ON DELETE CASCADE,
    title               TEXT                     NOT NULL,
    chunks              JSONB                    NOT NULL,
    metadata            JSONB                    NOT NULL DEFAULT '{}',
    created_at          TIMESTAMPTZ              NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ              NOT NULL DEFAULT NOW()
);

CREATE TABLE embedders (
    embedder_id         SERIAL PRIMARY KEY,
    name                TEXT                     NOT NULL,
    owner_id            TEXT                     NOT NULL,
    owner_display_name  TEXT                     NOT NULL,
    provider            TEXT                     NOT NULL,
    base_url            TEXT                     NOT NULL,
    api_key_encrypted   TEXT,
    config              JSONB                    NOT NULL DEFAULT '{}',
    batch_size          INTEGER                  NOT NULL DEFAULT 96,
    dimensions          INTEGER                  NOT NULL DEFAULT 1536,
    max_input_tokens    INTEGER                  NOT NULL DEFAULT 8191,
    truncate_strategy   VARCHAR(50)              NOT NULL DEFAULT 'NONE',
    collection_name     TEXT,
    is_public           BOOLEAN                  NOT NULL DEFAULT FALSE,
    created_at          TIMESTAMPTZ              NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ              NOT NULL DEFAULT NOW()
);

CREATE TABLE llms (
    llm_id              SERIAL PRIMARY KEY,
    name                TEXT                     NOT NULL,
    owner_id            TEXT                     NOT NULL,
    owner_display_name  TEXT                     NOT NULL,
    provider            TEXT                     NOT NULL,
    base_url            TEXT                     NOT NULL,
    api_key_encrypted   TEXT,
    model               TEXT                     NOT NULL,
    config              JSONB                    NOT NULL DEFAULT '{}',
    is_public           BOOLEAN                  NOT NULL DEFAULT FALSE,
    created_at          TIMESTAMPTZ              NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ              NOT NULL DEFAULT NOW()
);

CREATE TABLE collection_transforms (
    collection_transform_id SERIAL PRIMARY KEY,
    title                   TEXT                     NOT NULL,
    collection_id           INTEGER                  NOT NULL REFERENCES collections(collection_id) ON DELETE CASCADE,
    dataset_id              INTEGER                  NOT NULL REFERENCES datasets(dataset_id) ON DELETE CASCADE,
    owner_id                TEXT                     NOT NULL,
    owner_display_name      TEXT                     NOT NULL,
    is_enabled              BOOLEAN                  NOT NULL DEFAULT TRUE,
    chunk_size              INTEGER                  NOT NULL DEFAULT 200,
    job_config              JSONB                    NOT NULL DEFAULT '{}',
    created_at              TIMESTAMPTZ              NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ              NOT NULL DEFAULT NOW()
);

CREATE TABLE dataset_transforms (
    dataset_transform_id SERIAL PRIMARY KEY,
    title                TEXT                     NOT NULL,
    source_dataset_id    INTEGER                  NOT NULL REFERENCES datasets(dataset_id) ON DELETE CASCADE,
    embedder_ids         INTEGER[]                NOT NULL DEFAULT '{}',
    owner_id             TEXT                     NOT NULL,
    owner_display_name   TEXT                     NOT NULL,
    is_enabled           BOOLEAN                  NOT NULL DEFAULT TRUE,
    job_config           JSONB                    NOT NULL DEFAULT '{}',
    created_at           TIMESTAMPTZ              NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMPTZ              NOT NULL DEFAULT NOW()
);

-- No FK constraints: validation handled by validate_embedded_dataset_fks() trigger.
-- dataset_transform_id/source_dataset_id/embedder_id = 0 is the standalone sentinel.
CREATE TABLE embedded_datasets (
    embedded_dataset_id  SERIAL PRIMARY KEY,
    title                TEXT                     NOT NULL,
    dataset_transform_id INTEGER                  NOT NULL,
    source_dataset_id    INTEGER                  NOT NULL,
    embedder_id          INTEGER                  NOT NULL,
    owner_id             TEXT                     NOT NULL,
    owner_display_name   TEXT                     NOT NULL,
    collection_name      TEXT                     NOT NULL,
    dimensions           INTEGER,
    source_dataset_version TIMESTAMPTZ,
    scan_locked_at       TIMESTAMPTZ,
    last_processed_at    TIMESTAMPTZ,
    created_at           TIMESTAMPTZ              NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMPTZ              NOT NULL DEFAULT NOW(),
    UNIQUE(dataset_transform_id, embedder_id)
);

CREATE TABLE dataset_transform_batches (
    id                      SERIAL PRIMARY KEY,
    dataset_transform_id    INTEGER                  NOT NULL REFERENCES dataset_transforms(dataset_transform_id) ON DELETE CASCADE,
    batch_key               VARCHAR(255)             NOT NULL,
    processed_at            TIMESTAMPTZ              NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status                  VARCHAR(50)              NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'success', 'failed', 'skipped')),
    chunk_count             INTEGER                  NOT NULL DEFAULT 0,
    error_message           TEXT,
    processing_duration_ms  BIGINT,
    created_at              TIMESTAMPTZ              NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at              TIMESTAMPTZ              NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(dataset_transform_id, batch_key)
);

CREATE TABLE dataset_transform_stats (
    dataset_transform_id        INTEGER                  PRIMARY KEY REFERENCES dataset_transforms(dataset_transform_id) ON DELETE CASCADE,
    total_chunks_embedded       BIGINT                   NOT NULL DEFAULT 0,
    total_chunks_processing     BIGINT                   NOT NULL DEFAULT 0,
    total_chunks_failed         BIGINT                   NOT NULL DEFAULT 0,
    total_chunks_to_process     BIGINT                   NOT NULL DEFAULT 0,
    successful_batches          BIGINT                   NOT NULL DEFAULT 0,
    failed_batches              BIGINT                   NOT NULL DEFAULT 0,
    processing_batches          BIGINT                   NOT NULL DEFAULT 0,
    total_batches_dispatched    BIGINT                   NOT NULL DEFAULT 0,
    total_chunks_dispatched     BIGINT                   NOT NULL DEFAULT 0,
    current_run_id              TEXT,
    current_run_started_at      TIMESTAMPTZ,
    last_processed_at           TIMESTAMPTZ,
    first_processing_at         TIMESTAMPTZ,
    created_at                  TIMESTAMPTZ              NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at                  TIMESTAMPTZ              NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE pending_batches (
    id                      SERIAL PRIMARY KEY,
    batch_type              TEXT                     NOT NULL CHECK (batch_type IN ('dataset', 'collection', 'visualization')),
    dataset_transform_id    INTEGER                  REFERENCES dataset_transforms(dataset_transform_id) ON DELETE CASCADE,
    embedded_dataset_id     INTEGER                  REFERENCES embedded_datasets(embedded_dataset_id) ON DELETE CASCADE,
    collection_transform_id INTEGER                  REFERENCES collection_transforms(collection_transform_id) ON DELETE CASCADE,
    batch_key               TEXT                     NOT NULL,
    s3_bucket               TEXT                     NOT NULL,
    job_payload             JSONB                    NOT NULL,
    retry_count             INTEGER                  NOT NULL DEFAULT 0,
    max_retries             INTEGER                  NOT NULL DEFAULT 10,
    last_error              TEXT,
    next_retry_at           TIMESTAMPTZ              NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status                  TEXT                     NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'published', 'failed', 'expired')),
    created_at              TIMESTAMPTZ              NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at              TIMESTAMPTZ              NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE reconciliation_runs (
    id                      SERIAL PRIMARY KEY,
    run_type                TEXT                     NOT NULL CHECK (run_type IN ('dataset', 'collection', 'all')),
    dataset_transform_id    INTEGER                  REFERENCES dataset_transforms(dataset_transform_id) ON DELETE SET NULL,
    started_at              TIMESTAMPTZ              NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at            TIMESTAMPTZ,
    status                  TEXT                     NOT NULL DEFAULT 'running' CHECK (status IN ('running', 'completed', 'failed')),
    orphaned_batches_found  INTEGER                  NOT NULL DEFAULT 0,
    batches_recovered       INTEGER                  NOT NULL DEFAULT 0,
    batches_cleaned_up      INTEGER                  NOT NULL DEFAULT 0,
    error_message           TEXT,
    details                 JSONB
);

CREATE TABLE visualization_transforms (
    visualization_transform_id SERIAL PRIMARY KEY,
    title                      TEXT                     NOT NULL,
    embedded_dataset_id        INTEGER                  NOT NULL REFERENCES embedded_datasets(embedded_dataset_id) ON DELETE CASCADE,
    owner_id                   TEXT                     NOT NULL,
    owner_display_name         TEXT                     NOT NULL,
    is_enabled                 BOOLEAN                  NOT NULL DEFAULT TRUE,
    reduced_collection_name    TEXT,
    topics_collection_name     TEXT,
    visualization_config       JSONB                    NOT NULL DEFAULT '{}',
    last_run_status            TEXT,
    last_run_at                TIMESTAMPTZ,
    last_error                 TEXT,
    last_run_stats             JSONB,
    created_at                 TIMESTAMPTZ              NOT NULL DEFAULT NOW(),
    updated_at                 TIMESTAMPTZ              NOT NULL DEFAULT NOW()
);

CREATE TABLE visualizations (
    visualization_id           SERIAL PRIMARY KEY,
    visualization_transform_id INTEGER                  NOT NULL REFERENCES visualization_transforms(visualization_transform_id) ON DELETE CASCADE,
    status                     TEXT                     NOT NULL CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    started_at                 TIMESTAMPTZ,
    completed_at               TIMESTAMPTZ,
    html_s3_key                TEXT,
    point_count                INTEGER,
    cluster_count              INTEGER,
    error_message              TEXT,
    stats_json                 JSONB,
    created_at                 TIMESTAMPTZ              NOT NULL DEFAULT NOW()
);

CREATE TABLE transform_processed_files (
    id                      SERIAL PRIMARY KEY,
    transform_type          TEXT                     NOT NULL,
    transform_id            INTEGER                  NOT NULL,
    file_key                TEXT                     NOT NULL,
    processed_at            TIMESTAMPTZ              NOT NULL DEFAULT NOW(),
    item_count              INTEGER                  NOT NULL DEFAULT 0,
    process_status          TEXT                     NOT NULL DEFAULT 'completed',
    process_error           TEXT,
    processing_duration_ms  BIGINT,
    UNIQUE(transform_type, transform_id, file_key)
);

CREATE TABLE chat_sessions (
    session_id          TEXT PRIMARY KEY,
    owner_id            TEXT                     NOT NULL,
    owner_display_name  TEXT                     NOT NULL,
    embedded_dataset_id INTEGER                  NOT NULL REFERENCES embedded_datasets(embedded_dataset_id) ON DELETE CASCADE,
    llm_id              INTEGER                  NOT NULL REFERENCES llms(llm_id) ON DELETE CASCADE,
    title               TEXT                     NOT NULL DEFAULT '',
    created_at          TIMESTAMPTZ              NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ              NOT NULL DEFAULT NOW()
);

CREATE TABLE chat_messages (
    message_id          SERIAL PRIMARY KEY,
    session_id          TEXT                     NOT NULL REFERENCES chat_sessions(session_id) ON DELETE CASCADE,
    role                TEXT                     NOT NULL CHECK (role IN ('user', 'assistant')),
    content             TEXT                     NOT NULL,
    documents_retrieved INTEGER,
    status              TEXT                     NOT NULL DEFAULT 'complete' CHECK (status IN ('complete', 'incomplete', 'error')),
    created_at          TIMESTAMPTZ              NOT NULL DEFAULT NOW()
);

CREATE TABLE chat_message_retrieved_documents (
    id               SERIAL PRIMARY KEY,
    message_id       INTEGER                  NOT NULL REFERENCES chat_messages(message_id) ON DELETE CASCADE,
    document_id      TEXT,
    text             TEXT                     NOT NULL,
    similarity_score REAL                     NOT NULL,
    item_title       TEXT,
    source           TEXT,
    created_at       TIMESTAMPTZ              NOT NULL DEFAULT NOW()
);

CREATE TABLE audit_events (
    audit_event_id   BIGSERIAL PRIMARY KEY,
    timestamp        TIMESTAMPTZ              NOT NULL DEFAULT NOW(),
    event_type       TEXT                     NOT NULL,
    outcome          TEXT                     NOT NULL,
    user_id          TEXT                     NOT NULL,
    username_display TEXT                     NOT NULL,
    resource_type    TEXT,
    resource_id      TEXT,
    details          TEXT
);
