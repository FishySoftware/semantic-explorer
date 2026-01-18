//! Model discovery and listing service.
//!
//! Provides model information and listing functionality.

use fastembed::{EmbeddingModel, RerankerModel, TextEmbedding, TextRerank};
use serde::Serialize;
use utoipa::ToSchema;

use crate::config::ModelConfig;

/// Information about an available model
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ModelInfo {
    /// Model identifier (HuggingFace repo format)
    pub id: String,
    /// Human-readable model name
    pub name: String,
    /// Model description
    pub description: String,
    /// Model type (embedding or reranker)
    pub model_type: String,
    /// Output dimensions (for embeddings)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,
}

/// Get information about available embedding models
pub fn get_embedding_models(config: &ModelConfig) -> Vec<ModelInfo> {
    let supported_models = TextEmbedding::list_supported_models();

    let mut models: Vec<ModelInfo> = supported_models
        .iter()
        // .filter(|model| model.model_file == "onnx/model.onnx") // Filter out quantized models.
        .filter_map(|m| {
            let model_id = model_enum_to_id(&m.model);

            // Filter by allowed models if configured
            if !config.is_embedding_model_allowed(&model_id) {
                return None;
            }

            Some(ModelInfo {
                id: model_id,
                name: m.model_code.clone(),
                description: m.description.clone(),
                model_type: "embedding".to_string(),
                dimensions: Some(m.dim),
            })
        })
        .collect();

    // Sort by name
    models.sort_by(|a, b| a.name.cmp(&b.name));

    models
}

/// Get information about available reranker models
pub fn get_reranker_models(config: &ModelConfig) -> Vec<ModelInfo> {
    let supported_models = TextRerank::list_supported_models();

    let mut models: Vec<ModelInfo> = supported_models
        .iter()
        .filter_map(|m| {
            let model_id = reranker_enum_to_id(&m.model);

            // Filter by allowed models if configured
            if !config.is_rerank_model_allowed(&model_id) {
                return None;
            }

            Some(ModelInfo {
                id: model_id,
                name: m.model_code.clone(),
                description: m.description.clone(),
                model_type: "reranker".to_string(),
                dimensions: None,
            })
        })
        .collect();

    // Sort by name
    models.sort_by(|a, b| a.name.cmp(&b.name));

    models
}

/// Convert EmbeddingModel enum to HuggingFace-style model ID
fn model_enum_to_id(model: &EmbeddingModel) -> String {
    match model {
        // Sorted alphabetically by model ID
        EmbeddingModel::GTEBaseENV15 => "Alibaba-NLP/gte-base-en-v1.5".to_string(),
        EmbeddingModel::GTEBaseENV15Q => "Alibaba-NLP/gte-base-en-v1.5".to_string(),
        EmbeddingModel::GTELargeENV15 => "Alibaba-NLP/gte-large-en-v1.5".to_string(),
        EmbeddingModel::GTELargeENV15Q => "Alibaba-NLP/gte-large-en-v1.5".to_string(),
        EmbeddingModel::BGEBaseENV15 => "BAAI/bge-base-en-v1.5".to_string(),
        EmbeddingModel::BGEBaseENV15Q => "BAAI/bge-base-en-v1.5".to_string(),
        EmbeddingModel::BGELargeENV15 => "BAAI/bge-large-en-v1.5".to_string(),
        EmbeddingModel::BGELargeENV15Q => "BAAI/bge-large-en-v1.5".to_string(),
        EmbeddingModel::BGELargeZHV15 => "BAAI/bge-large-zh-v1.5".to_string(),
        EmbeddingModel::BGEM3 => "BAAI/bge-m3".to_string(),
        EmbeddingModel::BGESmallENV15 => "BAAI/bge-small-en-v1.5".to_string(),
        EmbeddingModel::BGESmallENV15Q => "BAAI/bge-small-en-v1.5".to_string(),
        EmbeddingModel::BGESmallZHV15 => "BAAI/bge-small-zh-v1.5".to_string(),
        EmbeddingModel::MultilingualE5Base => "intfloat/multilingual-e5-base".to_string(),
        EmbeddingModel::MultilingualE5Large => "intfloat/multilingual-e5-large".to_string(),
        EmbeddingModel::MultilingualE5Small => "intfloat/multilingual-e5-small".to_string(),
        EmbeddingModel::JinaEmbeddingsV2BaseCode => {
            "jinaai/jina-embeddings-v2-base-code".to_string()
        }
        EmbeddingModel::JinaEmbeddingsV2BaseEN => "jinaai/jina-embeddings-v2-base-en".to_string(),
        EmbeddingModel::ModernBertEmbedLarge => "lightonai/modernbert-embed-large".to_string(),
        EmbeddingModel::MxbaiEmbedLargeV1 => "mixedbread-ai/mxbai-embed-large-v1".to_string(),
        EmbeddingModel::MxbaiEmbedLargeV1Q => "mixedbread-ai/mxbai-embed-large-v1".to_string(),
        EmbeddingModel::NomicEmbedTextV1 => "nomic-ai/nomic-embed-text-v1".to_string(),
        EmbeddingModel::NomicEmbedTextV15 => "nomic-ai/nomic-embed-text-v1.5".to_string(),
        EmbeddingModel::NomicEmbedTextV15Q => "nomic-ai/nomic-embed-text-v1.5".to_string(),
        EmbeddingModel::EmbeddingGemma300M => "onnx-community/embeddinggemma-300m-ONNX".to_string(),
        EmbeddingModel::ClipVitB32 => "Qdrant/clip-ViT-B-32-text".to_string(),
        EmbeddingModel::AllMiniLML12V2 => "sentence-transformers/all-MiniLM-L12-v2".to_string(),
        EmbeddingModel::AllMiniLML12V2Q => "sentence-transformers/all-MiniLM-L12-v2".to_string(),
        EmbeddingModel::AllMiniLML6V2 => "sentence-transformers/all-MiniLM-L6-v2".to_string(),
        EmbeddingModel::AllMiniLML6V2Q => "sentence-transformers/all-MiniLM-L6-v2".to_string(),
        EmbeddingModel::AllMpnetBaseV2 => "sentence-transformers/all-mpnet-base-v2".to_string(),
        EmbeddingModel::ParaphraseMLMiniLML12V2 => {
            "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2".to_string()
        }
        EmbeddingModel::ParaphraseMLMiniLML12V2Q => {
            "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2".to_string()
        }
        EmbeddingModel::ParaphraseMLMpnetBaseV2 => {
            "sentence-transformers/paraphrase-multilingual-mpnet-base-v2".to_string()
        }
        EmbeddingModel::SnowflakeArcticEmbedL => "snowflake/snowflake-arctic-embed-l".to_string(),
        EmbeddingModel::SnowflakeArcticEmbedLQ => "snowflake/snowflake-arctic-embed-l".to_string(),
        EmbeddingModel::SnowflakeArcticEmbedM => "snowflake/snowflake-arctic-embed-m".to_string(),
        EmbeddingModel::SnowflakeArcticEmbedMLong => {
            "snowflake/snowflake-arctic-embed-m-long".to_string()
        }
        EmbeddingModel::SnowflakeArcticEmbedMLongQ => {
            "snowflake/snowflake-arctic-embed-m-long".to_string()
        }
        EmbeddingModel::SnowflakeArcticEmbedMQ => "snowflake/snowflake-arctic-embed-m".to_string(),
        EmbeddingModel::SnowflakeArcticEmbedS => "snowflake/snowflake-arctic-embed-s".to_string(),
        EmbeddingModel::SnowflakeArcticEmbedSQ => "snowflake/snowflake-arctic-embed-s".to_string(),
        EmbeddingModel::SnowflakeArcticEmbedXS => "snowflake/snowflake-arctic-embed-xs".to_string(),
        EmbeddingModel::SnowflakeArcticEmbedXSQ => {
            "snowflake/snowflake-arctic-embed-xs".to_string()
        }
    }
}

/// Convert RerankerModel enum to HuggingFace-style model ID
fn reranker_enum_to_id(model: &RerankerModel) -> String {
    match model {
        // BGE Rerankers
        RerankerModel::BGERerankerBase => "BAAI/bge-reranker-base".to_string(),
        RerankerModel::BGERerankerV2M3 => "rozgo/bge-reranker-v2-m3".to_string(),

        // Jina Rerankers
        RerankerModel::JINARerankerV1TurboEn => "jinaai/jina-reranker-v1-turbo-en".to_string(),
        RerankerModel::JINARerankerV2BaseMultiligual => {
            "jinaai/jina-reranker-v2-base-multilingual".to_string()
        }
    }
}
