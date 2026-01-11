use anyhow::Result;
use sqlx::{
    Pool, Postgres,
    types::chrono::{DateTime, Utc},
};

use crate::transforms::visualization::models::{Visualization, VisualizationTransform};

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
    WHERE v.visualization_id = $1 AND vt.owner = $2
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
    WHERE vt.owner = $1
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
    owner: &str,
) -> Result<Visualization> {
    let visualization = sqlx::query_as::<_, Visualization>(GET_VISUALIZATION_WITH_OWNER_QUERY)
        .bind(visualization_id)
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

#[allow(clippy::too_many_arguments)]
pub async fn update_visualization(
    pool: &Pool<Postgres>,
    visualization_id: i32,
    status: Option<&str>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    html_s3_key: Option<&str>,
    point_count: Option<i32>,
    cluster_count: Option<i32>,
    error_message: Option<&str>,
    stats_json: Option<&serde_json::Value>,
) -> Result<Visualization> {
    let visualization = sqlx::query_as::<_, Visualization>(UPDATE_VISUALIZATION_QUERY)
        .bind(visualization_id)
        .bind(status)
        .bind(started_at)
        .bind(completed_at)
        .bind(html_s3_key)
        .bind(point_count)
        .bind(cluster_count)
        .bind(error_message)
        .bind(stats_json)
        .fetch_one(pool)
        .await?;
    Ok(visualization)
}

// ============================================================================
// Visualization Transforms CRUD
// ============================================================================

const GET_VISUALIZATION_TRANSFORMS_QUERY: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at
    FROM visualization_transforms
    WHERE owner = $1
    ORDER BY created_at DESC
"#;

const GET_VISUALIZATION_TRANSFORM_BY_ID_QUERY: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at
    FROM visualization_transforms
    WHERE visualization_transform_id = $1
"#;

const CREATE_VISUALIZATION_TRANSFORM_QUERY: &str = r#"
    INSERT INTO visualization_transforms (
        title, embedded_dataset_id, owner, is_enabled, visualization_config, created_at, updated_at
    )
    VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
    RETURNING visualization_transform_id, title, embedded_dataset_id, owner, is_enabled,
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
    RETURNING visualization_transform_id, title, embedded_dataset_id, owner, is_enabled,
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
    SELECT visualization_transform_id, title, embedded_dataset_id, owner, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats, created_at, updated_at
    FROM visualization_transforms
    WHERE embedded_dataset_id = $1 AND owner = $2
    ORDER BY created_at DESC
"#;

pub async fn get_visualization_transforms(
    pool: &Pool<Postgres>,
    owner: &str,
) -> Result<Vec<VisualizationTransform>> {
    let transforms =
        sqlx::query_as::<_, VisualizationTransform>(GET_VISUALIZATION_TRANSFORMS_QUERY)
            .bind(owner)
            .fetch_all(pool)
            .await?;
    Ok(transforms)
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
    owner: &str,
    visualization_config: &serde_json::Value,
) -> Result<VisualizationTransform> {
    let transform =
        sqlx::query_as::<_, VisualizationTransform>(CREATE_VISUALIZATION_TRANSFORM_QUERY)
            .bind(title)
            .bind(embedded_dataset_id)
            .bind(owner)
            .bind(true) // is_enabled
            .bind(visualization_config)
            .fetch_one(pool)
            .await?;
    Ok(transform)
}

pub async fn update_visualization_transform(
    pool: &Pool<Postgres>,
    id: i32,
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
            .fetch_one(pool)
            .await?;
    Ok(transform)
}

pub async fn delete_visualization_transform(pool: &Pool<Postgres>, id: i32) -> Result<()> {
    sqlx::query(DELETE_VISUALIZATION_TRANSFORM_QUERY)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_visualization_transforms_by_embedded_dataset(
    pool: &Pool<Postgres>,
    embedded_dataset_id: i32,
    owner: &str,
) -> Result<Vec<VisualizationTransform>> {
    let transforms = sqlx::query_as::<_, VisualizationTransform>(
        GET_VISUALIZATION_TRANSFORMS_BY_EMBEDDED_DATASET_QUERY,
    )
    .bind(embedded_dataset_id)
    .bind(owner)
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
