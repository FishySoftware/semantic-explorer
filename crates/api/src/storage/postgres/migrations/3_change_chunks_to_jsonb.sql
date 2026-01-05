-- Change chunks column from TEXT[] to JSONB
-- Chunks are stored as JSON objects with content and metadata fields
ALTER TABLE dataset_items
ALTER COLUMN chunks TYPE JSONB USING chunks::text::jsonb;
