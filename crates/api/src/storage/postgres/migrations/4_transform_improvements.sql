-- ============================================================================
-- Transform System Improvements
-- ============================================================================
-- This migration adds:
-- 1. collection_transform_id to pending_batches for collection transform recovery
-- 2. source_dataset_version tracking on embedded_datasets for efficient stats refresh
-- 3. Index improvements for orphaned batch queries

-- Add collection_transform_id to pending_batches for collection transform recovery
ALTER TABLE pending_batches 
    ADD COLUMN IF NOT EXISTS collection_transform_id INTEGER NULL 
    REFERENCES collection_transforms(collection_transform_id) ON DELETE CASCADE;

-- Update unique index to consider collection_transform_id
DROP INDEX IF EXISTS idx_pending_batches_unique_batch;
CREATE UNIQUE INDEX IF NOT EXISTS idx_pending_batches_unique_batch 
    ON pending_batches(batch_type, COALESCE(dataset_transform_id, 0), COALESCE(collection_transform_id, 0), batch_key) 
    WHERE status = 'pending';

-- Add index for collection transform pending batches
CREATE INDEX IF NOT EXISTS idx_pending_batches_collection_transform 
    ON pending_batches(collection_transform_id) 
    WHERE collection_transform_id IS NOT NULL;

-- Add source_dataset_version to embedded_datasets for efficient stats refresh (#4)
-- This tracks the dataset's updated_at so we only refresh stats when the source dataset changes
ALTER TABLE embedded_datasets 
    ADD COLUMN IF NOT EXISTS source_dataset_version TIMESTAMP WITH TIME ZONE NULL;

-- Add index for orphaned batch cleanup queries (#7)
CREATE INDEX IF NOT EXISTS idx_pending_batches_created_status 
    ON pending_batches(created_at, status) 
    WHERE status = 'pending';

-- Add index on dataset_transform_batches for failed batch queries (#6)
CREATE INDEX IF NOT EXISTS idx_dataset_transform_batches_status 
    ON dataset_transform_batches(dataset_transform_id, status) 
    WHERE status = 'failed';
