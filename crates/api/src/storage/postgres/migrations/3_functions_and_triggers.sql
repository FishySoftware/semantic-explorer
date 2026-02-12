-- Incremental dataset stats maintenance (avoids COUNT/SUM scans)
CREATE OR REPLACE FUNCTION update_dataset_stats() RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE datasets SET
            item_count = item_count + 1,
            total_chunks = total_chunks + jsonb_array_length(NEW.chunks),
            updated_at = NOW()
        WHERE dataset_id = NEW.dataset_id;
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        UPDATE datasets SET
            item_count = GREATEST(item_count - 1, 0),
            total_chunks = GREATEST(total_chunks - jsonb_array_length(OLD.chunks), 0),
            updated_at = NOW()
        WHERE dataset_id = OLD.dataset_id;
        RETURN OLD;
    ELSIF TG_OP = 'UPDATE' THEN
        IF OLD.chunks IS DISTINCT FROM NEW.chunks THEN
            UPDATE datasets SET
                total_chunks = total_chunks - jsonb_array_length(OLD.chunks) + jsonb_array_length(NEW.chunks),
                updated_at = NOW()
            WHERE dataset_id = NEW.dataset_id;
        END IF;
        RETURN NEW;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_update_dataset_stats
AFTER INSERT OR DELETE OR UPDATE OF chunks ON dataset_items
FOR EACH ROW
EXECUTE FUNCTION update_dataset_stats();

-- Validates FK references for embedded_datasets (sentinel 0 = standalone)
CREATE OR REPLACE FUNCTION validate_embedded_dataset_fks() RETURNS TRIGGER AS $$
BEGIN
    IF NEW.dataset_transform_id = 0 AND NEW.source_dataset_id = 0 AND NEW.embedder_id = 0 THEN
        IF NEW.dimensions IS NULL OR NEW.dimensions <= 0 THEN
            RAISE EXCEPTION 'Standalone embedded datasets must have dimensions > 0';
        END IF;
        RETURN NEW;
    END IF;

    IF NEW.dataset_transform_id = 0 OR NEW.source_dataset_id = 0 OR NEW.embedder_id = 0 THEN
        RAISE EXCEPTION 'Non-standalone embedded datasets must have all FK fields set (dataset_transform_id, source_dataset_id, embedder_id)';
    END IF;

    IF NOT EXISTS (SELECT 1 FROM dataset_transforms WHERE dataset_transform_id = NEW.dataset_transform_id) THEN
        RAISE EXCEPTION 'dataset_transform_id % does not exist', NEW.dataset_transform_id;
    END IF;

    IF NOT EXISTS (SELECT 1 FROM datasets WHERE dataset_id = NEW.source_dataset_id) THEN
        RAISE EXCEPTION 'source_dataset_id % does not exist', NEW.source_dataset_id;
    END IF;

    IF NOT EXISTS (SELECT 1 FROM embedders WHERE embedder_id = NEW.embedder_id) THEN
        RAISE EXCEPTION 'embedder_id % does not exist', NEW.embedder_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_validate_embedded_dataset_fks
BEFORE INSERT OR UPDATE ON embedded_datasets
FOR EACH ROW
EXECUTE FUNCTION validate_embedded_dataset_fks();

-- Atomically increment dispatched batch count (upsert)
CREATE OR REPLACE FUNCTION increment_dispatched_batch(
    p_dataset_transform_id INTEGER,
    p_chunk_count BIGINT
) RETURNS VOID AS $$
BEGIN
    UPDATE dataset_transform_stats
    SET
        total_batches_dispatched = total_batches_dispatched + 1,
        total_chunks_dispatched = total_chunks_dispatched + p_chunk_count,
        updated_at = NOW()
    WHERE dataset_transform_id = p_dataset_transform_id;

    IF NOT FOUND THEN
        INSERT INTO dataset_transform_stats (
            dataset_transform_id,
            total_batches_dispatched,
            total_chunks_dispatched,
            created_at,
            updated_at
        )
        VALUES (
            p_dataset_transform_id,
            1,
            p_chunk_count,
            NOW(),
            NOW()
        )
        ON CONFLICT (dataset_transform_id) DO UPDATE
        SET
            total_batches_dispatched = dataset_transform_stats.total_batches_dispatched + 1,
            total_chunks_dispatched = dataset_transform_stats.total_chunks_dispatched + p_chunk_count,
            updated_at = NOW();
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Reset stats for a new transform run
CREATE OR REPLACE FUNCTION reset_transform_stats_for_run(
    p_dataset_transform_id INTEGER,
    p_run_id TEXT
) RETURNS VOID AS $$
BEGIN
    UPDATE dataset_transform_stats
    SET
        total_batches_dispatched = 0,
        total_chunks_dispatched = 0,
        total_chunks_embedded = 0,
        total_chunks_processing = 0,
        total_chunks_failed = 0,
        successful_batches = 0,
        failed_batches = 0,
        processing_batches = 0,
        current_run_id = p_run_id,
        current_run_started_at = NOW(),
        first_processing_at = NULL,
        last_processed_at = NULL,
        updated_at = NOW()
    WHERE dataset_transform_id = p_dataset_transform_id;
END;
$$ LANGUAGE plpgsql;
