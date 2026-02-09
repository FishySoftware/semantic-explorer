-- ============================================================================
-- Fix Watermark Advancement Bug
-- ============================================================================
-- This migration adds:
-- 1. Composite index on dataset_items for the watermark query 
--    (COALESCE(updated_at, created_at), item_id) to support the >= filter
--    with deterministic tiebreaker ordering
-- 2. Scan lock column on embedded_datasets to prevent concurrent scans
--    from racing on the same watermark

-- Add composite index for the watermark query used by the scanner.
-- The query filters by dataset_id and COALESCE(updated_at, created_at) >= $2,
-- ordered by COALESCE(updated_at, created_at) ASC, item_id ASC.
-- This index supports both the filter and the sort order efficiently.
CREATE INDEX IF NOT EXISTS idx_dataset_items_watermark
    ON dataset_items (dataset_id, COALESCE(updated_at, created_at) ASC, item_id ASC);

-- Add scan_locked_at to embedded_datasets for distributed scan locking.
-- When a scanner begins processing, it sets scan_locked_at = NOW().
-- Other scanners skip rows where scan_locked_at is recent (within timeout).
-- This prevents concurrent scans from racing on the same watermark.
ALTER TABLE embedded_datasets
    ADD COLUMN IF NOT EXISTS scan_locked_at TIMESTAMP WITH TIME ZONE NULL;

-- Add index for efficient lock-check queries
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_scan_lock
    ON embedded_datasets (embedded_dataset_id, scan_locked_at)
    WHERE scan_locked_at IS NOT NULL;
