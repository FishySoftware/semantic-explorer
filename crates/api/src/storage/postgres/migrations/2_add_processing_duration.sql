-- Add processing_duration_ms column to track how long each batch took to process
ALTER TABLE transform_processed_files
ADD COLUMN IF NOT EXISTS processing_duration_ms BIGINT NULL;
