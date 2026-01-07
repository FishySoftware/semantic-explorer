-- Migration: Add status tracking to visualization transforms
-- This allows tracking the success/failure of visualization processing jobs

ALTER TABLE VISUALIZATION_TRANSFORMS
ADD COLUMN IF NOT EXISTS last_run_status TEXT NULL,
ADD COLUMN IF NOT EXISTS last_run_at TIMESTAMP WITH TIME ZONE NULL,
ADD COLUMN IF NOT EXISTS last_error TEXT NULL,
ADD COLUMN IF NOT EXISTS last_run_stats JSONB NULL;

CREATE INDEX IF NOT EXISTS idx_visualization_transforms_last_run_status 
    ON VISUALIZATION_TRANSFORMS(last_run_status);