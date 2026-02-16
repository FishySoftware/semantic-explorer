-- Fix: Allow multiple standalone embedded datasets
-- The original UNIQUE(dataset_transform_id, embedder_id) constraint prevents creating
-- more than one standalone dataset because they all use sentinel value 0 for both columns.
-- Replace with a partial unique index that only applies to non-standalone datasets.

ALTER TABLE embedded_datasets DROP CONSTRAINT IF EXISTS embedded_datasets_dataset_transform_id_embedder_id_key;

CREATE UNIQUE INDEX IF NOT EXISTS idx_embedded_datasets_transform_embedder_unique
    ON embedded_datasets (dataset_transform_id, embedder_id)
    WHERE dataset_transform_id != 0;
