-- Migration: Refactor unified Transforms into separate transform types
-- Date: 2026-01-06
-- Description: Splits TRANSFORMS table into COLLECTION_TRANSFORMS, DATASET_TRANSFORMS,
--              EMBEDDED_DATASETS, and VISUALIZATION_TRANSFORMS for better separation of concerns

-- Drop old unified TRANSFORMS table
DROP TABLE IF EXISTS TRANSFORM_PROCESSED_FILES CASCADE;
DROP TABLE IF EXISTS TRANSFORMS CASCADE;

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
    job_config              JSONB                    NOT NULL DEFAULT '{}',  -- extraction + chunking config
    created_at              TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    FOREIGN KEY (collection_id) REFERENCES COLLECTIONS(collection_id) ON DELETE CASCADE,
    FOREIGN KEY (dataset_id) REFERENCES DATASETS(dataset_id) ON DELETE CASCADE
);

CREATE INDEX idx_collection_transforms_owner ON COLLECTION_TRANSFORMS(owner);
CREATE INDEX idx_collection_transforms_enabled ON COLLECTION_TRANSFORMS(is_enabled);
CREATE INDEX idx_collection_transforms_collection_id ON COLLECTION_TRANSFORMS(collection_id);
CREATE INDEX idx_collection_transforms_dataset_id ON COLLECTION_TRANSFORMS(dataset_id);
CREATE INDEX idx_collection_transforms_owner_enabled ON COLLECTION_TRANSFORMS(owner, is_enabled);

-- ============================================================================
-- DATASET_TRANSFORMS: Dataset → Embedded Datasets (embedding with 1-N embedders)
-- ============================================================================
CREATE TABLE IF NOT EXISTS DATASET_TRANSFORMS (
    dataset_transform_id SERIAL PRIMARY KEY,
    title                TEXT                     NOT NULL,
    source_dataset_id    INTEGER                  NOT NULL,
    embedder_ids         INTEGER[]                NOT NULL,  -- Array of embedder IDs (1-N)
    owner                TEXT                     NOT NULL,
    is_enabled           BOOLEAN                  NOT NULL DEFAULT TRUE,
    job_config           JSONB                    NOT NULL DEFAULT '{}',  -- batch size, wipe settings
    created_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    FOREIGN KEY (source_dataset_id) REFERENCES DATASETS(dataset_id) ON DELETE CASCADE
);

CREATE INDEX idx_dataset_transforms_owner ON DATASET_TRANSFORMS(owner);
CREATE INDEX idx_dataset_transforms_enabled ON DATASET_TRANSFORMS(is_enabled);
CREATE INDEX idx_dataset_transforms_source_dataset_id ON DATASET_TRANSFORMS(source_dataset_id);
CREATE INDEX idx_dataset_transforms_owner_enabled ON DATASET_TRANSFORMS(owner, is_enabled);

-- ============================================================================
-- EMBEDDED_DATASETS: Result entity (one per embedder from Dataset Transform)
-- ============================================================================
CREATE TABLE IF NOT EXISTS EMBEDDED_DATASETS (
    embedded_dataset_id  SERIAL PRIMARY KEY,
    title                TEXT                     NOT NULL,
    dataset_transform_id INTEGER                  NOT NULL,  -- Which transform created it
    source_dataset_id    INTEGER                  NOT NULL,
    embedder_id          INTEGER                  NOT NULL,  -- Single embedder
    owner                TEXT                     NOT NULL,
    collection_name      TEXT                     NOT NULL,  -- Qdrant collection name
    created_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    FOREIGN KEY (dataset_transform_id) REFERENCES DATASET_TRANSFORMS(dataset_transform_id) ON DELETE CASCADE,
    FOREIGN KEY (source_dataset_id) REFERENCES DATASETS(dataset_id) ON DELETE CASCADE,
    FOREIGN KEY (embedder_id) REFERENCES EMBEDDERS(embedder_id) ON DELETE CASCADE,
    UNIQUE(dataset_transform_id, embedder_id)  -- One embedded dataset per transform per embedder
);

CREATE INDEX idx_embedded_datasets_owner ON EMBEDDED_DATASETS(owner);
CREATE INDEX idx_embedded_datasets_dataset_transform_id ON EMBEDDED_DATASETS(dataset_transform_id);
CREATE INDEX idx_embedded_datasets_source_dataset_id ON EMBEDDED_DATASETS(source_dataset_id);
CREATE INDEX idx_embedded_datasets_embedder_id ON EMBEDDED_DATASETS(embedder_id);
CREATE INDEX idx_embedded_datasets_collection_name ON EMBEDDED_DATASETS(collection_name);

-- ============================================================================
-- VISUALIZATION_TRANSFORMS: Embedded Dataset → 3D visualization (UMAP + HDBSCAN)
-- ============================================================================
CREATE TABLE IF NOT EXISTS VISUALIZATION_TRANSFORMS (
    visualization_transform_id SERIAL PRIMARY KEY,
    title                      TEXT                     NOT NULL,
    embedded_dataset_id        INTEGER                  NOT NULL,
    owner                      TEXT                     NOT NULL,
    is_enabled                 BOOLEAN                  NOT NULL DEFAULT TRUE,
    reduced_collection_name    TEXT                     NULL,  -- Qdrant collection for UMAP 3D points
    topics_collection_name     TEXT                     NULL,  -- Qdrant collection for topic centroids
    visualization_config       JSONB                    NOT NULL DEFAULT '{}',  -- UMAP + HDBSCAN params
    created_at                 TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at                 TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    FOREIGN KEY (embedded_dataset_id) REFERENCES EMBEDDED_DATASETS(embedded_dataset_id) ON DELETE CASCADE
);

CREATE INDEX idx_visualization_transforms_owner ON VISUALIZATION_TRANSFORMS(owner);
CREATE INDEX idx_visualization_transforms_enabled ON VISUALIZATION_TRANSFORMS(is_enabled);
CREATE INDEX idx_visualization_transforms_embedded_dataset_id ON VISUALIZATION_TRANSFORMS(embedded_dataset_id);
CREATE INDEX idx_visualization_transforms_owner_enabled ON VISUALIZATION_TRANSFORMS(owner, is_enabled);

-- ============================================================================
-- TRANSFORM_PROCESSED_FILES: Shared tracking table for all transform types
-- ============================================================================
CREATE TABLE IF NOT EXISTS TRANSFORM_PROCESSED_FILES (
    id                      SERIAL PRIMARY KEY,
    transform_type          TEXT                     NOT NULL,  -- 'collection', 'dataset', 'visualization'
    transform_id            INTEGER                  NOT NULL,  -- References appropriate table (polymorphic)
    file_key                TEXT                     NOT NULL,  -- S3 file key or batch key
    processed_at            TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    item_count              INTEGER                  NOT NULL DEFAULT 0,
    process_status          TEXT                     NOT NULL DEFAULT 'completed',  -- 'completed', 'failed', 'pending'
    process_error           TEXT                     NULL,
    processing_duration_ms  BIGINT                   NULL,
    UNIQUE(transform_type, transform_id, file_key)
);

CREATE INDEX idx_transform_processed_files_type_id ON TRANSFORM_PROCESSED_FILES(transform_type, transform_id);
CREATE INDEX idx_transform_processed_files_status ON TRANSFORM_PROCESSED_FILES(process_status);
CREATE INDEX idx_transform_processed_files_processed_at ON TRANSFORM_PROCESSED_FILES(processed_at);

-- ============================================================================
-- Migration Complete
-- ============================================================================
-- Summary:
-- - Dropped old TRANSFORMS and TRANSFORM_PROCESSED_FILES tables
-- - Created 4 transform tables: COLLECTION_TRANSFORMS, DATASET_TRANSFORMS,
--   EMBEDDED_DATASETS, VISUALIZATION_TRANSFORMS
-- - Recreated TRANSFORM_PROCESSED_FILES with polymorphic transform_type field
-- - Added comprehensive indexes for performance
-- - Total tables: 5 new tables with proper foreign key constraints
