use sqlx::{Pool, Postgres};

const INSERT_AUDIT_EVENT_QUERY: &str = r#"
    INSERT INTO audit_events (
        timestamp,
        event_type,
        outcome,
        username,
        request_id,
        client_ip,
        resource_type,
        resource_id,
        details
    )
    VALUES (
        $1::timestamp with time zone, $2, $3, $4, $5, $6::inet, $7, $8, $9
    )
    ON CONFLICT DO NOTHING
"#;

/// Stores an audit event in the database.
#[allow(clippy::too_many_arguments)]
pub async fn store_audit_event(
    pool: &Pool<Postgres>,
    timestamp: &str,
    event_type: &str,
    outcome: &str,
    username: &str,
    request_id: Option<&str>,
    client_ip: Option<&str>,
    resource_type: Option<&str>,
    resource_id: Option<&str>,
    details: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(INSERT_AUDIT_EVENT_QUERY)
        .bind(timestamp)
        .bind(event_type)
        .bind(outcome)
        .bind(username)
        .bind(request_id)
        .bind(client_ip)
        .bind(resource_type)
        .bind(resource_id)
        .bind(details)
        .execute(pool)
        .await?;

    Ok(())
}
