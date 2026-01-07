pub(crate) mod models;

use anyhow::Result;
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::{Client, config::Credentials};
use semantic_explorer_core::observability::record_storage_operation;

use std::{env, time::Instant};

use crate::storage::rustfs::models::{CollectionFile, DocumentUpload, PaginatedFiles};

pub(crate) async fn initialize_client() -> Result<aws_sdk_s3::Client> {
    let aws_region = env::var("AWS_REGION")?;
    let aws_access_key = env::var("AWS_ACCESS_KEY_ID")?;
    let aws_endpoint_url = env::var("AWS_ENDPOINT_URL")?;
    let shard_config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(aws_region))
        .credentials_provider(Credentials::new(
            aws_access_key,
            env::var("AWS_SECRET_ACCESS_KEY")?,
            None,
            None,
            "rustfs",
        ))
        .endpoint_url(&aws_endpoint_url)
        .load()
        .await;
    let client = Client::new(&shard_config);
    Ok(client)
}

#[tracing::instrument(name = "s3.upload_document", skip(client, document), fields(storage.system = "s3", bucket = %document.collection_id, key = %document.name, size = document.content.len()))]
pub(crate) async fn upload_document(client: &Client, document: DocumentUpload) -> Result<()> {
    let start = Instant::now();
    let file_size = document.content.len() as u64;

    tracing::debug!(
        bucket = %document.collection_id,
        key = %document.name,
        size = file_size,
        mime_type = %document.mime_type,
        "Uploading document to S3"
    );

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

    match &result {
        Ok(_) => {
            tracing::debug!(
                bucket = %document.collection_id,
                key = %document.name,
                duration_ms = duration * 1000.0,
                "Successfully uploaded document to S3"
            );
        }
        Err(e) => {
            tracing::error!(
                bucket = %document.collection_id,
                key = %document.name,
                error = %e,
                duration_ms = duration * 1000.0,
                "Failed to upload document to S3. Check network connectivity and bucket permissions."
            );
        }
    }

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

    tracing::debug!(
        bucket = %bucket,
        page_size = page_size,
        has_continuation_token = continuation_token.is_some(),
        "Listing files in S3 bucket"
    );

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

        let output = match request.send().await {
            Ok(output) => output,
            Err(e) => {
                tracing::error!(
                    bucket = %bucket,
                    error = %e,
                    "Failed to list files in S3 bucket. Check network connectivity and bucket existence."
                );
                return Err(e.into());
            }
        };

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

    tracing::debug!(
        bucket = %bucket,
        file_count = files.len(),
        has_more = has_more,
        duration_ms = duration * 1000.0,
        "Successfully listed files from S3 bucket"
    );

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

    tracing::debug!(
        bucket = %bucket,
        key = %key,
        "Retrieving file from S3"
    );

    let result = client.get_object().bucket(bucket).key(key).send().await;

    match result {
        Ok(response) => {
            let data = response.body.collect().await?;
            let bytes = data.into_bytes().to_vec();
            let file_size = bytes.len() as u64;

            let duration = start.elapsed().as_secs_f64();
            record_storage_operation("download", duration, Some(file_size), true);

            tracing::debug!(
                bucket = %bucket,
                key = %key,
                size = file_size,
                duration_ms = duration * 1000.0,
                "Successfully retrieved file from S3"
            );

            Ok(bytes)
        }
        Err(e) => {
            let duration = start.elapsed().as_secs_f64();
            record_storage_operation("download", duration, None, false);

            tracing::error!(
                bucket = %bucket,
                key = %key,
                error = %e,
                duration_ms = duration * 1000.0,
                "Failed to retrieve file from S3. Check network connectivity, bucket existence, and file permissions."
            );

            Err(e.into())
        }
    }
}

#[tracing::instrument(name = "s3.delete_file", skip(client), fields(storage.system = "s3", bucket = %bucket, key = %key))]
pub(crate) async fn delete_file(client: &Client, bucket: &str, key: &str) -> Result<()> {
    let start = Instant::now();

    tracing::debug!(
        bucket = %bucket,
        key = %key,
        "Deleting file from S3"
    );

    let result = client.delete_object().bucket(bucket).key(key).send().await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_storage_operation("delete", duration, None, success);

    match &result {
        Ok(_) => {
            tracing::info!(
                bucket = %bucket,
                key = %key,
                duration_ms = duration * 1000.0,
                "Successfully deleted file from S3"
            );
        }
        Err(e) => {
            tracing::error!(
                bucket = %bucket,
                key = %key,
                error = %e,
                duration_ms = duration * 1000.0,
                "Failed to delete file from S3. Check network connectivity and file permissions."
            );
        }
    }

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
        let output = match result {
            Ok(output) => output,
            Err(e) => {
                tracing::error!(
                    bucket = %bucket,
                    error = %e,
                    "Failed to count files in S3 bucket. Check network connectivity and bucket existence."
                );
                return Err(e.into());
            }
        };
        for obj in output.contents() {
            let key = obj.key().unwrap_or_default();
            if !key.starts_with("chunks/") {
                count += 1;
            }
        }
    }

    let duration = start.elapsed().as_secs_f64();
    record_storage_operation("count", duration, None, true);

    tracing::debug!(
        bucket = %bucket,
        file_count = count,
        duration_ms = duration * 1000.0,
        "Successfully counted files in S3 bucket"
    );

    Ok(count)
}
