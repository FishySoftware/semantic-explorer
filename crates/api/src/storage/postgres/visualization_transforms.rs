use anyhow::Result;
use sqlx::{
    FromRow, Pool, Postgres,
    types::chrono::{DateTime, Utc},
};
use std::time::Instant;

use crate::transforms::visualization::models::{Visualization, VisualizationTransform};
use semantic_explorer_core::models::PaginatedResponse;
use semantic_explorer_core::observability::record_database_query;

/// Helper struct for paginated queries that include total_count via COUNT(*) OVER()
#[derive(Debug, Clone, FromRow)]
struct VisualizationTransformWithCount {
    pub visualization_transform_id: i32,
    pub title: String,
    pub embedded_dataset_id: i32,
    pub owner_id: String,
    pub owner_display_name: String,
    pub is_enabled: bool,
    pub reduced_collection_name: Option<String>,
    pub topics_collection_name: Option<String>,
    pub visualization_config: serde_json::Value,
    pub last_run_status: Option<String>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub last_run_stats: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub total_count: i64,
}

impl VisualizationTransformWithCount {
    fn into_parts(rows: Vec<Self>) -> (i64, Vec<VisualizationTransform>) {
        let total_count = rows.first().map_or(0, |r| r.total_count);
        let items = rows
            .into_iter()
            .map(|r| VisualizationTransform {
                visualization_transform_id: r.visualization_transform_id,
                title: r.title,
                embedded_dataset_id: r.embedded_dataset_id,
                owner_id: r.owner_id,
                owner_display_name: r.owner_display_name,
                is_enabled: r.is_enabled,
                reduced_collection_name: r.reduced_collection_name,
                topics_collection_name: r.topics_collection_name,
                visualization_config: r.visualization_config,
                last_run_status: r.last_run_status,
                last_run_at: r.last_run_at,
                last_error: r.last_error,
                last_run_stats: r.last_run_stats,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect();
        (total_count, items)
    }
}

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

fn validate_sort_field(sort_by: &str) -> Result<&'static str> {
    match sort_by {
        "title" => Ok("title"),
        "is_enabled" => Ok("is_enabled"),
        "last_run_status" => Ok("last_run_status"),
        "created_at" => Ok("created_at"),
        "updated_at" => Ok("updated_at"),
        _ => anyhow::bail!("Invalid sort field: {}", sort_by),
    }
}

fn validate_sort_direction(direction: &str) -> Result<&'static str> {
    match direction.to_lowercase().as_str() {
        "asc" => Ok("ASC"),
        "desc" => Ok("DESC"),
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
    SET status = COALESCE($2, status),
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

/// Atomically update visualization to processing status only if not already completed/failed.
/// This prevents race conditions when processing messages arrive after success/failure.
const UPDATE_VISUALIZATION_TO_PROCESSING_QUERY: &str = r#"
    UPDATE visualizations
    SET status = 'processing',
        started_at = COALESCE($2, started_at)
    WHERE visualization_id = $1
      AND status NOT IN ('completed', 'failed')
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
    let visualization = sqlx::query_as::<_, Visualization>(GET_VISUALIZATION_WITH_OWNER_QUERY)
        .bind(visualization_id)
        .bind(transform_id)
        .bind(owner)
        .fetch_one(pool)
        .await?;

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
    let visualizations = sqlx::query_as::<_, Visualization>(GET_RECENT_VISUALIZATIONS_QUERY)
        .bind(owner)
        .bind(limit)
        .fetch_all(pool)
        .await?;

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

/// Atomically update visualization to processing status only if not already completed/failed.
/// Returns Some(visualization) if update succeeded, None if status was already terminal.
/// This prevents race conditions when processing messages arrive after success/failure.
pub async fn update_visualization_to_processing(
    pool: &Pool<Postgres>,
    visualization_id: i32,
    started_at: Option<DateTime<Utc>>,
) -> Result<Option<Visualization>> {
    let visualization =
        sqlx::query_as::<_, Visualization>(UPDATE_VISUALIZATION_TO_PROCESSING_QUERY)
            .bind(visualization_id)
            .bind(started_at)
            .fetch_optional(pool)
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
    WHERE visualization_transform_id = $1 AND owner_id = $5
    RETURNING visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
              reduced_collection_name, topics_collection_name, visualization_config,
              last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at
"#;

const DELETE_VISUALIZATION_TRANSFORM_QUERY: &str = r#"
    DELETE FROM visualization_transforms
    WHERE visualization_transform_id = $1 AND owner_id = $2
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
    LIMIT $3 OFFSET $4
"#;

// Static sort query variants for plan caching
const VT_PAGINATED_TITLE_ASC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE owner_id = $1
    ORDER BY title ASC LIMIT $2 OFFSET $3
"#;
const VT_PAGINATED_TITLE_DESC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE owner_id = $1
    ORDER BY title DESC LIMIT $2 OFFSET $3
"#;
const VT_PAGINATED_IS_ENABLED_ASC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE owner_id = $1
    ORDER BY is_enabled ASC LIMIT $2 OFFSET $3
"#;
const VT_PAGINATED_IS_ENABLED_DESC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE owner_id = $1
    ORDER BY is_enabled DESC LIMIT $2 OFFSET $3
"#;
const VT_PAGINATED_LAST_RUN_STATUS_ASC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE owner_id = $1
    ORDER BY last_run_status ASC LIMIT $2 OFFSET $3
"#;
const VT_PAGINATED_LAST_RUN_STATUS_DESC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE owner_id = $1
    ORDER BY last_run_status DESC LIMIT $2 OFFSET $3
"#;
const VT_PAGINATED_CREATED_AT_ASC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE owner_id = $1
    ORDER BY created_at ASC LIMIT $2 OFFSET $3
"#;
const VT_PAGINATED_CREATED_AT_DESC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE owner_id = $1
    ORDER BY created_at DESC LIMIT $2 OFFSET $3
"#;
const VT_PAGINATED_UPDATED_AT_ASC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE owner_id = $1
    ORDER BY updated_at ASC LIMIT $2 OFFSET $3
"#;
const VT_PAGINATED_UPDATED_AT_DESC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE owner_id = $1
    ORDER BY updated_at DESC LIMIT $2 OFFSET $3
"#;

const VT_SEARCH_TITLE_ASC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY title ASC LIMIT $3 OFFSET $4
"#;
const VT_SEARCH_TITLE_DESC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY title DESC LIMIT $3 OFFSET $4
"#;
const VT_SEARCH_IS_ENABLED_ASC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY is_enabled ASC LIMIT $3 OFFSET $4
"#;
const VT_SEARCH_IS_ENABLED_DESC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY is_enabled DESC LIMIT $3 OFFSET $4
"#;
const VT_SEARCH_LAST_RUN_STATUS_ASC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY last_run_status ASC LIMIT $3 OFFSET $4
"#;
const VT_SEARCH_LAST_RUN_STATUS_DESC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY last_run_status DESC LIMIT $3 OFFSET $4
"#;
const VT_SEARCH_CREATED_AT_ASC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY created_at ASC LIMIT $3 OFFSET $4
"#;
const VT_SEARCH_CREATED_AT_DESC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY created_at DESC LIMIT $3 OFFSET $4
"#;
const VT_SEARCH_UPDATED_AT_ASC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY updated_at ASC LIMIT $3 OFFSET $4
"#;
const VT_SEARCH_UPDATED_AT_DESC: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner_id, owner_display_name, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at,
           COUNT(*) OVER() AS total_count
    FROM visualization_transforms
    WHERE title ILIKE $1 AND owner_id = $2
    ORDER BY updated_at DESC LIMIT $3 OFFSET $4
"#;

fn get_vt_paginated_query(sort_field: &str, sort_dir: &str) -> &'static str {
    match (sort_field, sort_dir) {
        ("title", "ASC") => VT_PAGINATED_TITLE_ASC,
        ("title", "DESC") => VT_PAGINATED_TITLE_DESC,
        ("is_enabled", "ASC") => VT_PAGINATED_IS_ENABLED_ASC,
        ("is_enabled", "DESC") => VT_PAGINATED_IS_ENABLED_DESC,
        ("last_run_status", "ASC") => VT_PAGINATED_LAST_RUN_STATUS_ASC,
        ("last_run_status", "DESC") => VT_PAGINATED_LAST_RUN_STATUS_DESC,
        ("created_at", "ASC") => VT_PAGINATED_CREATED_AT_ASC,
        ("updated_at", "ASC") => VT_PAGINATED_UPDATED_AT_ASC,
        ("updated_at", "DESC") => VT_PAGINATED_UPDATED_AT_DESC,
        _ => VT_PAGINATED_CREATED_AT_DESC, // default
    }
}

fn get_vt_search_query(sort_field: &str, sort_dir: &str) -> &'static str {
    match (sort_field, sort_dir) {
        ("title", "ASC") => VT_SEARCH_TITLE_ASC,
        ("title", "DESC") => VT_SEARCH_TITLE_DESC,
        ("is_enabled", "ASC") => VT_SEARCH_IS_ENABLED_ASC,
        ("is_enabled", "DESC") => VT_SEARCH_IS_ENABLED_DESC,
        ("last_run_status", "ASC") => VT_SEARCH_LAST_RUN_STATUS_ASC,
        ("last_run_status", "DESC") => VT_SEARCH_LAST_RUN_STATUS_DESC,
        ("created_at", "ASC") => VT_SEARCH_CREATED_AT_ASC,
        ("updated_at", "ASC") => VT_SEARCH_UPDATED_AT_ASC,
        ("updated_at", "DESC") => VT_SEARCH_UPDATED_AT_DESC,
        _ => VT_SEARCH_CREATED_AT_DESC, // default
    }
}

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

    let start = Instant::now();

    let (total_count, transforms) = if let Some(search_term) = search {
        let search_pattern = format!("%{}%", search_term);

        // Use static query variant for plan caching (includes COUNT(*) OVER())
        let query_str = get_vt_search_query(sort_field, sort_dir);

        let rows = sqlx::query_as::<_, VisualizationTransformWithCount>(query_str)
            .bind(&search_pattern)
            .bind(owner)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        VisualizationTransformWithCount::into_parts(rows)
    } else {
        // Use static query variant for plan caching (includes COUNT(*) OVER())
        let query_str = get_vt_paginated_query(sort_field, sort_dir);

        let rows = sqlx::query_as::<_, VisualizationTransformWithCount>(query_str)
            .bind(owner)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        VisualizationTransformWithCount::into_parts(rows)
    };

    let duration = start.elapsed().as_secs_f64();
    record_database_query("SELECT", "visualization_transforms", duration, true);

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
    let transform =
        sqlx::query_as::<_, VisualizationTransform>(CREATE_VISUALIZATION_TRANSFORM_QUERY)
            .bind(title)
            .bind(embedded_dataset_id)
            .bind(owner_id)
            .bind(owner_display_name)
            .bind(true) // is_enabled
            .bind(visualization_config)
            .fetch_one(pool)
            .await?;

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
    let transform =
        sqlx::query_as::<_, VisualizationTransform>(UPDATE_VISUALIZATION_TRANSFORM_QUERY)
            .bind(id)
            .bind(title)
            .bind(is_enabled)
            .bind(visualization_config)
            .bind(owner)
            .fetch_one(pool)
            .await?;

    Ok(transform)
}

pub async fn delete_visualization_transform(
    pool: &Pool<Postgres>,
    id: i32,
    owner: &str,
) -> Result<()> {
    sqlx::query(DELETE_VISUALIZATION_TRANSFORM_QUERY)
        .bind(id)
        .bind(owner)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn get_visualization_transforms_by_embedded_dataset(
    pool: &Pool<Postgres>,
    embedded_dataset_id: i32,
    owner: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<VisualizationTransform>> {
    let transforms = sqlx::query_as::<_, VisualizationTransform>(
        GET_VISUALIZATION_TRANSFORMS_BY_EMBEDDED_DATASET_QUERY,
    )
    .bind(embedded_dataset_id)
    .bind(owner)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

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
