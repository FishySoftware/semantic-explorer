-- Migration: Add public resource support
-- This allows users to mark their Collections, Datasets, and Embedders as public
-- Public resources can be discovered and copied by other users via the marketplace

ALTER TABLE COLLECTIONS
ADD COLUMN IF NOT EXISTS is_public BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE DATASETS
ADD COLUMN IF NOT EXISTS is_public BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE EMBEDDERS
ADD COLUMN IF NOT EXISTS is_public BOOLEAN NOT NULL DEFAULT FALSE;

-- Create partial indexes for efficient public resource queries
-- Partial indexes only index rows where is_public = TRUE to save space
CREATE INDEX IF NOT EXISTS idx_collections_public
    ON COLLECTIONS(is_public) WHERE is_public = TRUE;

CREATE INDEX IF NOT EXISTS idx_datasets_public
    ON DATASETS(is_public) WHERE is_public = TRUE;

CREATE INDEX IF NOT EXISTS idx_embedders_public
    ON EMBEDDERS(is_public) WHERE is_public = TRUE;

-- Create composite indexes for marketplace queries (public + owner)
CREATE INDEX IF NOT EXISTS idx_collections_public_owner
    ON COLLECTIONS(is_public, owner);

CREATE INDEX IF NOT EXISTS idx_datasets_public_owner
    ON DATASETS(is_public, owner);

CREATE INDEX IF NOT EXISTS idx_embedders_public_owner
    ON EMBEDDERS(is_public, owner);
