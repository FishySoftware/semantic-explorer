pub(crate) mod models;

use anyhow::{Context, Result, bail};
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::{Client, config::Credentials};
use semantic_explorer_core::observability::record_storage_operation;

use std::{env, time::Instant};
use tracing::warn;

use crate::storage::rustfs::models::{CollectionFile, DocumentUpload, PaginatedFiles};

// Maximum file size for API downloads: 100MB
// NOTE: Limit prevents:
// - Memory exhaustion from large file downloads
// - DoS attacks via resource consumption
// - Timeout issues in API requests
// Production deployments should use direct S3 presigned URLs for large files.
// FUTURE: Make configurable via STORAGE_MAX_DOWNLOAD_SIZE env var if needed
const MAX_DOWNLOAD_SIZE_BYTES: i64 = 100 * 1024 * 1024;

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

    // Use path-style addressing for MinIO/S3-compatible storage when enabled
    // Virtual-host style (default) tries to resolve bucket.endpoint as DNS
    // Path-style uses endpoint/bucket instead
    // Enable with S3_FORCE_PATH_STYLE=true for MinIO, disable for AWS S3
    let force_path_style = env::var("S3_FORCE_PATH_STYLE")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);

    let s3_config = aws_sdk_s3::config::Builder::from(&shard_config)
        .force_path_style(force_path_style)
        .build();
    let client = Client::from_conf(s3_config);
    Ok(client)
}

/// Get S3 bucket and key for collection files
/// Uses single-bucket architecture: S3_BUCKET_NAME/collections/{collection_id}/{filename}
fn get_collection_s3_path(collection_id: &str, key: &str) -> Result<(String, String)> {
    let s3_bucket =
        env::var("S3_BUCKET_NAME").context("S3_BUCKET_NAME is required for S3 operations")?;
    let full_key = format!("collections/{}/{}", collection_id, key);
    Ok((s3_bucket, full_key))
}

/// Get S3 bucket and prefix for listing collection files
fn get_collection_s3_prefix(collection_id: &str) -> Result<(String, String)> {
    let s3_bucket =
        env::var("S3_BUCKET_NAME").context("S3_BUCKET_NAME is required for S3 operations")?;
    let prefix = format!("collections/{}/", collection_id);
    Ok((s3_bucket, prefix))
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

    // Use single-bucket architecture
    let (bucket, key) = get_collection_s3_path(&document.collection_id, &document.name)?;

    let result = client
        .put_object()
        .bucket(&bucket)
        .key(&key)
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
                bucket = %bucket,
                key = %key,
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

    // Convert to new single-bucket format
    let (actual_bucket, prefix) = if bucket.chars().all(|c| c.is_numeric()) {
        // Old format: bucket is collection_id
        let s3_bucket =
            env::var("S3_BUCKET_NAME").context("S3_BUCKET_NAME is required for S3 operations")?;
        (s3_bucket, format!("collections/{}/", bucket))
    } else {
        (bucket.to_string(), String::new())
    };

    let mut files = Vec::new();
    let page_size_usize = page_size as usize;
    let target_count = page_size_usize + 1; // Need page_size + 1 to know if there are more

    // Build the paginator request
    let mut request = s3_client
        .list_objects_v2()
        .bucket(&actual_bucket)
        .prefix(&prefix);

    // If we have a continuation token, use it as start_after
    if let Some(token) = continuation_token {
        request = request.start_after(token);
    }

    let mut paginator = request.into_paginator().send();

    // Iterate through pages until we have enough files or run out
    while let Some(result) = paginator.next().await {
        let output = match result {
            Ok(output) => output,
            Err(e) => {
                tracing::error!(
                    bucket = %actual_bucket,
                    prefix = %prefix,
                    error = %e,
                    "Failed to list files in S3 bucket"
                );
                return Err(e.into());
            }
        };

        for obj in output.contents() {
            let key = obj.key().unwrap_or_default();

            // Skip chunk files
            if key.starts_with(&format!("{}chunks/", prefix)) {
                continue;
            }

            // Extract display key (without prefix)
            let display_key = key.strip_prefix(&prefix).unwrap_or(key).to_string();

            files.push(CollectionFile {
                key: display_key.clone(),
                size: obj.size().unwrap_or(0),
                last_modified: obj.last_modified().map(|dt| dt.to_string()),
                content_type: mime_guess::from_path(&display_key)
                    .first_raw()
                    .map(|s| s.to_string()),
            });

            // Stop once we have enough files to determine if there are more pages
            if files.len() >= target_count {
                break;
            }
        }

        // Stop paginating if we have enough files
        if files.len() >= target_count {
            break;
        }
    }

    // Determine if there are more pages
    let has_more = files.len() > page_size_usize;

    // Truncate to page_size and determine next cursor
    let next_cursor = if has_more {
        files.truncate(page_size_usize);
        // Use the last file's full S3 key as the cursor for start_after pagination
        files.last().map(|f| format!("{}{}", prefix, &f.key))
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
        continuation_token: next_cursor,
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

    // Use single-bucket architecture
    let (actual_bucket, actual_key) = get_collection_s3_path(bucket, key)?;

    let result = client
        .get_object()
        .bucket(&actual_bucket)
        .key(&actual_key)
        .send()
        .await;

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

/// Get file with size validation to prevent OOM on large downloads
/// Returns error if file exceeds MAX_DOWNLOAD_SIZE_BYTES (100MB)
#[tracing::instrument(name = "s3.get_file_with_size_check", skip(client), fields(storage.system = "s3", bucket = %bucket, key = %key))]
pub(crate) async fn get_file_with_size_check(
    client: &Client,
    bucket: &str,
    key: &str,
) -> Result<Vec<u8>> {
    let start = Instant::now();

    tracing::debug!(
        bucket = %bucket,
        key = %key,
        "Checking file size before download"
    );

    // Use single-bucket architecture
    let (actual_bucket, actual_key) = get_collection_s3_path(bucket, key)?;

    // First, check file size using head_object
    let head_result = client
        .head_object()
        .bucket(&actual_bucket)
        .key(&actual_key)
        .send()
        .await
        .context("Failed to get file metadata")?;

    let file_size = head_result.content_length().unwrap_or(0);

    if file_size > MAX_DOWNLOAD_SIZE_BYTES {
        let duration = start.elapsed().as_secs_f64();
        record_storage_operation("download", duration, None, false);
        warn!(
            bucket = %bucket,
            key = %key,
            file_size_mb = file_size / (1024 * 1024),
            max_size_mb = MAX_DOWNLOAD_SIZE_BYTES / (1024 * 1024),
            "File exceeds maximum download size limit"
        );
        bail!(
            "File size ({} MB) exceeds maximum download limit of {} MB",
            file_size / (1024 * 1024),
            MAX_DOWNLOAD_SIZE_BYTES / (1024 * 1024)
        );
    }

    // Size is acceptable, proceed with download
    get_file(client, bucket, key).await
}

#[tracing::instrument(name = "s3.delete_file", skip(client), fields(storage.system = "s3", bucket = %bucket, key = %key))]
pub(crate) async fn delete_file(client: &Client, bucket: &str, key: &str) -> Result<()> {
    let start = Instant::now();

    tracing::debug!(
        bucket = %bucket,
        key = %key,
        "Deleting file from S3"
    );

    // Use single-bucket architecture
    let (actual_bucket, actual_key) = get_collection_s3_path(bucket, key)?;

    let result = client
        .delete_object()
        .bucket(&actual_bucket)
        .key(&actual_key)
        .send()
        .await;

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

    // Convert to new single-bucket format
    let (actual_bucket, prefix) = if bucket.chars().all(|c| c.is_numeric()) {
        // Old format: bucket is collection_id
        let s3_bucket =
            env::var("S3_BUCKET_NAME").context("S3_BUCKET_NAME is required for S3 operations")?;
        (s3_bucket, format!("collections/{}/", bucket))
    } else {
        (bucket.to_string(), String::new())
    };

    let mut paginator = client
        .list_objects_v2()
        .bucket(&actual_bucket)
        .prefix(&prefix)
        .into_paginator()
        .send();

    while let Some(result) = paginator.next().await {
        let output = match result {
            Ok(output) => output,
            Err(e) => {
                tracing::error!(
                    bucket = %actual_bucket,
                    prefix = %prefix,
                    error = %e,
                    "Failed to count files in S3 bucket. Check network connectivity and bucket existence."
                );
                return Err(e.into());
            }
        };
        for obj in output.contents() {
            let key = obj.key().unwrap_or_default();
            // Skip chunk files - only exclude if they are in the chunks directory
            if !key.starts_with(&format!("{}chunks/", prefix)) {
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

pub(crate) async fn copy_bucket_files(
    s3_client: &Client,
    source_bucket: &str,
    destination_bucket: &str,
) -> Result<usize> {
    let start = Instant::now();
    let mut copied_count = 0;

    tracing::info!(
        source_bucket = %source_bucket,
        destination_bucket = %destination_bucket,
        "Copying all files from source bucket to destination bucket"
    );

    // First, create the destination bucket
    match s3_client
        .create_bucket()
        .bucket(destination_bucket)
        .send()
        .await
    {
        Ok(_) => {
            tracing::debug!(
                bucket = %destination_bucket,
                "Successfully created destination bucket"
            );
        }
        Err(e) => {
            tracing::error!(
                bucket = %destination_bucket,
                error = %e,
                "Failed to create destination bucket - cannot proceed"
            );
            return Err(e.into());
        }
    }

    // List all objects in source bucket
    let mut paginator = s3_client
        .list_objects_v2()
        .bucket(source_bucket)
        .into_paginator()
        .send();

    while let Some(result) = paginator.next().await {
        let output = match result {
            Ok(output) => output,
            Err(e) => {
                tracing::error!(
                    bucket = %source_bucket,
                    error = %e,
                    "Failed to list objects in source bucket"
                );
                return Err(e.into());
            }
        };

        for obj in output.contents() {
            let key = obj.key().unwrap_or_default();
            let copy_source = format!("{}/{}", source_bucket, key);

            // Copy the object
            match s3_client
                .copy_object()
                .copy_source(&copy_source)
                .bucket(destination_bucket)
                .key(key)
                .send()
                .await
            {
                Ok(_) => {
                    copied_count += 1;
                    tracing::debug!(
                        key = %key,
                        source = %source_bucket,
                        destination = %destination_bucket,
                        "Successfully copied file"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        key = %key,
                        source = %source_bucket,
                        destination = %destination_bucket,
                        error = %e,
                        "Failed to copy file"
                    );
                    return Err(e.into());
                }
            }
        }
    }

    let duration = start.elapsed().as_secs_f64();
    record_storage_operation("copy_bucket", duration, None, true);

    tracing::info!(
        source_bucket = %source_bucket,
        destination_bucket = %destination_bucket,
        copied_count = copied_count,
        duration_ms = duration * 1000.0,
        "Successfully copied all files from source to destination bucket"
    );

    Ok(copied_count)
}
#[tracing::instrument(name = "s3.empty_bucket", skip(client), fields(storage.system = "s3", bucket = %bucket))]
pub(crate) async fn empty_bucket(client: &Client, bucket: &str) -> Result<()> {
    let start = Instant::now();

    tracing::debug!(bucket = %bucket, "Emptying S3 bucket");

    // Use single-bucket architecture: S3_BUCKET_NAME/collections/{collection_id}/
    let (actual_bucket, prefix) = get_collection_s3_prefix(bucket)?;

    let mut paginator = client
        .list_objects_v2()
        .bucket(&actual_bucket)
        .prefix(&prefix)
        .into_paginator()
        .send();

    let mut deleted_count = 0;
    const BATCH_SIZE: usize = 1000; // AWS S3 max batch delete size

    while let Some(result) = paginator.next().await {
        let output = match result {
            Ok(output) => output,
            Err(e) => {
                tracing::error!(
                    bucket = %bucket,
                    error = %e,
                    "Failed to list objects in bucket"
                );
                return Err(e.into());
            }
        };

        // Collect object keys for batch deletion
        let contents = output.contents();
        let keys: Vec<_> = contents
            .iter()
            .filter_map(|obj| obj.key().map(|k| k.to_string()))
            .collect();

        if keys.is_empty() {
            continue;
        }

        // Delete objects in batches
        for batch in keys.chunks(BATCH_SIZE) {
            let delete_objects: Vec<_> = batch
                .iter()
                .map(|key| {
                    aws_sdk_s3::types::ObjectIdentifier::builder()
                        .key(key)
                        .build()
                        .expect("Failed to build ObjectIdentifier")
                })
                .collect();

            match client
                .delete_objects()
                .bucket(&actual_bucket)
                .delete(
                    aws_sdk_s3::types::Delete::builder()
                        .set_objects(Some(delete_objects))
                        .build()
                        .expect("Failed to build Delete request"),
                )
                .send()
                .await
            {
                Ok(response) => {
                    let deleted = response.deleted();
                    deleted_count += deleted.len();
                    if !deleted.is_empty() {
                        tracing::debug!(
                            bucket = %bucket,
                            count = deleted.len(),
                            "Batch deleted objects"
                        );
                    }
                    let errors = response.errors();
                    if !errors.is_empty() {
                        for error in errors {
                            tracing::warn!(
                                bucket = %bucket,
                                key = %error.key().unwrap_or("unknown"),
                                code = %error.code().unwrap_or("unknown"),
                                message = %error.message().unwrap_or("unknown"),
                                "Failed to delete object in batch"
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        bucket = %bucket,
                        error = %e,
                        "Failed to batch delete objects"
                    );
                    return Err(e.into());
                }
            }
        }
    }

    let duration = start.elapsed().as_secs_f64();
    record_storage_operation("empty_bucket", duration, None, true);

    tracing::info!(
        bucket = %bucket,
        deleted_count = deleted_count,
        duration_ms = duration * 1000.0,
        "Successfully emptied bucket"
    );

    Ok(())
}
