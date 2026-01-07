use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize, Debug)]
pub(crate) struct VisualizationPointsQuery {
    #[serde(default = "default_limit")]
    pub(crate) limit: u32,
    #[serde(default)]
    pub(crate) offset: Option<String>,
}

fn default_limit() -> u32 {
    1000
}

#[derive(Serialize, ToSchema)]
pub(crate) struct VisualizationPoint {
    pub(crate) id: String,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) z: f32,
    pub(crate) cluster_id: Option<i32>,
    pub(crate) topic_label: Option<String>,
    pub(crate) text: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct VisualizationPointsResponse {
    pub(crate) points: Vec<VisualizationPoint>,
    pub(crate) next_offset: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct VisualizationTopic {
    pub(crate) id: String,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) z: f32,
    pub(crate) cluster_id: i32,
    pub(crate) label: String,
    pub(crate) size: Option<i32>,
}
