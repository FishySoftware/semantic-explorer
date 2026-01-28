-- ============================================================================
-- Standalone Embedded Datasets Support
-- ============================================================================
-- This migration adds support for standalone embedded datasets that can be
-- created without a dataset transform, embedder, or source dataset.
-- These datasets allow users to push vectors directly and use them in
-- visualizations (but not in search/chat since they lack an embedder).
-- ============================================================================

-- Add dimensions column for standalone embedded datasets
-- This stores the vector dimensionality which is required when creating the Qdrant collection
-- For transform-based datasets, this is derived from the embedder; for standalone, it's user-specified
ALTER TABLE embedded_datasets ADD COLUMN IF NOT EXISTS dimensions INTEGER NULL;

-- For standalone datasets, we use sentinel value 0 for dataset_transform_id, source_dataset_id, and embedder_id
-- The existing foreign key constraints will prevent this, so we need to:
-- 1. Drop the existing foreign key constraints
-- 2. Recreate them to allow 0 as a special value (check constraint instead of FK)

-- First, we need to drop the existing foreign key constraints
ALTER TABLE embedded_datasets DROP CONSTRAINT IF EXISTS embedded_datasets_dataset_transform_id_fkey;
ALTER TABLE embedded_datasets DROP CONSTRAINT IF EXISTS embedded_datasets_source_dataset_id_fkey;
ALTER TABLE embedded_datasets DROP CONSTRAINT IF EXISTS embedded_datasets_embedder_id_fkey;

-- Recreate foreign key constraints that allow 0 as a sentinel value
-- These use partial constraints: FK only applies when value != 0
-- Note: PostgreSQL doesn't support conditional FKs directly, so we use triggers instead

-- Create a function to validate foreign keys for non-standalone datasets
CREATE OR REPLACE FUNCTION validate_embedded_dataset_fks()
RETURNS TRIGGER AS $$
BEGIN
    -- For standalone datasets (sentinel value 0), skip FK validation
    IF NEW.dataset_transform_id = 0 AND NEW.source_dataset_id = 0 AND NEW.embedder_id = 0 THEN
        -- Standalone dataset: dimensions must be set
        IF NEW.dimensions IS NULL OR NEW.dimensions <= 0 THEN
            RAISE EXCEPTION 'Standalone embedded datasets must have dimensions > 0';
        END IF;
        RETURN NEW;
    END IF;
    
    -- For regular datasets, all three must be non-zero and valid
    IF NEW.dataset_transform_id = 0 OR NEW.source_dataset_id = 0 OR NEW.embedder_id = 0 THEN
        RAISE EXCEPTION 'Non-standalone embedded datasets must have all FK fields set (dataset_transform_id, source_dataset_id, embedder_id)';
    END IF;
    
    -- Validate dataset_transform_id exists
    IF NOT EXISTS (SELECT 1 FROM dataset_transforms WHERE dataset_transform_id = NEW.dataset_transform_id) THEN
        RAISE EXCEPTION 'dataset_transform_id % does not exist', NEW.dataset_transform_id;
    END IF;
    
    -- Validate source_dataset_id exists
    IF NOT EXISTS (SELECT 1 FROM datasets WHERE dataset_id = NEW.source_dataset_id) THEN
        RAISE EXCEPTION 'source_dataset_id % does not exist', NEW.source_dataset_id;
    END IF;
    
    -- Validate embedder_id exists
    IF NOT EXISTS (SELECT 1 FROM embedders WHERE embedder_id = NEW.embedder_id) THEN
        RAISE EXCEPTION 'embedder_id % does not exist', NEW.embedder_id;
    END IF;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create the trigger
DROP TRIGGER IF EXISTS trg_validate_embedded_dataset_fks ON embedded_datasets;
CREATE TRIGGER trg_validate_embedded_dataset_fks
BEFORE INSERT OR UPDATE ON embedded_datasets
FOR EACH ROW
EXECUTE FUNCTION validate_embedded_dataset_fks();

-- Add index for filtering standalone vs transform-based datasets
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_standalone 
    ON embedded_datasets(dataset_transform_id) 
    WHERE dataset_transform_id = 0;

-- Add index for transform-based datasets (non-standalone)
CREATE INDEX IF NOT EXISTS idx_embedded_datasets_non_standalone 
    ON embedded_datasets(dataset_transform_id) 
    WHERE dataset_transform_id != 0;

-- Comment explaining the sentinel value convention
COMMENT ON COLUMN embedded_datasets.dataset_transform_id IS 'Foreign key to dataset_transforms. Value 0 indicates a standalone embedded dataset.';
COMMENT ON COLUMN embedded_datasets.source_dataset_id IS 'Foreign key to datasets. Value 0 indicates a standalone embedded dataset.';
COMMENT ON COLUMN embedded_datasets.embedder_id IS 'Foreign key to embedders. Value 0 indicates a standalone embedded dataset.';
COMMENT ON COLUMN embedded_datasets.dimensions IS 'Vector dimensions. Required for standalone datasets, optional for transform-based (derived from embedder).';
