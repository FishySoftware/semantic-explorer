-- ============================================================================
-- Add max_input_tokens column to embedders table
-- ============================================================================
--
-- This migration adds support for tracking the maximum input token limit
-- for each embedder model. This allows the system to truncate text before
-- embedding to avoid exceeding model limits.
--
-- Default value: 8191 (OpenAI text-embedding-ada-002 limit)

ALTER TABLE embedders 
ADD COLUMN IF NOT EXISTS max_input_tokens INTEGER NOT NULL DEFAULT 8191;

CREATE INDEX IF NOT EXISTS idx_embedders_max_input_tokens ON embedders(max_input_tokens);

COMMENT ON COLUMN embedders.max_input_tokens IS 'Maximum input tokens accepted by this embedder model';
