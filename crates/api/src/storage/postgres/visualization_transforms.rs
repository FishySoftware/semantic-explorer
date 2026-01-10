use anyhow::Result;
use sqlx::{
    Pool, Postgres,
    types::chrono::{DateTime, Utc},
};

use crate::transforms::visualization::models::{VisualizationTransform, VisualizationTransformRun};

// ============================================================================
// Visualization Transform Runs (NEW: for tracking individual executions)
// ============================================================================

const CREATE_VISUALIZATION_RUN_QUERY: &str = r#"
    INSERT INTO visualization_transform_runs (visualization_transform_id, status, created_at)
    VALUES ($1, $2, NOW())
    RETURNING run_id, visualization_transform_id, status, started_at, completed_at,
              html_s3_key, point_count, cluster_count, error_message, stats_json, created_at
"#;

const GET_VISUALIZATION_RUN_QUERY: &str = r#"
    SELECT run_id, visualization_transform_id, status, started_at, completed_at,
           html_s3_key, point_count, cluster_count, error_message, stats_json, created_at
    FROM visualization_transform_runs
    WHERE run_id = $1
"#;

const GET_VISUALIZATION_RUN_WITH_OWNER_QUERY: &str = r#"
    SELECT vtr.run_id, vtr.visualization_transform_id, vtr.status, vtr.started_at, vtr.completed_at,
           vtr.html_s3_key, vtr.point_count, vtr.cluster_count, vtr.error_message, vtr.stats_json, vtr.created_at
    FROM visualization_transform_runs vtr
    INNER JOIN visualization_transforms vt ON vtr.visualization_transform_id = vt.visualization_transform_id
    WHERE vtr.run_id = $1 AND vt.owner = $2
"#;

const GET_LATEST_VISUALIZATION_RUN_QUERY: &str = r#"
    SELECT run_id, visualization_transform_id, status, started_at, completed_at,
           html_s3_key, point_count, cluster_count, error_message, stats_json, created_at
    FROM visualization_transform_runs
    WHERE visualization_transform_id = $1
    ORDER BY created_at DESC
    LIMIT 1
"#;

const LIST_VISUALIZATION_RUNS_QUERY: &str = r#"
    SELECT run_id, visualization_transform_id, status, started_at, completed_at,
           html_s3_key, point_count, cluster_count, error_message, stats_json, created_at
    FROM visualization_transform_runs
    WHERE visualization_transform_id = $1
    ORDER BY created_at DESC
    LIMIT $2 OFFSET $3
"#;

const UPDATE_VISUALIZATION_RUN_QUERY: &str = r#"
    UPDATE visualization_transform_runs
    SET status = COALESCE($2, status),
        started_at = COALESCE($3, started_at),
        completed_at = COALESCE($4, completed_at),
        html_s3_key = COALESCE($5, html_s3_key),
        point_count = COALESCE($6, point_count),
        cluster_count = COALESCE($7, cluster_count),
        error_message = COALESCE($8, error_message),
        stats_json = COALESCE($9, stats_json)
    WHERE run_id = $1
    RETURNING run_id, visualization_transform_id, status, started_at, completed_at,
              html_s3_key, point_count, cluster_count, error_message, stats_json, created_at
"#;

pub async fn create_visualization_run(
    pool: &Pool<Postgres>,
    visualization_transform_id: i32,
) -> Result<VisualizationTransformRun> {
    let run = sqlx::query_as::<_, VisualizationTransformRun>(CREATE_VISUALIZATION_RUN_QUERY)
        .bind(visualization_transform_id)
        .bind("pending")
        .fetch_one(pool)
        .await?;
    Ok(run)
}

pub async fn get_visualization_run(
    pool: &Pool<Postgres>,
    run_id: i32,
) -> Result<VisualizationTransformRun> {
    let run = sqlx::query_as::<_, VisualizationTransformRun>(GET_VISUALIZATION_RUN_QUERY)
        .bind(run_id)
        .fetch_one(pool)
        .await?;
    Ok(run)
}

pub async fn get_visualization_run_with_owner(
    pool: &Pool<Postgres>,
    run_id: i32,
    owner: &str,
) -> Result<VisualizationTransformRun> {
    let run = sqlx::query_as::<_, VisualizationTransformRun>(GET_VISUALIZATION_RUN_WITH_OWNER_QUERY)
        .bind(run_id)
        .bind(owner)
        .fetch_one(pool)
        .await?;
    Ok(run)
}

pub async fn get_latest_visualization_run(
    pool: &Pool<Postgres>,
    visualization_transform_id: i32,
) -> Result<Option<VisualizationTransformRun>> {
    let run = sqlx::query_as::<_, VisualizationTransformRun>(GET_LATEST_VISUALIZATION_RUN_QUERY)
        .bind(visualization_transform_id)
        .fetch_optional(pool)
        .await?;
    Ok(run)
}

pub async fn list_visualization_runs(
    pool: &Pool<Postgres>,
    visualization_transform_id: i32,
    limit: i64,
    offset: i64,
) -> Result<Vec<VisualizationTransformRun>> {
    let runs = sqlx::query_as::<_, VisualizationTransformRun>(LIST_VISUALIZATION_RUNS_QUERY)
        .bind(visualization_transform_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    Ok(runs)
}

#[allow(clippy::too_many_arguments)]
pub async fn update_visualization_run(
    pool: &Pool<Postgres>,
    run_id: i32,
    status: Option<&str>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    html_s3_key: Option<&str>,
    point_count: Option<i32>,
    cluster_count: Option<i32>,
    error_message: Option<&str>,
    stats_json: Option<&serde_json::Value>,
) -> Result<VisualizationTransformRun> {
    let run = sqlx::query_as::<_, VisualizationTransformRun>(UPDATE_VISUALIZATION_RUN_QUERY)
        .bind(run_id)
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
    Ok(run)
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
