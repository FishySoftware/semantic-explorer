-- Audit events table for security and compliance monitoring
CREATE TABLE IF NOT EXISTS audit_events (
    audit_event_id      BIGSERIAL PRIMARY KEY,
    timestamp           TIMESTAMP WITH TIME ZONE NOT NULL,
    event_type          TEXT                     NOT NULL,
    outcome             TEXT                     NOT NULL,
    username            TEXT                     NOT NULL,
    request_id          TEXT                     NULL,
    client_ip           INET                     NULL,
    resource_type       TEXT                     NULL,
    resource_id         TEXT                     NULL,
    details             TEXT                     NULL,
    created_at          TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Indexes for common audit queries
CREATE INDEX IF NOT EXISTS idx_audit_events_timestamp
    ON audit_events(timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_audit_events_username
    ON audit_events(username);

CREATE INDEX IF NOT EXISTS idx_audit_events_event_type
    ON audit_events(event_type);

CREATE INDEX IF NOT EXISTS idx_audit_events_outcome
    ON audit_events(outcome);

CREATE INDEX IF NOT EXISTS idx_audit_events_resource_type
    ON audit_events(resource_type);

CREATE INDEX IF NOT EXISTS idx_audit_events_username_timestamp
    ON audit_events(username, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_audit_events_event_type_timestamp
    ON audit_events(event_type, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_audit_events_created_at
    ON audit_events(created_at DESC);

-- Composite index for common filtering
CREATE INDEX IF NOT EXISTS idx_audit_events_username_event_type_timestamp
    ON audit_events(username, event_type, timestamp DESC);
