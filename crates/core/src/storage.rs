use crate::observability::record_storage_operation;
use anyhow::Result;
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::{Client, config::Credentials};
use serde::Serialize;
use std::{env, time::Instant};

#[derive(Debug, Clone)]
pub struct DocumentUpload {
    pub collection_id: String,
    pub name: String,
    pub content: Vec<u8>,
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CollectionFile {
    pub key: String,
    pub size: i64,
    pub last_modified: Option<String>,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaginatedFiles {
    pub files: Vec<CollectionFile>,
    pub page: i32,
    pub page_size: i32,
    pub has_more: bool,
    pub continuation_token: Option<String>,
}

pub async fn initialize_client() -> Result<aws_sdk_s3::Client> {
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
pub async fn upload_document(client: &Client, document: DocumentUpload) -> Result<()> {
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

#[tracing::instrument(name = "s3.list_files", skip(s3_client), fields(storage.system = "s3", bucket = %bucket, page = %page, page_size = %page_size))]
pub async fn list_files(
    s3_client: &Client,
    bucket: &str,
    page: i32,
    page_size: i32,
) -> Result<PaginatedFiles> {
    let start = Instant::now();

    let mut request = s3_client
        .list_objects_v2()
        .bucket(bucket)
        .max_keys(page_size + 1);

    if page > 0 {
        let skip = page * page_size;
        let mut temp_paginator = s3_client
            .list_objects_v2()
            .bucket(bucket)
            .into_paginator()
            .send();

        let mut count = 0;
        let mut continuation_token = None;

        while let Some(result) = temp_paginator.next().await {
            let output = result?;
            for _ in output.contents() {
                count += 1;
                if count == skip {
                    continuation_token = output.next_continuation_token().map(|s| s.to_string());
                    break;
                }
            }
            if continuation_token.is_some() || count >= skip {
                break;
            }
        }

        if let Some(token) = continuation_token {
            request = request.continuation_token(token);
        }
    }

    let output = request.send().await?;
    let mut files = Vec::with_capacity(page_size as usize);
    let mut count = 0;

    for obj in output.contents() {
        if count < page_size {
            files.push(CollectionFile {
                key: obj.key().unwrap_or_default().to_string(),
                size: obj.size().unwrap_or(0),
                last_modified: obj.last_modified().map(|d| d.to_string()),
                content_type: None,
            });
            count += 1;
        }
    }

    let has_more = output.contents().len() > page_size as usize;
    let continuation_token = output.next_continuation_token().map(|s| s.to_string());

    let duration = start.elapsed().as_secs_f64();
    record_storage_operation("list", duration, None, true);

    Ok(PaginatedFiles {
        files,
        page,
        page_size,
        has_more,
        continuation_token,
    })
}

#[tracing::instrument(name = "s3.ensure_bucket_exists", skip(client), fields(storage.system = "s3", bucket = %bucket))]
pub async fn ensure_bucket_exists(client: &Client, bucket: &str) -> Result<()> {
    match client.create_bucket().bucket(bucket).send().await {
        Ok(_) => Ok(()),
        Err(e) => {
            let error_str = format!("{e:?}");
            if error_str.contains("BucketAlreadyExists")
                || error_str.contains("BucketAlreadyOwnedByYou")
            {
                Ok(())
            } else {
                match client.head_bucket().bucket(bucket).send().await {
                    Ok(_) => Ok(()),
                    Err(_) => {
                        match client
                            .list_objects_v2()
                            .bucket(bucket)
                            .max_keys(1)
                            .send()
                            .await
                        {
                            Ok(_) => Ok(()),
                            Err(_) => Err(e.into()),
                        }
                    }
                }
            }
        }
    }
}

#[tracing::instrument(name = "s3.get_file", skip(client), fields(storage.system = "s3", bucket = %bucket, key = %key))]
pub async fn get_file(client: &Client, bucket: &str, key: &str) -> Result<Vec<u8>> {
    let start = Instant::now();

    let result = client.get_object().bucket(bucket).key(key).send().await;

    match result {
        Ok(output) => {
            let data = output.body.collect().await?.into_bytes();
            let duration = start.elapsed().as_secs_f64();
            record_storage_operation("download", duration, Some(data.len() as u64), true);
            Ok(data.to_vec())
        }
        Err(e) => {
            let duration = start.elapsed().as_secs_f64();
            record_storage_operation("download", duration, None, false);
            Err(e.into())
        }
    }
}

#[tracing::instrument(name = "s3.count_files", skip(client), fields(storage.system = "s3", bucket = %bucket))]
pub async fn count_files(client: &Client, bucket: &str) -> Result<i64> {
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
