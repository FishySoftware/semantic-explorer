-- Remove file_count column from collections table.
ALTER TABLE collections DROP COLUMN IF EXISTS file_count;
