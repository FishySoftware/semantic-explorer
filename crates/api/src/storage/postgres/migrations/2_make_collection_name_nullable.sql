-- Make collection_name nullable in embedders table
-- Collection name is optional for embedders
ALTER TABLE embedders
ALTER COLUMN collection_name DROP NOT NULL;
