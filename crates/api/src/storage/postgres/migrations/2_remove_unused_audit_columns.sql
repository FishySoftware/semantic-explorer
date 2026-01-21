-- Remove unused audit event columns
-- These columns were never populated in practice

ALTER TABLE audit_events DROP COLUMN IF EXISTS request_id;
ALTER TABLE audit_events DROP COLUMN IF EXISTS client_ip;
