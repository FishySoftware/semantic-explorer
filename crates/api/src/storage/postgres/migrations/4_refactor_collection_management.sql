-- Migration: Refactor collection management from Embedders to Transforms
-- 
-- This migration decouples Qdrant collections from Embedders, making them stateless.
-- Collections are now managed by Transforms, allowing embedders to be reused across
-- multiple datasets/transforms.

-- Step 1: Add collection_mappings to TRANSFORMS table
-- This stores a mapping of embedder_id -> collection_name for each transform
ALTER TABLE TRANSFORMS 
ADD COLUMN collection_mappings JSONB DEFAULT '{}';

COMMENT ON COLUMN TRANSFORMS.collection_mappings IS 'Maps embedder_id to collection_name for DatasetToVectorStorage transforms';

-- Step 2: Migrate existing data
-- For each transform with embedder_ids, create collection mappings using the embedder's current collection_name
DO $$
DECLARE
    transform_record RECORD;
    embedder_record RECORD;
    mappings JSONB;
    embedder_id INTEGER;
BEGIN
    -- Iterate over all transforms that have embedder_ids
    FOR transform_record IN 
        SELECT transform_id, embedder_ids, owner
        FROM TRANSFORMS
        WHERE embedder_ids IS NOT NULL AND array_length(embedder_ids, 1) > 0
    LOOP
        mappings := '{}'::JSONB;
        
        -- For each embedder in this transform, get its collection_name
        FOREACH embedder_id IN ARRAY transform_record.embedder_ids
        LOOP
            SELECT collection_name INTO embedder_record
            FROM EMBEDDERS
            WHERE embedder_id = embedder_id AND owner = transform_record.owner;
            
            IF FOUND THEN
                -- Add mapping: {"embedder_id": "collection_name"}
                mappings := jsonb_set(
                    mappings,
                    ARRAY[embedder_id::TEXT],
                    to_jsonb(embedder_record.collection_name)
                );
            END IF;
        END LOOP;
        
        -- Update transform with the mappings
        UPDATE TRANSFORMS
        SET collection_mappings = mappings
        WHERE transform_id = transform_record.transform_id;
    END LOOP;
END $$;

-- Step 3: Make embedders.collection_name nullable
-- This allows embedders to be pure configuration without state
ALTER TABLE EMBEDDERS 
ALTER COLUMN collection_name DROP NOT NULL;

COMMENT ON COLUMN EMBEDDERS.collection_name IS 'Deprecated: Collections are now managed by Transforms. This column is kept for backward compatibility but should not be used for new embedders.';

-- Step 4: Create index on collection_mappings for efficient lookups
CREATE INDEX idx_transforms_collection_mappings ON TRANSFORMS USING GIN (collection_mappings);

-- Step 5: Update existing indexes (no changes needed, but document them)
-- The existing indexes on TRANSFORMS remain:
-- - idx_transforms_owner
-- - idx_transforms_enabled
-- - idx_transforms_job_type
-- - idx_transforms_owner_enabled

-- Migration complete
-- Next steps:
-- 1. Update application code to use collection_mappings from Transforms
-- 2. Update Transform scanner to generate collection names
-- 3. Update search/delete APIs to resolve collections via Transforms
-- 4. New embedders should be created with collection_name = NULL
