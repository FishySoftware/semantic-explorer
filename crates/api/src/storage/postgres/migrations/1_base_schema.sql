-- ============================================================================
-- Semantic Explorer - Unified Database Schema
-- ============================================================================
--
-- Schema includes:
-- 1. Collections (raw file storage)
-- 2. Datasets (structured data)
-- 3. Dataset Items
-- 4. Embedders (embedding providers)
-- 5. Collection Transforms (Collection → Dataset)
-- 6. Dataset Transforms (Dataset → Embeddings)
-- 7. Embedded Datasets (embedding results)
-- 8. Visualization Transforms (Embedded Dataset → 3D visualization)
-- 9. Transform Processed Files (job processing history)
-- 10. LLMs (Large Language Models for chat)
-- 11. Chat Sessions (RAG chat conversations)
-- 12. Chat Messages (conversation history)
-- 13. Chat Message Retrieved Documents (RAG document tracking)

-- ============================================================================
-- USERS TABLE
-- ============================================================================
CREATE TABLE IF NOT EXISTS users (
    username TEXT PRIMARY KEY,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- COLLECTIONS: Raw file storage
-- ============================================================================
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
CREATE INDEX IF NOT EXISTS idx_collections_public
    ON COLLECTIONS(is_public) WHERE is_public = TRUE;
CREATE INDEX IF NOT EXISTS idx_collections_is_public
    ON collections(is_public)
    WHERE is_public = TRUE;
CREATE INDEX IF NOT EXISTS idx_collections_owner_created
    ON collections(owner, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_collections_public_owner
    ON COLLECTIONS(is_public, owner);

-- ============================================================================
-- DATASETS: Structured data
-- ============================================================================
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

CREATE INDEX IF NOT EXISTS idx_datasets_public
    ON DATASETS(is_public) WHERE is_public = TRUE;
CREATE INDEX IF NOT EXISTS idx_datasets_is_public
    ON datasets(is_public)
    WHERE is_public = TRUE;
CREATE INDEX IF NOT EXISTS idx_datasets_owner_created
    ON datasets(owner, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_datasets_public_owner
    ON DATASETS(is_public, owner);

-- ============================================================================
-- DATASET_ITEMS: Items within datasets
-- ============================================================================
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
CREATE INDEX IF NOT EXISTS idx_dataset_items_dataset_item
    ON DATASET_ITEMS(dataset_id, item_id);
CREATE INDEX IF NOT EXISTS idx_dataset_items_dataset_created
    ON dataset_items(dataset_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_dataset_items_updated_at
    ON dataset_items(dataset_id, updated_at DESC);

-- Unique constraint to prevent duplicate items in a dataset
CREATE UNIQUE INDEX IF NOT EXISTS idx_dataset_items_dataset_title_unique
    ON dataset_items(dataset_id, title);

-- ============================================================================
-- EMBEDDERS: Embedding providers
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

CREATE INDEX IF NOT EXISTS idx_embedders_owner ON EMBEDDERS(owner);
CREATE INDEX IF NOT EXISTS idx_embedders_provider ON EMBEDDERS(provider);
CREATE INDEX IF NOT EXISTS idx_embedders_batch_size ON EMBEDDERS(batch_size);
CREATE INDEX IF NOT EXISTS idx_embedders_max_batch_size ON EMBEDDERS(max_batch_size);
CREATE INDEX IF NOT EXISTS idx_embedders_dimensions ON EMBEDDERS(dimensions);
CREATE INDEX IF NOT EXISTS idx_embedders_max_input_tokens ON embedders(max_input_tokens);
CREATE INDEX IF NOT EXISTS idx_embedders_owner_id
    ON EMBEDDERS(owner, embedder_id);
CREATE INDEX IF NOT EXISTS idx_embedders_public
    ON EMBEDDERS(is_public) WHERE is_public = TRUE;
CREATE INDEX IF NOT EXISTS idx_embedders_owner_public
    ON embedders(owner, is_public);
CREATE INDEX IF NOT EXISTS idx_embedders_public_owner
    ON EMBEDDERS(is_public, owner);

COMMENT ON COLUMN embedders.max_input_tokens IS 'Maximum input tokens accepted by this embedder model';
COMMENT ON COLUMN embedders.truncate_strategy IS 'Text truncation strategy: NONE, START, END, or custom value';

-- ============================================================================
-- COLLECTION_TRANSFORMS: Collection → Dataset (file extraction & chunking)
-- ============================================================================
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

CREATE INDEX IF NOT EXISTS idx_collection_transforms_owner ON COLLECTION_TRANSFORMS(owner);
CREATE INDEX IF NOT EXISTS idx_collection_transforms_enabled ON COLLECTION_TRANSFORMS(is_enabled);
CREATE INDEX IF NOT EXISTS idx_collection_transforms_collection_id ON COLLECTION_TRANSFORMS(collection_id);
CREATE INDEX IF NOT EXISTS idx_collection_transforms_dataset_id ON COLLECTION_TRANSFORMS(dataset_id);
CREATE INDEX IF NOT EXISTS idx_collection_transforms_owner_enabled
    ON COLLECTION_TRANSFORMS(owner, is_enabled)
    WHERE is_enabled = TRUE;
CREATE INDEX IF NOT EXISTS idx_collection_transforms_dataset
    ON collection_transforms(dataset_id);

-- ============================================================================
-- DATASET_TRANSFORMS: Dataset → Embedded Datasets (embedding with 1-N embedders)
-- ============================================================================
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

CREATE INDEX IF NOT EXISTS idx_dataset_transforms_owner ON DATASET_TRANSFORMS(owner);
CREATE INDEX IF NOT EXISTS idx_dataset_transforms_enabled ON DATASET_TRANSFORMS(is_enabled);
CREATE INDEX IF NOT EXISTS idx_dataset_transforms_source_dataset_id ON DATASET_TRANSFORMS(source_dataset_id);
CREATE INDEX IF NOT EXISTS idx_dataset_transforms_owner_enabled
    ON DATASET_TRANSFORMS(owner, is_enabled)
    WHERE is_enabled = TRUE;

-- ============================================================================
-- EMBEDDED_DATASETS: Result entity (one per embedder from Dataset Transform)
-- ============================================================================
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

CREATE INDEX IF NOT EXISTS idx_embedded_datasets_owner ON EMBEDDED_DATASETS(owner);
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_dataset_transform_id ON EMBEDDED_DATASETS(dataset_transform_id);
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_source_dataset_id ON EMBEDDED_DATASETS(source_dataset_id);
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_embedder_id ON EMBEDDED_DATASETS(embedder_id);
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_collection_name ON EMBEDDED_DATASETS(collection_name);
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_embedder
    ON embedded_datasets(embedder_id);
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_source
    ON embedded_datasets(source_dataset_id);
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_owner_created
    ON embedded_datasets(owner, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_last_processed_at
    ON embedded_datasets(embedded_dataset_id, last_processed_at);

-- ============================================================================
-- VISUALIZATION_TRANSFORMS: Embedded Dataset → 3D visualization
-- ============================================================================
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

CREATE INDEX IF NOT EXISTS idx_visualization_transforms_owner ON VISUALIZATION_TRANSFORMS(owner);
CREATE INDEX IF NOT EXISTS idx_visualization_transforms_enabled ON VISUALIZATION_TRANSFORMS(is_enabled);
CREATE INDEX IF NOT EXISTS idx_visualization_transforms_embedded_dataset_id ON VISUALIZATION_TRANSFORMS(embedded_dataset_id);
CREATE INDEX IF NOT EXISTS idx_visualization_transforms_owner_enabled
    ON VISUALIZATION_TRANSFORMS(owner, is_enabled)
    WHERE is_enabled = TRUE;
CREATE INDEX IF NOT EXISTS idx_visualization_transforms_last_run_status
    ON VISUALIZATION_TRANSFORMS(last_run_status);

-- ============================================================================
-- TRANSFORM_PROCESSED_FILES: Shared tracking table for all transform types
-- ============================================================================
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

CREATE INDEX IF NOT EXISTS idx_transform_processed_files_type_id ON TRANSFORM_PROCESSED_FILES(transform_type, transform_id);
CREATE INDEX IF NOT EXISTS idx_transform_processed_files_status ON TRANSFORM_PROCESSED_FILES(process_status);
CREATE INDEX IF NOT EXISTS idx_transform_processed_files_processed_at ON TRANSFORM_PROCESSED_FILES(processed_at);
CREATE INDEX IF NOT EXISTS idx_transform_files_transform_status
    ON transform_processed_files(transform_id, process_status);
CREATE INDEX IF NOT EXISTS idx_transform_files_processed_at
    ON transform_processed_files(processed_at DESC);

-- ============================================================================
-- LLMS: Large Language Models for chat and topic naming
-- ============================================================================
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
    ON LLMS(owner);
CREATE INDEX IF NOT EXISTS idx_llms_provider
    ON LLMS(provider);
CREATE INDEX IF NOT EXISTS idx_llms_public
    ON LLMS(is_public) WHERE is_public = TRUE;
CREATE INDEX IF NOT EXISTS idx_llms_public_owner
    ON LLMS(is_public, owner);

-- ============================================================================
-- CHAT_SESSIONS: RAG chat conversation sessions
-- ============================================================================
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

-- ============================================================================
-- CHAT_MESSAGES: Chat conversation message history
-- ============================================================================
CREATE TABLE IF NOT EXISTS chat_messages (
    message_id SERIAL PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES chat_sessions(session_id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    content TEXT NOT NULL,
    documents_retrieved INTEGER,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id);
CREATE INDEX IF NOT EXISTS idx_chat_messages_created_at ON chat_messages(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_chat_messages_conversation_created
    ON chat_messages(session_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_chat_messages_conversation_role
    ON chat_messages(session_id, role);

-- ============================================================================
-- CHAT_MESSAGE_RETRIEVED_DOCUMENTS: Retrieved documents for chat messages
-- ============================================================================
CREATE TABLE IF NOT EXISTS chat_message_retrieved_documents (
    id SERIAL PRIMARY KEY,
    message_id INTEGER NOT NULL REFERENCES chat_messages(message_id) ON DELETE CASCADE,
    document_id TEXT,
    text TEXT NOT NULL,
    similarity_score REAL NOT NULL,
    source TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_chat_message_retrieved_documents_message_id 
    ON chat_message_retrieved_documents(message_id);
CREATE INDEX IF NOT EXISTS idx_chat_message_retrieved_documents_score 
    ON chat_message_retrieved_documents(message_id, similarity_score DESC);

-- ============================================================================
-- COMPREHENSIVE QUERY OPTIMIZATION INDICES
-- ============================================================================

-- TRANSFORMS tables optimization
CREATE INDEX IF NOT EXISTS idx_transforms_collection_id
    ON collection_transforms(collection_id);
CREATE INDEX IF NOT EXISTS idx_transforms_source_dataset
    ON dataset_transforms(source_dataset_id);
CREATE INDEX IF NOT EXISTS idx_transforms_target_dataset
    ON collection_transforms(dataset_id);
CREATE INDEX IF NOT EXISTS idx_transforms_owner_type_enabled
    ON collection_transforms(owner, collection_transform_id, is_enabled)
    WHERE is_enabled = TRUE;
