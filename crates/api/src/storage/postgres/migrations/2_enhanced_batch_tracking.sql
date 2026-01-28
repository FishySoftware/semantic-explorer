-- ============================================================================
-- Enhanced Batch Tracking for Resilient Transform Processing
-- ============================================================================
-- This migration adds:
-- 1. pending_batches table: Tracks batches that failed to publish to NATS
-- 2. Enhanced dataset_transform_stats: Adds total_batches_dispatched for accurate tracking
-- 3. reconciliation_runs table: Tracks reconciliation job history

-- Pending Batches: Stores batches that failed to publish to NATS for retry
-- This enables recovery when NATS is temporarily unavailable
CREATE TABLE IF NOT EXISTS pending_batches (
    id                      SERIAL PRIMARY KEY,
    batch_type              TEXT                     NOT NULL CHECK (batch_type IN ('dataset', 'collection', 'visualization')),
    dataset_transform_id    INTEGER                  NULL REFERENCES dataset_transforms(dataset_transform_id) ON DELETE CASCADE,
    embedded_dataset_id     INTEGER                  NULL REFERENCES embedded_datasets(embedded_dataset_id) ON DELETE CASCADE,
    batch_key               TEXT                     NOT NULL,
    s3_bucket               TEXT                     NOT NULL,
    job_payload             JSONB                    NOT NULL,
    retry_count             INTEGER                  NOT NULL DEFAULT 0,
    max_retries             INTEGER                  NOT NULL DEFAULT 10,
    last_error              TEXT                     NULL,
    next_retry_at           TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status                  TEXT                     NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'published', 'failed', 'expired')),
    created_at              TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at              TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for efficient pending batch queries
CREATE INDEX IF NOT EXISTS idx_pending_batches_status_retry 
    ON pending_batches(status, next_retry_at) 
    WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS idx_pending_batches_transform 
    ON pending_batches(dataset_transform_id) 
    WHERE dataset_transform_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_pending_batches_unique_batch 
    ON pending_batches(batch_type, COALESCE(dataset_transform_id, 0), batch_key) 
    WHERE status = 'pending';

-- Add total_batches_dispatched to dataset_transform_stats for accurate completion tracking
-- This allows us to know when all dispatched batches have been processed
ALTER TABLE dataset_transform_stats 
    ADD COLUMN IF NOT EXISTS total_batches_dispatched BIGINT NOT NULL DEFAULT 0;

-- Add total_chunks_dispatched for tracking chunks sent to workers
ALTER TABLE dataset_transform_stats 
    ADD COLUMN IF NOT EXISTS total_chunks_dispatched BIGINT NOT NULL DEFAULT 0;

-- Add run tracking fields for living pipeline support
-- run_id allows us to track which "run" each batch belongs to
ALTER TABLE dataset_transform_stats 
    ADD COLUMN IF NOT EXISTS current_run_id TEXT NULL;
ALTER TABLE dataset_transform_stats 
    ADD COLUMN IF NOT EXISTS current_run_started_at TIMESTAMP WITH TIME ZONE NULL;

-- Reconciliation Runs: Tracks history of reconciliation job executions
CREATE TABLE IF NOT EXISTS reconciliation_runs (
    id                      SERIAL PRIMARY KEY,
    run_type                TEXT                     NOT NULL CHECK (run_type IN ('dataset', 'collection', 'all')),
    dataset_transform_id    INTEGER                  NULL REFERENCES dataset_transforms(dataset_transform_id) ON DELETE SET NULL,
    started_at              TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at            TIMESTAMP WITH TIME ZONE NULL,
    status                  TEXT                     NOT NULL DEFAULT 'running' CHECK (status IN ('running', 'completed', 'failed')),
    orphaned_batches_found  INTEGER                  NOT NULL DEFAULT 0,
    batches_recovered       INTEGER                  NOT NULL DEFAULT 0,
    batches_cleaned_up      INTEGER                  NOT NULL DEFAULT 0,
    error_message           TEXT                     NULL,
    details                 JSONB                    NULL
);

CREATE INDEX IF NOT EXISTS idx_reconciliation_runs_status 
    ON reconciliation_runs(status, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_reconciliation_runs_transform 
    ON reconciliation_runs(dataset_transform_id) 
    WHERE dataset_transform_id IS NOT NULL;

-- Add batch_key unique constraint to dataset_transform_batches if not exists
-- This prevents duplicate batch records
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint 
        WHERE conname = 'dataset_transform_batches_unique_batch'
    ) THEN
        ALTER TABLE dataset_transform_batches 
            ADD CONSTRAINT dataset_transform_batches_unique_batch 
            UNIQUE (dataset_transform_id, batch_key);
    END IF;
END $$;

-- Function to safely increment dispatched batch count
CREATE OR REPLACE FUNCTION increment_dispatched_batch(
    p_dataset_transform_id INTEGER,
    p_chunk_count BIGINT
) RETURNS VOID AS $$
BEGIN
    UPDATE dataset_transform_stats
    SET 
        total_batches_dispatched = total_batches_dispatched + 1,
        total_chunks_dispatched = total_chunks_dispatched + p_chunk_count,
        updated_at = NOW()
    WHERE dataset_transform_id = p_dataset_transform_id;
    
    -- If no row was updated, the stats record doesn't exist yet
    IF NOT FOUND THEN
        INSERT INTO dataset_transform_stats (
            dataset_transform_id, 
            total_batches_dispatched, 
            total_chunks_dispatched,
            created_at, 
            updated_at
        )
        VALUES (
            p_dataset_transform_id, 
            1, 
            p_chunk_count,
            NOW(), 
            NOW()
        )
        ON CONFLICT (dataset_transform_id) DO UPDATE
        SET 
            total_batches_dispatched = dataset_transform_stats.total_batches_dispatched + 1,
            total_chunks_dispatched = dataset_transform_stats.total_chunks_dispatched + p_chunk_count,
            updated_at = NOW();
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Function to reset stats for a new run
CREATE OR REPLACE FUNCTION reset_transform_stats_for_run(
    p_dataset_transform_id INTEGER,
    p_run_id TEXT
) RETURNS VOID AS $$
BEGIN
    UPDATE dataset_transform_stats
    SET 
        total_batches_dispatched = 0,
        total_chunks_dispatched = 0,
        total_chunks_embedded = 0,
        total_chunks_processing = 0,
        total_chunks_failed = 0,
        successful_batches = 0,
        failed_batches = 0,
        processing_batches = 0,
        current_run_id = p_run_id,
        current_run_started_at = NOW(),
        first_processing_at = NULL,
        last_processed_at = NULL,
        updated_at = NOW()
    WHERE dataset_transform_id = p_dataset_transform_id;
END;
$$ LANGUAGE plpgsql;
