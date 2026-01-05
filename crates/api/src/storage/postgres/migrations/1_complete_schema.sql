-- Complete database schema for Semantic Explorer
-- Collections for raw file storage
CREATE TABLE IF NOT EXISTS COLLECTIONS
(
    collection_id    SERIAL PRIMARY KEY,
    title            TEXT                     NOT NULL,
    details          TEXT                     NULL,
    owner           TEXT                     NOT NULL,
    bucket           TEXT                     NOT NULL,
    tags             TEXT[]                   NOT NULL,
    created_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_collections_owner ON collections(owner);

-- Datasets for structured data
CREATE TABLE IF NOT EXISTS DATASETS
(
    dataset_id       SERIAL PRIMARY KEY,
    title            TEXT                     NOT NULL,
    details          TEXT                     NULL,
    owner           TEXT                     NOT NULL,
    tags             TEXT[]                   NOT NULL,
    created_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_datasets_owner ON datasets(owner);

-- Dataset items with chunks and metadata
CREATE TABLE IF NOT EXISTS DATASET_ITEMS
(
    item_id          SERIAL PRIMARY KEY,
    dataset_id       INTEGER             NOT NULL,
    title            TEXT                NOT NULL,
    chunks           TEXT[]              NOT NULL,
    metadata         JSONB               NOT NULL,
    FOREIGN KEY (dataset_id) REFERENCES DATASETS(dataset_id) ON DELETE CASCADE
);
CREATE INDEX idx_dataset_items_dataset_id ON dataset_items(dataset_id);

-- Composite index for dataset item lookups by dataset
-- Used by: get_dataset_items, delete_dataset_item
CREATE INDEX IF NOT EXISTS idx_dataset_items_dataset_item
    ON DATASET_ITEMS(dataset_id, item_id);

-- Embedders for managing embedding providers
CREATE TABLE IF NOT EXISTS EMBEDDERS
(
    embedder_id          SERIAL PRIMARY KEY,
    name                 TEXT                     NOT NULL,
    owner               TEXT                     NOT NULL,
    provider             TEXT                     NOT NULL,
    base_url             TEXT                     NOT NULL,
    api_key              TEXT                     NULL,
    config               JSONB                    NOT NULL DEFAULT '{}',
    batch_size           INTEGER                  NOT NULL DEFAULT 100,
    collection_name      TEXT                     NOT NULL,
    created_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_embedders_owner ON EMBEDDERS(owner);
CREATE INDEX idx_embedders_provider ON EMBEDDERS(provider);
CREATE INDEX idx_embedders_batch_size ON EMBEDDERS(batch_size);

-- Index for embedder batch lookups by ID
-- Used by: get_embedders_by_ids batch validation
CREATE INDEX IF NOT EXISTS idx_embedders_owner_id
    ON EMBEDDERS(owner, embedder_id);

-- Transforms for job processing (Collection->Dataset, Dataset->Embeddings, API Transform)
CREATE TABLE IF NOT EXISTS TRANSFORMS
(
    transform_id        SERIAL PRIMARY KEY,
    title               TEXT                     NOT NULL,
    collection_id       INTEGER                  NULL,                   -- Nullable, only for CollectionToDataset
    dataset_id          INTEGER                  NOT NULL,               -- Target dataset for most transforms
    owner              TEXT                     NOT NULL,
    chunk_size          INTEGER                  NOT NULL DEFAULT 200,
    is_enabled          BOOLEAN                  NOT NULL DEFAULT TRUE,
    job_type            TEXT                     NOT NULL DEFAULT 'collection_to_dataset',
    source_dataset_id   INTEGER                  NULL,                   -- Source dataset for GenericTransform
    target_dataset_id   INTEGER                  NULL,                   -- Target dataset for GenericTransform
    embedder_ids        INTEGER[]                NULL,                   -- Array of embedder IDs for DatasetToVectorStorage
    job_config          JSONB                    NOT NULL DEFAULT '{}',
    created_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    FOREIGN KEY (collection_id) REFERENCES COLLECTIONS(collection_id) ON DELETE CASCADE,
    FOREIGN KEY (dataset_id) REFERENCES DATASETS(dataset_id) ON DELETE CASCADE,
    FOREIGN KEY (source_dataset_id) REFERENCES DATASETS(dataset_id) ON DELETE CASCADE,
    FOREIGN KEY (target_dataset_id) REFERENCES DATASETS(dataset_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_transforms_owner ON TRANSFORMS(owner);
CREATE INDEX IF NOT EXISTS idx_transforms_enabled ON TRANSFORMS(is_enabled);
CREATE INDEX IF NOT EXISTS idx_transforms_job_type ON TRANSFORMS(job_type);

-- Composite index for querying active transforms by owner
-- Used by: GET_ACTIVE_TRANSFORMS_QUERY, scanning enabled transforms
CREATE INDEX IF NOT EXISTS idx_transforms_owner_enabled
    ON TRANSFORMS(owner, is_enabled)
    WHERE is_enabled = TRUE;

-- Transform processing history
CREATE TABLE IF NOT EXISTS TRANSFORM_PROCESSED_FILES
(
    id                  SERIAL PRIMARY KEY,
    transform_id        INTEGER                  NOT NULL,
    file_key            TEXT                     NOT NULL,
    processed_at        TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    item_count          INTEGER                  NOT NULL DEFAULT 0,
    process_status      TEXT                     NOT NULL DEFAULT 'completed',
    process_error       TEXT                     NULL,
    processing_duration_ms BIGINT                NULL,
    FOREIGN KEY (transform_id) REFERENCES TRANSFORMS(transform_id) ON DELETE CASCADE,
    UNIQUE(transform_id, file_key)
);

CREATE INDEX IF NOT EXISTS idx_transform_processed_files_transform_id ON TRANSFORM_PROCESSED_FILES(transform_id);
CREATE INDEX IF NOT EXISTS idx_transform_processed_files_status ON TRANSFORM_PROCESSED_FILES(process_status);

-- Composite index for transform stats queries
-- Used by: get_transform_stats, counting processed files by status
CREATE INDEX IF NOT EXISTS idx_transform_processed_files_transform_status
    ON TRANSFORM_PROCESSED_FILES(transform_id, process_status);

-- Partial index for finding unprocessed/failed files efficiently
-- Used by: scanner to find files that need reprocessing
CREATE INDEX IF NOT EXISTS idx_transform_processed_files_incomplete
    ON TRANSFORM_PROCESSED_FILES(transform_id, file_key)
    WHERE process_status != 'completed';
