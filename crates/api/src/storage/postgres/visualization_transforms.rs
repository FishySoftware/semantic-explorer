use anyhow::Result;
use sqlx::{Pool, Postgres};

use crate::transforms::visualization::{VisualizationTransform, VisualizationTransformStats};

// Query constants
const GET_VISUALIZATION_TRANSFORM_QUERY: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config, created_at, updated_at
    FROM visualization_transforms
    WHERE owner = $1 AND visualization_transform_id = $2
"#;

const GET_VISUALIZATION_TRANSFORMS_QUERY: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config, created_at, updated_at
    FROM visualization_transforms
    WHERE owner = $1
    ORDER BY created_at DESC
"#;

const GET_VISUALIZATION_TRANSFORMS_FOR_EMBEDDED_DATASET_QUERY: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config, created_at, updated_at
    FROM visualization_transforms
    WHERE owner = $1 AND embedded_dataset_id = $2
    ORDER BY created_at DESC
"#;

const GET_ACTIVE_VISUALIZATION_TRANSFORMS_QUERY: &str = r#"
    SELECT visualization_transform_id, title, embedded_dataset_id, owner, is_enabled,
           reduced_collection_name, topics_collection_name, visualization_config, created_at, updated_at
    FROM visualization_transforms
    WHERE is_enabled = TRUE
    ORDER BY created_at DESC
"#;

const CREATE_VISUALIZATION_TRANSFORM_QUERY: &str = r#"
    INSERT INTO visualization_transforms (title, embedded_dataset_id, owner, visualization_config)
    VALUES ($1, $2, $3, $4)
    RETURNING visualization_transform_id, title, embedded_dataset_id, owner, is_enabled,
              reduced_collection_name, topics_collection_name, visualization_config, created_at, updated_at
"#;

const UPDATE_VISUALIZATION_TRANSFORM_QUERY: &str = r#"
    UPDATE visualization_transforms
    SET title = COALESCE($3, title),
        is_enabled = COALESCE($4, is_enabled),
        visualization_config = COALESCE($5, visualization_config),
        updated_at = NOW()
    WHERE owner = $1 AND visualization_transform_id = $2
    RETURNING visualization_transform_id, title, embedded_dataset_id, owner, is_enabled,
              reduced_collection_name, topics_collection_name, visualization_config, created_at, updated_at
"#;

const UPDATE_VISUALIZATION_TRANSFORM_COLLECTION_NAMES_QUERY: &str = r#"
    UPDATE visualization_transforms
    SET reduced_collection_name = $2,
        topics_collection_name = $3,
        updated_at = NOW()
    WHERE visualization_transform_id = $1
    RETURNING visualization_transform_id, title, embedded_dataset_id, owner, is_enabled,
              reduced_collection_name, topics_collection_name, visualization_config, created_at, updated_at
"#;

const DELETE_VISUALIZATION_TRANSFORM_QUERY: &str = r#"
    DELETE FROM visualization_transforms
    WHERE owner = $1 AND visualization_transform_id = $2
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
    let mut transform =
        sqlx::query_as::<_, VisualizationTransform>(CREATE_VISUALIZATION_TRANSFORM_QUERY)
            .bind(title)
            .bind(embedded_dataset_id)
            .bind(owner)
            .bind(visualization_config)
            .fetch_one(pool)
            .await?;

    // Generate and update Qdrant collection names with actual ID
    let (reduced_name, topics_name) = VisualizationTransform::generate_collection_names(
        transform.visualization_transform_id,
        owner,
    );

    transform = sqlx::query_as::<_, VisualizationTransform>(
        UPDATE_VISUALIZATION_TRANSFORM_COLLECTION_NAMES_QUERY,
    )
    .bind(transform.visualization_transform_id)
    .bind(&reduced_name)
    .bind(&topics_name)
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
