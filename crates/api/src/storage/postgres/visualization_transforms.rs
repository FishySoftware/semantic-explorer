use anyhow::Result;
use sqlx::{
    Pool, Postgres,
    types::chrono::{DateTime, Utc},
};

use crate::transforms::visualization::models::{Visualization, VisualizationTransform};
use semantic_explorer_core::models::PaginatedResponse;

/// Builder for updating visualization fields.
///
/// This builder provides a clean API for constructing partial updates to visualizations
/// with optional fields.
#[derive(Debug, Default, Clone)]
pub struct VisualizationUpdate {
    pub status: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub html_s3_key: Option<String>,
    pub point_count: Option<i32>,
    pub cluster_count: Option<i32>,
    pub error_message: Option<String>,
    pub stats_json: Option<serde_json::Value>,
}

impl VisualizationUpdate {
    /// Create a new empty update builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the status
    pub fn status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }

    /// Set the started_at timestamp
    pub fn started_at(mut self, started_at: DateTime<Utc>) -> Self {
        self.started_at = Some(started_at);
        self
    }

    /// Set the completed_at timestamp
    pub fn completed_at(mut self, completed_at: DateTime<Utc>) -> Self {
        self.completed_at = Some(completed_at);
        self
    }

    /// Set the HTML S3 key
    pub fn html_s3_key(mut self, html_s3_key: impl Into<String>) -> Self {
        self.html_s3_key = Some(html_s3_key.into());
        self
    }

    /// Set the point count
    pub fn point_count(mut self, point_count: i32) -> Self {
        self.point_count = Some(point_count);
        self
    }

    /// Set the cluster count
    pub fn cluster_count(mut self, cluster_count: i32) -> Self {
        self.cluster_count = Some(cluster_count);
        self
    }

    /// Set the error message
    pub fn error_message(mut self, error_message: impl Into<String>) -> Self {
        self.error_message = Some(error_message.into());
        self
    }

    /// Set the stats JSON
    pub fn stats_json(mut self, stats_json: serde_json::Value) -> Self {
        self.stats_json = Some(stats_json);
        self
    }
}

fn validate_sort_field(sort_by: &str) -> Result<String> {
    match sort_by {
        "title" | "is_enabled" | "last_run_status" | "created_at" | "updated_at" => {
            Ok(sort_by.to_string())
        }
        _ => anyhow::bail!("Invalid sort field: {}", sort_by),
    }
}

fn validate_sort_direction(direction: &str) -> Result<String> {
    match direction.to_lowercase().as_str() {
        "asc" | "desc" => Ok(direction.to_uppercase()),
        _ => anyhow::bail!("Invalid sort direction: {}", direction),
    }
}

const CREATE_VISUALIZATION_QUERY: &str = r#"
    INSERT INTO visualizations (visualization_transform_id, status, created_at)
    VALUES ($1, $2, NOW())
    RETURNING visualization_id, visualization_transform_id, status, started_at, completed_at,
              html_s3_key, point_count, cluster_count, error_message, stats_json, created_at
"#;

const GET_VISUALIZATION_QUERY: &str = r#"
    SELECT visualization_id, visualization_transform_id, status, started_at, completed_at,
           html_s3_key, point_count, cluster_count, error_message, stats_json, created_at
    FROM visualizations
    WHERE visualization_id = $1
"#;

const GET_VISUALIZATION_WITH_OWNER_QUERY: &str = r#"
    SELECT v.visualization_id, v.visualization_transform_id, v.status, v.started_at, v.completed_at,
           v.html_s3_key, v.point_count, v.cluster_count, v.error_message, v.stats_json, v.created_at
    FROM visualizations v
    INNER JOIN visualization_transforms vt ON v.visualization_transform_id = vt.visualization_transform_id
    WHERE v.visualization_id = $1 AND v.visualization_transform_id = $2 AND vt.owner_id = $3
"#;

const GET_LATEST_VISUALIZATION_QUERY: &str = r#"
    SELECT visualization_id, visualization_transform_id, status, started_at, completed_at,
           html_s3_key, point_count, cluster_count, error_message, stats_json, created_at
    FROM visualizations
    WHERE visualization_transform_id = $1
    ORDER BY created_at DESC
    LIMIT 1
"#;

const LIST_VISUALIZATIONS_QUERY: &str = r#"
    SELECT visualization_id, visualization_transform_id, status, started_at, completed_at,
           html_s3_key, point_count, cluster_count, error_message, stats_json, created_at
    FROM visualizations
    WHERE visualization_transform_id = $1
    ORDER BY created_at DESC
    LIMIT $2 OFFSET $3
"#;

const UPDATE_VISUALIZATION_QUERY: &str = r#"
    UPDATE visualizations
    SET status = $2,
        started_at = COALESCE($3, started_at),
        completed_at = COALESCE($4, completed_at),
        html_s3_key = COALESCE($5, html_s3_key),
        point_count = COALESCE($6, point_count),
        cluster_count = COALESCE($7, cluster_count),
        error_message = COALESCE($8, error_message),
        stats_json = COALESCE($9, stats_json)
    WHERE visualization_id = $1
    RETURNING visualization_id, visualization_transform_id, status, started_at, completed_at,
              html_s3_key, point_count, cluster_count, error_message, stats_json, created_at
"#;

const GET_RECENT_VISUALIZATIONS_QUERY: &str = r#"
    SELECT v.visualization_id, v.visualization_transform_id, v.status, v.started_at, v.completed_at,
           v.html_s3_key, v.point_count, v.cluster_count, v.error_message, v.stats_json, v.created_at
    FROM visualizations v
    INNER JOIN visualization_transforms vt ON v.visualization_transform_id = vt.visualization_transform_id
    WHERE vt.owner_id = $1
    ORDER BY v.created_at DESC
    LIMIT $2
"#;

pub async fn create_visualization(
    pool: &Pool<Postgres>,
    visualization_transform_id: i32,
) -> Result<Visualization> {
    let visualization = sqlx::query_as::<_, Visualization>(CREATE_VISUALIZATION_QUERY)
        .bind(visualization_transform_id)
        .bind("pending")
        .fetch_one(pool)
        .await?;
    Ok(visualization)
}

pub async fn get_visualization(
    pool: &Pool<Postgres>,
    visualization_id: i32,
) -> Result<Visualization> {
    let visualization = sqlx::query_as::<_, Visualization>(GET_VISUALIZATION_QUERY)
        .bind(visualization_id)
        .fetch_one(pool)
        .await?;
    Ok(visualization)
}

pub async fn get_visualization_with_owner(
    pool: &Pool<Postgres>,
    visualization_id: i32,
    transform_id: i32,
    owner: &str,
) -> Result<Visualization> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    let visualization = sqlx::query_as::<_, Visualization>(GET_VISUALIZATION_WITH_OWNER_QUERY)
        .bind(visualization_id)
        .bind(transform_id)
        .bind(owner)
        .fetch_one(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(visualization)
}

pub async fn get_latest_visualization(
    pool: &Pool<Postgres>,
    visualization_transform_id: i32,
) -> Result<Option<Visualization>> {
    let visualization = sqlx::query_as::<_, Visualization>(GET_LATEST_VISUALIZATION_QUERY)
        .bind(visualization_transform_id)
        .fetch_optional(pool)
        .await?;
    Ok(visualization)
}

pub async fn list_visualizations(
    pool: &Pool<Postgres>,
    visualization_transform_id: i32,
    limit: i64,
    offset: i64,
) -> Result<Vec<Visualization>> {
    let visualizations = sqlx::query_as::<_, Visualization>(LIST_VISUALIZATIONS_QUERY)
        .bind(visualization_transform_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    Ok(visualizations)
}

pub async fn get_recent_visualizations(
    pool: &Pool<Postgres>,
    owner: &str,
    limit: i64,
) -> Result<Vec<Visualization>> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    let visualizations = sqlx::query_as::<_, Visualization>(GET_RECENT_VISUALIZATIONS_QUERY)
        .bind(owner)
        .bind(limit)
        .fetch_all(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(visualizations)
}

pub async fn update_visualization(
    pool: &Pool<Postgres>,
    visualization_id: i32,
    update: &VisualizationUpdate,
) -> Result<Visualization> {
    let visualization = sqlx::query_as::<_, Visualization>(UPDATE_VISUALIZATION_QUERY)
        .bind(visualization_id)
        .bind(update.status.as_deref())
        .bind(update.started_at)
        .bind(update.completed_at)
        .bind(update.html_s3_key.as_deref())
        .bind(update.point_count)
        .bind(update.cluster_count)
        .bind(update.error_message.as_deref())
        .bind(update.stats_json.as_ref())
        .fetch_one(pool)
        .await?;
    Ok(visualization)
}

const GET_VISUALIZATION_TRANSFORM_BY_ID_QUERY: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at
    FROM visualization_transforms
    WHERE visualization_transform_id = $1
"#;

const CREATE_VISUALIZATION_TRANSFORM_QUERY: &str = r#"
    INSERT INTO visualization_transforms (
        title, embedded_dataset_id, owner_id, owner_display_name, is_enabled, visualization_config, created_at, updated_at
    )
    VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
    RETURNING visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
              reduced_collection_name, topics_collection_name, visualization_config,
              last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at
"#;

const UPDATE_VISUALIZATION_TRANSFORM_QUERY: &str = r#"
    UPDATE visualization_transforms
    SET title = COALESCE($2, title),
        is_enabled = COALESCE($3, is_enabled),
        visualization_config = COALESCE($4, visualization_config),
        updated_at = NOW()
    WHERE visualization_transform_id = $1
    RETURNING visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
              reduced_collection_name, topics_collection_name, visualization_config,
              last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at
"#;

const DELETE_VISUALIZATION_TRANSFORM_QUERY: &str = r#"
    DELETE FROM visualization_transforms
    WHERE visualization_transform_id = $1
"#;

const UPDATE_VISUALIZATION_TRANSFORM_STATUS_QUERY: &str = r#"
    UPDATE visualization_transforms
    SET last_run_status = COALESCE($2, last_run_status),
        last_run_at = COALESCE($3, last_run_at),
        last_error = $4,
        last_run_stats = COALESCE($5, last_run_stats),
        updated_at = NOW()
    WHERE visualization_transform_id = $1
"#;

const GET_VISUALIZATION_TRANSFORMS_BY_EMBEDDED_DATASET_QUERY: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at
    FROM visualization_transforms
    WHERE embedded_dataset_id = $1 AND owner_id = $2
    ORDER BY created_at DESC
"#;

const COUNT_VISUALIZATION_TRANSFORMS_QUERY: &str =
    "SELECT COUNT(*) as count FROM visualization_transforms WHERE owner_id = $1";
const COUNT_VISUALIZATION_TRANSFORMS_WITH_SEARCH_QUERY: &str =
    "SELECT COUNT(*) as count FROM visualization_transforms WHERE title ILIKE $1 AND owner_id = $2";

// Note: ORDER BY clause is built dynamically with validated identifiers
// Column names cannot be parameterized in PostgreSQL, so we validate and use format!
const GET_VISUALIZATION_TRANSFORMS_PAGINATED_BASE: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at
    FROM visualization_transforms
    WHERE owner_id = $1
"#;

const GET_VISUALIZATION_TRANSFORMS_PAGINATED_WITH_SEARCH_BASE: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at
    FROM visualization_transforms
    WHERE title ILIKE $1
    AND owner_id = $2
"#;

pub async fn get_visualization_transforms_paginated(
    pool: &Pool<Postgres>,
    owner: &str,
    limit: i64,
    offset: i64,
    sort_by: &str,
    sort_direction: &str,
    search: Option<&str>,
) -> Result<PaginatedResponse<VisualizationTransform>> {
    // Validate identifiers against allowlist to prevent SQL injection
    let sort_field = validate_sort_field(sort_by)?;
    let sort_dir = validate_sort_direction(sort_direction)?;

    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    let (total_count, transforms) = if let Some(search_term) = search {
        let search_pattern = format!("%{}%", search_term);

        let count_result: (i64,) = sqlx::query_as(COUNT_VISUALIZATION_TRANSFORMS_WITH_SEARCH_QUERY)
            .bind(&search_pattern)
            .bind(owner)
            .fetch_one(&mut *tx)
            .await?;
        let total = count_result.0;

        // Build query with validated identifiers (column names cannot be parameterized)
        let query_str = format!(
            "{} ORDER BY {} {} LIMIT $3 OFFSET $4",
            GET_VISUALIZATION_TRANSFORMS_PAGINATED_WITH_SEARCH_BASE, sort_field, sort_dir
        );

        let items = sqlx::query_as::<_, VisualizationTransform>(&query_str)
            .bind(&search_pattern)
            .bind(owner)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *tx)
            .await?;

        (total, items)
    } else {
        let count_result: (i64,) = sqlx::query_as(COUNT_VISUALIZATION_TRANSFORMS_QUERY)
            .bind(owner)
            .fetch_one(&mut *tx)
            .await?;
        let total = count_result.0;

        // Build query with validated identifiers (column names cannot be parameterized)
        let query_str = format!(
            "{} ORDER BY {} {} LIMIT $2 OFFSET $3",
            GET_VISUALIZATION_TRANSFORMS_PAGINATED_BASE, sort_field, sort_dir
        );

        let items = sqlx::query_as::<_, VisualizationTransform>(&query_str)
            .bind(owner)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *tx)
            .await?;

        (total, items)
    };

    tx.commit().await?;

    Ok(PaginatedResponse {
        items: transforms,
        total_count,
        limit,
        offset,
    })
}

pub async fn get_visualization_transform_by_id(
    pool: &Pool<Postgres>,
    id: i32,
) -> Result<Option<VisualizationTransform>> {
    let transform =
        sqlx::query_as::<_, VisualizationTransform>(GET_VISUALIZATION_TRANSFORM_BY_ID_QUERY)
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(transform)
}

pub async fn create_visualization_transform(
    pool: &Pool<Postgres>,
    title: &str,
    embedded_dataset_id: i32,
    owner_id: &str,
    owner_display_name: &str,
    visualization_config: &serde_json::Value,
) -> Result<VisualizationTransform> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner_id).await?;

    let transform =
        sqlx::query_as::<_, VisualizationTransform>(CREATE_VISUALIZATION_TRANSFORM_QUERY)
            .bind(title)
            .bind(embedded_dataset_id)
            .bind(owner_id)
            .bind(owner_display_name)
            .bind(true) // is_enabled
            .bind(visualization_config)
            .fetch_one(&mut *tx)
            .await?;

    tx.commit().await?;
    Ok(transform)
}

pub async fn update_visualization_transform(
    pool: &Pool<Postgres>,
    id: i32,
    owner: &str,
    title: Option<&str>,
    is_enabled: Option<bool>,
    visualization_config: Option<&serde_json::Value>,
) -> Result<VisualizationTransform> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    let transform =
        sqlx::query_as::<_, VisualizationTransform>(UPDATE_VISUALIZATION_TRANSFORM_QUERY)
            .bind(id)
            .bind(title)
            .bind(is_enabled)
            .bind(visualization_config)
            .fetch_one(&mut *tx)
            .await?;

    tx.commit().await?;
    Ok(transform)
}

pub async fn delete_visualization_transform(
    pool: &Pool<Postgres>,
    id: i32,
    owner: &str,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    sqlx::query(DELETE_VISUALIZATION_TRANSFORM_QUERY)
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn get_visualization_transforms_by_embedded_dataset(
    pool: &Pool<Postgres>,
    embedded_dataset_id: i32,
    owner: &str,
) -> Result<Vec<VisualizationTransform>> {
    let mut tx = pool.begin().await?;
    super::rls::set_rls_user_tx(&mut tx, owner).await?;

    let transforms = sqlx::query_as::<_, VisualizationTransform>(
        GET_VISUALIZATION_TRANSFORMS_BY_EMBEDDED_DATASET_QUERY,
    )
    .bind(embedded_dataset_id)
    .bind(owner)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(transforms)
}

/// Update the status fields on a visualization transform (last_run_status, last_run_at, last_error, last_run_stats)
pub async fn update_visualization_transform_status(
    pool: &Pool<Postgres>,
    id: i32,
    status: Option<&str>,
    run_at: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
    error: Option<&str>,
    stats: Option<&serde_json::Value>,
) -> Result<()> {
    sqlx::query(UPDATE_VISUALIZATION_TRANSFORM_STATUS_QUERY)
        .bind(id)
        .bind(status)
        .bind(run_at)
        .bind(error)
        .bind(stats)
        .execute(pool)
        .await?;
    Ok(())
}
