use anyhow::Result;
use sqlx::{Postgres, Transaction};
use tracing::warn;

/// Sets the current user for Row-Level Security within an existing transaction.
///
/// # Arguments
/// * `tx` - Active database transaction
/// * `username` - The username to set as the current user for RLS policies
///
/// # Example
/// ```rust
/// let mut tx = pool.begin().await?;
/// set_rls_user_tx(&mut tx, "john.doe").await?;
/// // Execute queries with RLS context
/// tx.commit().await?;
/// ```
#[tracing::instrument(name = "db.set_rls_user_tx", skip(tx), fields(username = %username))]
pub async fn set_rls_user_tx(tx: &mut Transaction<'_, Postgres>, username: &str) -> Result<()> {
    // PostgreSQL SET commands don't support parameterized queries, so we escape the username
    // and build the query string directly. Escape single quotes by doubling them.
    let escaped_username = username.replace('\'', "''");
    let query_str = format!("SET LOCAL \"app.current_user\" = '{}'", escaped_username);

    sqlx::query(&query_str)
        .execute(&mut **tx)
        .await
        .map_err(|e| {
            warn!("Failed to set RLS user context in transaction: {}", e);
            anyhow::anyhow!("Failed to set RLS user context in transaction: {}", e)
        })?;
    Ok(())
}
