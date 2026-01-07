use anyhow::Result;
use sqlx::{Pool, Postgres};

use crate::transforms::visualization::{VisualizationTransform, VisualizationTransformStats};

// Query constants
const GET_VISUALIZATION_TRANSFORM_QUERY: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats,
           created_at, updated_at
    FROM visualization_transforms
    WHERE owner = $1 AND visualization_transform_id = $2
"#;

const GET_VISUALIZATION_TRANSFORMS_QUERY: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats,
           created_at, updated_at
    FROM visualization_transforms
    WHERE owner = $1
    ORDER BY created_at DESC
"#;

const GET_VISUALIZATION_TRANSFORMS_FOR_EMBEDDED_DATASET_QUERY: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats,
           created_at, updated_at
    FROM visualization_transforms
    WHERE owner = $1 AND embedded_dataset_id = $2
    ORDER BY created_at DESC
"#;

const GET_ACTIVE_VISUALIZATION_TRANSFORMS_QUERY: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config,
           last_run_status, last_run_at, last_error, last_run_stats,
           created_at, updated_at
    FROM visualization_transforms
    WHERE is_enabled = TRUE
"#;

const CREATE_VISUALIZATION_TRANSFORM_QUERY: &str = r#"
    INSERT INTO visualization_transforms (title, embedded_dataset_id, owner, visualization_config)
    VALUES ($1, $2, $3, $4)
    RETURNING visualization_transform_id, title, embedded_dataset_id, owner, is_enabled,
              reduced_collection_name, topics_collection_name, visualization_config,
              last_run_status, last_run_at, last_error, last_run_stats,
              created_at, updated_at
"#;

const UPDATE_VISUALIZATION_TRANSFORM_QUERY: &str = r#"
    UPDATE visualization_transforms
    SET title = COALESCE($3, title),
        is_enabled = COALESCE($4, is_enabled),
        visualization_config = COALESCE($5, visualization_config),
        updated_at = NOW()
    WHERE owner = $1 AND visualization_transform_id = $2
    RETURNING visualization_transform_id, title, embedded_dataset_id, owner, is_enabled,
              reduced_collection_name, topics_collection_name, visualization_config,
              last_run_status, last_run_at, last_error, last_run_stats,
              created_at, updated_at
"#;

const DELETE_VISUALIZATION_TRANSFORM_QUERY: &str = r#"
    DELETE FROM visualization_transforms
    WHERE owner = $1 AND visualization_transform_id = $2
"#;

const UPDATE_VISUALIZATION_TRANSFORM_STATUS_PROCESSING: &str = r#"
    UPDATE visualization_transforms 
    SET last_run_status = 'processing', 
        last_run_at = NOW(),
        updated_at = NOW() 
    WHERE visualization_transform_id = $1
"#;

const UPDATE_VISUALIZATION_TRANSFORM_STATUS_FAILED: &str = r#"
    UPDATE visualization_transforms 
    SET last_run_status = 'failed', 
        last_run_at = NOW(), 
        last_error = $2,
        updated_at = NOW() 
    WHERE visualization_transform_id = $1
"#;

const UPDATE_VISUALIZATION_TRANSFORM_STATUS_COMPLETED: &str = r#"
    UPDATE visualization_transforms 
    SET reduced_collection_name = $2, 
        topics_collection_name = $3,
        last_run_status = 'completed',
        last_run_at = NOW(),
        last_error = NULL,
        last_run_stats = $4,
        updated_at = NOW() 
    WHERE visualization_transform_id = $1
"#;

// CRUD operations

pub async fn get_visualization_transform(
    pool: &Pool<Postgres>,
    owner: &str,
    visualization_transform_id: i32,
) -> Result<VisualizationTransform> {
    let transform = sqlx::query_as::<_, VisualizationTransform>(GET_VISUALIZATION_TRANSFORM_QUERY)
        .bind(owner)
        .bind(visualization_transform_id)
        .fetch_one(pool)
        .await?;
    Ok(transform)
}

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

pub async fn get_visualization_transforms_for_embedded_dataset(
    pool: &Pool<Postgres>,
    owner: &str,
    embedded_dataset_id: i32,
) -> Result<Vec<VisualizationTransform>> {
    let transforms = sqlx::query_as::<_, VisualizationTransform>(
        GET_VISUALIZATION_TRANSFORMS_FOR_EMBEDDED_DATASET_QUERY,
    )
    .bind(owner)
    .bind(embedded_dataset_id)
    .fetch_all(pool)
    .await?;
    Ok(transforms)
}

pub async fn get_active_visualization_transforms(
    pool: &Pool<Postgres>,
) -> Result<Vec<VisualizationTransform>> {
    let transforms =
        sqlx::query_as::<_, VisualizationTransform>(GET_ACTIVE_VISUALIZATION_TRANSFORMS_QUERY)
            .fetch_all(pool)
            .await?;
    Ok(transforms)
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
            .bind(visualization_config)
            .fetch_one(pool)
            .await?;
    Ok(transform)
}

pub async fn update_visualization_transform(
    pool: &Pool<Postgres>,
    owner: &str,
    visualization_transform_id: i32,
    title: Option<&str>,
    is_enabled: Option<bool>,
    visualization_config: Option<&serde_json::Value>,
) -> Result<VisualizationTransform> {
    let transform =
        sqlx::query_as::<_, VisualizationTransform>(UPDATE_VISUALIZATION_TRANSFORM_QUERY)
            .bind(owner)
            .bind(visualization_transform_id)
            .bind(title)
            .bind(is_enabled)
            .bind(visualization_config)
            .fetch_one(pool)
            .await?;
    Ok(transform)
}

pub async fn delete_visualization_transform(
    pool: &Pool<Postgres>,
    owner: &str,
    visualization_transform_id: i32,
) -> Result<()> {
    sqlx::query(DELETE_VISUALIZATION_TRANSFORM_QUERY)
        .bind(owner)
        .bind(visualization_transform_id)
        .execute(pool)
        .await?;
    Ok(())
}

// Statistics (placeholder - to be implemented based on actual requirements)
pub async fn get_visualization_transform_stats(
    _pool: &Pool<Postgres>,
    visualization_transform_id: i32,
) -> Result<VisualizationTransformStats> {
    // This would typically query the visualization's Qdrant collections for point/cluster counts
    // For now, return placeholder data
    Ok(VisualizationTransformStats {
        visualization_transform_id,
        total_points: 0,
        total_clusters: 0,
        noise_points: 0,
    })
}

// Status update operations

pub async fn update_visualization_transform_status_processing(
    pool: &Pool<Postgres>,
    visualization_transform_id: i32,
) -> Result<()> {
    sqlx::query(UPDATE_VISUALIZATION_TRANSFORM_STATUS_PROCESSING)
        .bind(visualization_transform_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_visualization_transform_status_failed(
    pool: &Pool<Postgres>,
    visualization_transform_id: i32,
    error_message: &str,
) -> Result<()> {
    sqlx::query(UPDATE_VISUALIZATION_TRANSFORM_STATUS_FAILED)
        .bind(visualization_transform_id)
        .bind(error_message)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_visualization_transform_status_completed(
    pool: &Pool<Postgres>,
    visualization_transform_id: i32,
    reduced_collection_name: &str,
    topics_collection_name: &str,
    stats: &serde_json::Value,
) -> Result<()> {
    sqlx::query(UPDATE_VISUALIZATION_TRANSFORM_STATUS_COMPLETED)
        .bind(visualization_transform_id)
        .bind(reduced_collection_name)
        .bind(topics_collection_name)
        .bind(stats)
        .execute(pool)
        .await?;
    Ok(())
}
