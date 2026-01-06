-- Migration: Add source_transform_id for visualization transforms
-- This field is used by dataset_visualization_transform to reference the source
-- dataset_to_vector_storage transform

ALTER TABLE TRANSFORMS
ADD COLUMN IF NOT EXISTS source_transform_id INTEGER NULL
    REFERENCES TRANSFORMS(transform_id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS idx_transforms_source_transform_id ON TRANSFORMS(source_transform_id);

COMMENT ON COLUMN TRANSFORMS.source_transform_id IS 'References the source transform for visualization transforms';
