-- ============================================================================
-- Add timestamps for improved chunk deduplication tracking
-- ============================================================================

-- Add updated_at to dataset_items for tracking when items are modified
ALTER TABLE dataset_items
ADD COLUMN updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW();

-- Create index for efficient timestamp queries
CREATE INDEX IF NOT EXISTS idx_dataset_items_updated_at
    ON dataset_items(dataset_id, updated_at DESC);

-- Add last_processed_at to embedded_datasets to track the most recent processing
ALTER TABLE embedded_datasets
ADD COLUMN last_processed_at TIMESTAMP WITH TIME ZONE NULL;

-- Create index for filtering by last_processed_at
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_last_processed_at
    ON embedded_datasets(embedded_dataset_id, last_processed_at);
