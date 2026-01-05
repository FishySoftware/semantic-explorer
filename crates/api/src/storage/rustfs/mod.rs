use anyhow::Result;
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::{Client, config::Credentials};
use semantic_explorer_core::observability::record_storage_operation;
use serde::Serialize;
use std::{env, time::Instant};
use utoipa::ToSchema;

#[derive(Debug, Clone)]
pub(crate) struct DocumentUpload {
    pub(crate) collection_id: String,
    pub(crate) name: String,
    pub(crate) content: Vec<u8>,
    pub(crate) mime_type: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct CollectionFile {
    pub(crate) key: String,
    pub(crate) size: i64,
    pub(crate) last_modified: Option<String>,
    pub(crate) content_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub(crate) struct PaginatedFiles {
    pub(crate) files: Vec<CollectionFile>,
    pub(crate) page: i32,
    pub(crate) page_size: i32,
    pub(crate) has_more: bool,
    pub(crate) continuation_token: Option<String>,
    pub(crate) total_count: Option<i64>,
}

pub(crate) async fn initialize_client() -> Result<aws_sdk_s3::Client> {
    let shard_config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(env::var("AWS_REGION")?))
        .credentials_provider(Credentials::new(
            env::var("AWS_ACCESS_KEY_ID")?,
            env::var("AWS_SECRET_ACCESS_KEY")?,
            None,
            None,
            "rustfs",
        ))
        .endpoint_url(env::var("AWS_ENDPOINT_URL")?)
        .load()
        .await;
    Ok(Client::new(&shard_config))
}

#[tracing::instrument(name = "s3.upload_document", skip(client, document), fields(storage.system = "s3", bucket = %document.collection_id, key = %document.name, size = document.content.len()))]
pub(crate) async fn upload_document(client: &Client, document: DocumentUpload) -> Result<()> {
    let start = Instant::now();
    let file_size = document.content.len() as u64;

    let result = client
        .put_object()
        .bucket(&document.collection_id)
        .key(&document.name)
        .body(document.content.into())
        .content_type(&document.mime_type)
        .send()
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_storage_operation("upload", duration, Some(file_size), success);

    result?;
    Ok(())
}

#[tracing::instrument(name = "s3.list_files", skip(s3_client), fields(storage.system = "s3", bucket = %bucket, page_size = %page_size))]
pub(crate) async fn list_files(
    s3_client: &Client,
    bucket: &str,
    page_size: i32,
    continuation_token: Option<&str>,
) -> Result<PaginatedFiles> {
    let start = Instant::now();

    let mut files = Vec::with_capacity(page_size as usize);
    let mut current_token = continuation_token.map(|s| s.to_string());

    loop {
        let mut request = s3_client
            .list_objects_v2()
            .bucket(bucket)
            .max_keys(page_size * 3);

        if let Some(token) = &current_token {
            request = request.start_after(token);
        }

        let output = request.send().await?;

        for obj in output.contents() {
            let key = obj.key().unwrap_or_default();

            if key.starts_with("chunks/") {
                continue;
            }

            files.push(CollectionFile {
                key: key.to_string(),
                size: obj.size().unwrap_or(0),
                last_modified: obj.last_modified().map(|dt| dt.to_string()),
                content_type: mime_guess::from_path(key)
                    .first_raw()
                    .map(|s| s.to_string()),
            });

            if files.len() > page_size as usize {
                break;
            }
        }

        current_token = output.next_continuation_token().map(|s| s.to_string());

        if files.len() > page_size as usize || current_token.is_none() {
            break;
        }
    }

    let has_more = files.len() > page_size as usize;

    if has_more {
        files.truncate(page_size as usize);
    }

    let next_token = if has_more {
        files.last().map(|f| f.key.clone())
    } else {
        None
    };

    let duration = start.elapsed().as_secs_f64();
    record_storage_operation("list", duration, None, true);

    Ok(PaginatedFiles {
        files,
        page: 0,
        page_size,
        has_more,
        continuation_token: next_token,
        total_count: None,
    })
}

#[tracing::instrument(name = "s3.get_file", skip(client), fields(storage.system = "s3", bucket = %bucket, key = %key))]
pub(crate) async fn get_file(client: &Client, bucket: &str, key: &str) -> Result<Vec<u8>> {
    let start = Instant::now();

    let result = client.get_object().bucket(bucket).key(key).send().await;

    match result {
        Ok(response) => {
            let data = response.body.collect().await?;
            let bytes = data.into_bytes().to_vec();
            let file_size = bytes.len() as u64;

            let duration = start.elapsed().as_secs_f64();
            record_storage_operation("download", duration, Some(file_size), true);

            Ok(bytes)
        }
        Err(e) => {
            let duration = start.elapsed().as_secs_f64();
            record_storage_operation("download", duration, None, false);
            Err(e.into())
        }
    }
}

#[tracing::instrument(name = "s3.delete_file", skip(client), fields(storage.system = "s3", bucket = %bucket, key = %key))]
pub(crate) async fn delete_file(client: &Client, bucket: &str, key: &str) -> Result<()> {
    let start = Instant::now();

    let result = client.delete_object().bucket(bucket).key(key).send().await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_storage_operation("delete", duration, None, success);

    result?;
    Ok(())
}

#[tracing::instrument(name = "s3.count_files", skip(client), fields(storage.system = "s3", bucket = %bucket))]
pub(crate) async fn count_files(client: &Client, bucket: &str) -> Result<i64> {
    let start = Instant::now();
    let mut count = 0i64;

    let mut paginator = client
        .list_objects_v2()
        .bucket(bucket)
        .into_paginator()
        .send();

    while let Some(result) = paginator.next().await {
        let output = result?;
        for obj in output.contents() {
            let key = obj.key().unwrap_or_default();
            // Skip chunks directory as those are derived data, not source files
            if !key.starts_with("chunks/") {
                count += 1;
            }
        }
    }

    let duration = start.elapsed().as_secs_f64();
    record_storage_operation("count", duration, None, true);

    Ok(count)
}
