pub(crate) mod models;

use anyhow::{Context, Result, bail};
use aws_sdk_s3::Client;
use semantic_explorer_core::observability::record_storage_operation;

use std::time::Instant;
use tracing::warn;

use crate::storage::s3::models::{CollectionFile, DocumentUpload, S3FileList};

/// Initialize S3 client using shared configuration from core
/// Supports both static credentials and IAM roles
pub(crate) async fn initialize_client() -> Result<aws_sdk_s3::Client> {
    semantic_explorer_core::storage::initialize_client().await
}

/// Upload document to collection using single-bucket architecture
/// Uses: S3_BUCKET_NAME/collections/{collection_id}/{filename}
#[tracing::instrument(name = "s3.upload_document", skip(client, bucket_name, document), fields(storage.system = "s3", collection_id = %document.collection_id, key = %document.name, size = document.size))]
pub(crate) async fn upload_document(
    client: &Client,
    bucket_name: &str,
    document: DocumentUpload,
) -> Result<()> {
    let start = Instant::now();
    let file_size = document.size;
    let key = format!("collections/{}/{}", document.collection_id, document.name);

    tracing::debug!(
        bucket = %bucket_name,
        collection_id = %document.collection_id,
        key = %document.name,
        size = file_size,
        mime_type = %document.mime_type,
        "Uploading document to S3"
    );

    let result = client
        .put_object()
        .bucket(bucket_name)
        .key(&key)
        .body(document.content)
        .content_type(&document.mime_type)
        .send()
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_storage_operation("upload", duration, Some(file_size), success);

    match &result {
        Ok(_) => {
            tracing::debug!(
                bucket = %bucket_name,
                key = %key,
                duration_ms = duration * 1000.0,
                "Successfully uploaded document to S3"
            );
        }
        Err(e) => {
            tracing::error!(
                bucket = %bucket_name,
                collection_id = %document.collection_id,
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

/// Uses: S3_BUCKET_NAME/collections/{collection_id}/
#[tracing::instrument(name = "s3.list_files", skip(s3_client, bucket_name), fields(storage.system = "s3", collection_id = %collection_id, page_size = %page_size))]
pub(crate) async fn list_files(
    s3_client: &Client,
    bucket_name: &str,
    collection_id: &str,
    page_size: i32,
    continuation_token: Option<&str>,
) -> Result<S3FileList> {
    let start = Instant::now();
    let prefix = format!("collections/{}/", collection_id);

    tracing::debug!(
        bucket = %bucket_name,
        collection_id = %collection_id,
        page_size = page_size,
        has_continuation_token = continuation_token.is_some(),
        "Listing files for collection"
    );

    let mut files = Vec::new();
    let page_size_usize = page_size as usize;
    let target_count = page_size_usize + 1; // Need page_size + 1 to know if there are more

    // Build the paginator request
    let mut request = s3_client
        .list_objects_v2()
        .bucket(bucket_name)
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
                    bucket = %bucket_name,
                    prefix = %prefix,
                    error = %e,
                    "Failed to list files in S3 bucket"
                );
                return Err(e.into());
            }
        };

        for obj in output.contents() {
            let key = obj.key().unwrap_or_default();

            // Skip the directory marker itself
            if key == prefix {
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
        bucket = %bucket_name,
        collection_id = %collection_id,
        file_count = files.len(),
        has_more = has_more,
        duration_ms = duration * 1000.0,
        "Successfully listed files for collection"
    );

    Ok(S3FileList {
        files,
        continuation_token: next_cursor,
    })
}

/// Get file from collection using single-bucket architecture
/// Uses: S3_BUCKET_NAME/collections/{collection_id}/{filename}
#[tracing::instrument(name = "s3.get_file", skip(client, bucket_name), fields(storage.system = "s3", collection_id = %collection_id, key = %key))]
pub(crate) async fn get_file(
    client: &Client,
    bucket_name: &str,
    collection_id: &str,
    key: &str,
) -> Result<Vec<u8>> {
    let start = Instant::now();
    let full_key = format!("collections/{}/{}", collection_id, key);

    tracing::debug!(
        bucket = %bucket_name,
        collection_id = %collection_id,
        key = %key,
        "Retrieving file from S3"
    );

    let result = client
        .get_object()
        .bucket(bucket_name)
        .key(&full_key)
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
                bucket = %bucket_name,
                collection_id = %collection_id,
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
                bucket = %bucket_name,
                collection_id = %collection_id,
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
/// Returns error if file exceeds the configured max_download_size_bytes
#[tracing::instrument(name = "s3.get_file_with_size_check", skip(client, bucket_name), fields(storage.system = "s3", collection_id = %collection_id, key = %key))]
pub(crate) async fn get_file_with_size_check(
    client: &Client,
    bucket_name: &str,
    collection_id: &str,
    key: &str,
    max_download_size_bytes: i64,
) -> Result<Vec<u8>> {
    let start = Instant::now();
    let full_key = format!("collections/{}/{}", collection_id, key);

    // First, check file size using head_object
    let head_result = client
        .head_object()
        .bucket(bucket_name)
        .key(&full_key)
        .send()
        .await
        .context("Failed to get file metadata")?;

    let file_size = head_result.content_length().unwrap_or(0);

    if file_size > max_download_size_bytes {
        let duration = start.elapsed().as_secs_f64();
        record_storage_operation("download", duration, None, false);
        warn!(
            bucket = %bucket_name,
            collection_id = %collection_id,
            key = %key,
            file_size_mb = file_size / (1024 * 1024),
            max_size_mb = max_download_size_bytes / (1024 * 1024),
            "File exceeds maximum download size limit"
        );
        bail!(
            "File size ({} MB) exceeds maximum download limit of {} MB",
            file_size / (1024 * 1024),
            max_download_size_bytes / (1024 * 1024)
        );
    }

    // Size is acceptable, proceed with download
    get_file(client, bucket_name, collection_id, key).await
}

/// Delete file from collection using single-bucket architecture
/// Uses: S3_BUCKET_NAME/collections/{collection_id}/{filename}
#[tracing::instrument(name = "s3.delete_file", skip(client, bucket_name), fields(storage.system = "s3", collection_id = %collection_id, key = %key))]
pub(crate) async fn delete_file(
    client: &Client,
    bucket_name: &str,
    collection_id: &str,
    key: &str,
) -> Result<()> {
    let start = Instant::now();
    let full_key = format!("collections/{}/{}", collection_id, key);

    tracing::debug!(
        bucket = %bucket_name,
        collection_id = %collection_id,
        key = %key,
        "Deleting file from S3"
    );

    let result = client
        .delete_object()
        .bucket(bucket_name)
        .key(&full_key)
        .send()
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_storage_operation("delete", duration, None, success);

    match &result {
        Ok(_) => {
            tracing::info!(
                bucket = %bucket_name,
                collection_id = %collection_id,
                key = %key,
                duration_ms = duration * 1000.0,
                "Successfully deleted file from S3"
            );
        }
        Err(e) => {
            tracing::error!(
                bucket = %bucket_name,
                collection_id = %collection_id,
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

/// Count files for a specific collection in the single-bucket architecture
/// Uses: S3_BUCKET_NAME/collections/{collection_id}/
#[tracing::instrument(name = "s3.count_collection_files", skip(client, bucket_name), fields(storage.system = "s3", collection_id = %collection_id))]
pub(crate) async fn count_collection_files(
    client: &Client,
    bucket_name: &str,
    collection_id: i32,
) -> Result<i64> {
    let start = Instant::now();
    let mut count = 0i64;
    let prefix = format!("collections/{}/", collection_id);

    let mut paginator = client
        .list_objects_v2()
        .bucket(bucket_name)
        .prefix(&prefix)
        .into_paginator()
        .send();

    while let Some(result) = paginator.next().await {
        let output = match result {
            Ok(output) => output,
            Err(e) => {
                tracing::error!(
                    bucket = %bucket_name,
                    prefix = %prefix,
                    error = %e,
                    "Failed to count files in S3 bucket. Check network connectivity and bucket existence."
                );
                return Err(e.into());
            }
        };
        for obj in output.contents() {
            let key = obj.key().unwrap_or_default();
            // Skip the directory marker itself
            if key != prefix {
                count += 1;
            }
        }
    }

    let duration = start.elapsed().as_secs_f64();
    record_storage_operation("count", duration, None, true);

    tracing::debug!(
        bucket = %bucket_name,
        collection_id = %collection_id,
        file_count = count,
        duration_ms = duration * 1000.0,
        "Successfully counted files for collection"
    );

    Ok(count)
}

/// Copy files from one collection prefix to another within the same bucket
/// Uses single-bucket architecture: S3_BUCKET_NAME/collections/{source_collection_id}/* -> S3_BUCKET_NAME/collections/{dest_collection_id}/*
pub(crate) async fn copy_collection_files(
    s3_client: &Client,
    bucket_name: &str,
    source_collection_id: i32,
    destination_collection_id: i32,
) -> Result<usize> {
    let start = Instant::now();
    let mut copied_count = 0;

    let source_prefix = format!("collections/{}/", source_collection_id);
    let dest_prefix = format!("collections/{}/", destination_collection_id);

    tracing::debug!(
        bucket = %bucket_name,
        source_prefix = %source_prefix,
        dest_prefix = %dest_prefix,
        "Copying collection files within bucket"
    );

    // List all objects in source collection prefix
    let mut paginator = s3_client
        .list_objects_v2()
        .bucket(bucket_name)
        .prefix(&source_prefix)
        .into_paginator()
        .send();

    while let Some(result) = paginator.next().await {
        let output = match result {
            Ok(output) => output,
            Err(e) => {
                tracing::error!(
                    bucket = %bucket_name,
                    source_prefix = %source_prefix,
                    error = %e,
                    "Failed to list objects in source collection prefix"
                );
                return Err(e.into());
            }
        };

        for obj in output.contents() {
            let source_key = obj.key().unwrap_or_default();

            // Extract the filename from the source key (after the prefix)
            let filename = source_key
                .strip_prefix(&source_prefix)
                .unwrap_or(source_key);
            let dest_key = format!("{}{}", dest_prefix, filename);
            let copy_source = format!("{}/{}", bucket_name, source_key);

            // Copy the object within the same bucket
            match s3_client
                .copy_object()
                .copy_source(&copy_source)
                .bucket(bucket_name)
                .key(&dest_key)
                .send()
                .await
            {
                Ok(_) => {
                    copied_count += 1;
                    tracing::debug!(
                        source_key = %source_key,
                        dest_key = %dest_key,
                        "Successfully copied file"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        source_key = %source_key,
                        dest_key = %dest_key,
                        error = %e,
                        "Failed to copy file"
                    );
                    return Err(e.into());
                }
            }
        }
    }

    let duration = start.elapsed().as_secs_f64();
    record_storage_operation("copy_collection", duration, None, true);

    tracing::info!(
        bucket = %bucket_name,
        source_collection_id = %source_collection_id,
        dest_collection_id = %destination_collection_id,
        copied_count = copied_count,
        duration_ms = duration * 1000.0,
        "Successfully copied all files from source to destination collection prefix"
    );

    Ok(copied_count)
}

/// Empty all files in a collection using single-bucket architecture
/// Uses: S3_BUCKET_NAME/collections/{collection_id}/
#[tracing::instrument(name = "s3.empty_collection", skip(client, bucket_name), fields(storage.system = "s3", collection_id = %collection_id))]
pub(crate) async fn empty_collection(
    client: &Client,
    bucket_name: &str,
    collection_id: i32,
) -> Result<()> {
    let start = Instant::now();
    let prefix = format!("collections/{}/", collection_id);

    tracing::debug!(
        bucket = %bucket_name,
        collection_id = %collection_id,
        "Emptying collection files from S3"
    );

    let mut paginator = client
        .list_objects_v2()
        .bucket(bucket_name)
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
                    bucket = %bucket_name,
                    collection_id = %collection_id,
                    error = %e,
                    "Failed to list objects in collection"
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
                .bucket(bucket_name)
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
                            bucket = %bucket_name,
                            collection_id = %collection_id,
                            count = deleted.len(),
                            "Batch deleted objects"
                        );
                    }
                    let errors = response.errors();
                    if !errors.is_empty() {
                        for error in errors {
                            tracing::warn!(
                                bucket = %bucket_name,
                                collection_id = %collection_id,
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
                        bucket = %bucket_name,
                        collection_id = %collection_id,
                        error = %e,
                        "Failed to batch delete objects"
                    );
                    return Err(e.into());
                }
            }
        }
    }

    let duration = start.elapsed().as_secs_f64();
    record_storage_operation("empty_collection", duration, None, true);

    tracing::info!(
        bucket = %bucket_name,
        collection_id = %collection_id,
        deleted_count = deleted_count,
        duration_ms = duration * 1000.0,
        "Successfully emptied collection"
    );

    Ok(())
}
