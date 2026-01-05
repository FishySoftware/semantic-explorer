pub(crate) mod listeners;
pub(crate) mod models;
pub(crate) mod scanner;

use anyhow::Result;
use async_nats::Client as NatsClient;
use tracing::info;

use crate::transforms::models::ScanCollectionJob;

#[tracing::instrument(name = "enqueue_scan_job", skip(client))]
pub(crate) async fn enqueue_scan_job(client: &NatsClient, job: ScanCollectionJob) -> Result<()> {
    info!("Enqueuing scan job for transform {}", job.transform_id);
    let payload = serde_json::to_vec(&job)?;
    client
        .publish("worker.scan".to_string(), payload.into())
        .await?;
    Ok(())
}
