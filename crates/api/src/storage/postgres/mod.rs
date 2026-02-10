pub(crate) mod audit;
pub(crate) mod chat;
pub(crate) mod collection_transforms;
pub(crate) mod collections;
pub(crate) mod dataset_transform_batches;
pub(crate) mod dataset_transform_pending_batches;
pub(crate) mod dataset_transform_stats;
pub(crate) mod dataset_transforms;
pub(crate) mod datasets;
pub(crate) mod embedded_datasets;
pub(crate) mod embedders;
pub(crate) mod llms;
pub(crate) mod rls;
pub(crate) mod visualization_transforms;

use actix_web::rt::{spawn, time::interval};
use anyhow::Result;
use semantic_explorer_core::config::DatabaseConfig;
use semantic_explorer_core::observability::update_database_pool_stats;
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use std::time::Duration;

/// Default batch size for internal batched fetches.
pub(crate) const INTERNAL_BATCH_SIZE: i64 = 1000;

/// Fetch all rows by iterating in batches. Calls `fetch_page(limit, offset)`
/// repeatedly until a page returns fewer rows than the batch size.
///
/// Use this instead of passing `i64::MAX` — it bounds memory per round-trip
/// and makes the actual query plans observable.
pub(crate) async fn fetch_all_batched<T, F, Fut>(batch_size: i64, fetch_page: F) -> Result<Vec<T>>
where
    F: Fn(i64, i64) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<T>>>,
{
    let mut all = Vec::new();
    let mut offset = 0i64;
    loop {
        let page = fetch_page(batch_size, offset).await?;
        let count = page.len() as i64;
        all.extend(page);
        if count < batch_size {
            break;
        }
        offset += count;
    }
    Ok(all)
}

pub(crate) async fn initialize_pool(config: &DatabaseConfig) -> Result<Pool<Postgres>> {
    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(config.acquire_timeout)
        .idle_timeout(config.idle_timeout)
        .max_lifetime(config.max_lifetime)
        .test_before_acquire(false) // Skip pre-acquire health check for lower latency
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                // Set session-level timeouts to prevent runaway queries and idle transactions
                sqlx::query("SET statement_timeout = '30s'")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("SET idle_in_transaction_session_timeout = '60s'")
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&config.url)
        .await?;
    sqlx::migrate!("src/storage/postgres/migrations")
        .run(&pool)
        .await?;

    let pool_clone = pool.clone();
    spawn(async move {
        let mut interval = interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            let size = pool_clone.size() as u64;
            let num_idle = pool_clone.num_idle() as u64;
            let active = size.saturating_sub(num_idle);
            update_database_pool_stats(size, active, num_idle);
        }
    });

    Ok(pool)
}
