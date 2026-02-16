-- Add last_processed_item_id for composite (timestamp, item_id) watermark.
-- This fixes the bug where the scanner cannot make progress through items
-- that share the same timestamp (common with batch-inserted dataset items).
-- With only a timestamp watermark, the max_batches_per_scan cap causes the
-- scanner to re-fetch the same items on every cycle, permanently losing the
-- tail batches.
ALTER TABLE embedded_datasets ADD COLUMN IF NOT EXISTS last_processed_item_id INTEGER;
