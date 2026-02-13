-- Shared bearer token -> userinfo cache for multi-replica deployments.
-- Tokens are stored as SHA-256 hashes to avoid persisting raw credentials.
CREATE TABLE bearer_token_cache (
    token_hash  TEXT        PRIMARY KEY,
    user_info   JSONB       NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_bearer_token_cache_created ON bearer_token_cache(created_at);

-- Schedule periodic cleanup via pg_cron so application replicas don't all
-- compete to run the same DELETE.  Runs once per minute; the job itself
-- is cheap (index scan on created_at) so it is safe to run frequently.
DO $outer$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'pg_cron') THEN
        PERFORM cron.schedule(
            'cleanup_bearer_token_cache',
            '* * * * *',
            $$DELETE FROM bearer_token_cache WHERE created_at < NOW() - INTERVAL '120 seconds'$$
        );
    ELSE
        RAISE NOTICE 'pg_cron extension not available – bearer_token_cache cleanup job not scheduled. '
                     'Expired rows will still be excluded by the TTL filter in SELECT queries.';
    END IF;
END
$outer$;
