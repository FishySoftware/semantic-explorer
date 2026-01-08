-- ============================================================================
-- Add truncate_strategy column to embedders table
-- ============================================================================
--
-- This migration adds support for controlling text truncation behavior when
-- creating embeddings. Users can now explicitly set a truncation strategy
-- when creating an embedder.
--
-- Supported strategies:
-- - NONE: Do not truncate (default, return error if text exceeds max_input_tokens)
-- - START: Truncate from the beginning, keeping the end
-- - END: Truncate from the end, keeping the beginning
-- - Custom: Application-defined truncation strategy
--
-- Default value: 'NONE'

ALTER TABLE embedders 
ADD COLUMN IF NOT EXISTS truncate_strategy VARCHAR(50) NOT NULL DEFAULT 'NONE';

COMMENT ON COLUMN embedders.truncate_strategy IS 'Text truncation strategy: NONE, START, END, or custom value';

-- Update any existing records to use the default strategy if not set
UPDATE embedders SET truncate_strategy = 'NONE' WHERE truncate_strategy IS NULL;
