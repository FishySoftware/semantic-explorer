use sqlx::{Pool, Postgres};
use std::time::Instant;

use crate::audit::AuditEvent;
use semantic_explorer_core::observability::record_database_query;

const INSERT_AUDIT_EVENT_QUERY: &str = r#"
    INSERT INTO audit_events (
        timestamp,
        event_type,
        outcome,
        user_id,
        username_display,
        resource_type,
        resource_id,
        details
    )
    VALUES (
        $1::timestamp with time zone, $2, $3, $4, $5, $6, $7, $8
    )
    ON CONFLICT DO NOTHING
"#;

/// Stores an audit event in the database using a connection pool (non-transactional).
/// For transactional operations, use `store_audit_event` with a transaction instead.
pub async fn store_audit_event_simple(
    pool: &Pool<Postgres>,
    event: &AuditEvent,
) -> Result<(), sqlx::Error> {
    let start = Instant::now();
    let result = sqlx::query(INSERT_AUDIT_EVENT_QUERY)
        .bind(&event.timestamp)
        .bind(format!("{:?}", event.event_type))
        .bind(format!("{:?}", event.outcome))
        .bind(&event.user)
        .bind(&event.user_display)
        .bind(
            event
                .resource_type
                .as_ref()
                .map(|rt| format!("{:?}", rt))
                .as_deref(),
        )
        .bind(event.resource_id.as_deref())
        .bind(event.details.as_deref())
        .execute(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    record_database_query("INSERT", "audit_events", duration, result.is_ok());

    result?;
    Ok(())
}
