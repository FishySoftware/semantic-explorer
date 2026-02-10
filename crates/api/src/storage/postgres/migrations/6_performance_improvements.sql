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

ALTER TABLE audit_events SET (
    autovacuum_vacuum_scale_factor = 0.1,
    autovacuum_analyze_scale_factor = 0.05,
    autovacuum_vacuum_threshold = 500
);

ALTER TABLE transform_processed_files SET (
    autovacuum_vacuum_scale_factor = 0.1,
    autovacuum_analyze_scale_factor = 0.05
);

ALTER TABLE dataset_transform_batches SET (
    autovacuum_vacuum_scale_factor = 0.1,
    autovacuum_analyze_scale_factor = 0.05
);

ALTER TABLE dataset_transform_stats SET (
    autovacuum_vacuum_scale_factor = 0.05,
    autovacuum_analyze_scale_factor = 0.02
);
