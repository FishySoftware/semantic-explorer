pub(crate) mod chat;
pub(crate) mod collections;
pub(crate) mod datasets;
pub(crate) mod embedders;
pub(crate) mod llms;

pub(crate) mod collection_transforms;
pub(crate) mod dataset_transforms;
pub(crate) mod embedded_datasets;
pub(crate) mod visualization_transforms;

use actix_web::rt::{spawn, time::interval};
use anyhow::Result;
use semantic_explorer_core::config::DatabaseConfig;
use semantic_explorer_core::observability::update_database_pool_stats;
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use std::time::Duration;

pub(crate) async fn initialize_pool(config: &DatabaseConfig) -> Result<Pool<Postgres>> {
    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(config.acquire_timeout)
        .idle_timeout(config.idle_timeout)
        .max_lifetime(config.max_lifetime)
        .test_before_acquire(true) // Verify connection health
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
