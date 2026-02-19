//! Pre-download HuggingFace models using `hf_hub::api::sync::ApiBuilder::from_env()`.
//!
//! ## Why this module exists
//!
//! fastembed's `TextRerank::try_new()` and `Qwen3TextEmbedding::from_hf()` use
//! `hf_hub::ApiBuilder::new()` or `::from_cache()` internally, both of which
//! **hardcode** the endpoint to `https://huggingface.co` — ignoring `HF_ENDPOINT`.
//! Only `ApiBuilder::from_env()` reads `HF_ENDPOINT`.
//!
//! In air-gapped deployments behind an Artifactory/Nexus HF mirror, models cannot
//! be fetched from the public internet.  This module pre-downloads every file into
//! the standard `$HF_HOME/hub/` cache **before** fastembed tries to load them.
//! When fastembed later calls `ApiBuilder::from_cache()`, the files are already
//! present and no network request is made.
//!
//! ## CA certificates
//!
//! The companion `ureq = { version = "2", features = ["native-certs"] }` direct
//! dependency activates Cargo feature unification, replacing ureq's default
//! `webpki-roots` with `rustls-native-certs`.  This means `SSL_CERT_FILE` (or the
//! system CA store) is respected — essential when the mirror uses a private CA.
//!
//! ## Skip behaviour
//!
//! If neither `HF_ENDPOINT` nor `HF_TOKEN` is set, this module is a no-op —
//! models are expected to already exist on disk or be downloadable from the
//! default public endpoint without authentication.

use anyhow::{Context, Result};
use hf_hub::api::sync::{Api, ApiBuilder};
use tracing::{info, warn};

use crate::config::ModelConfig;
use crate::embedding::{self, is_qwen3_model};
use crate::reranker;

/// Pre-download model files for models that would otherwise ignore `HF_ENDPOINT`.
///
/// This must be called **before** `embedding::init_cache()` and `reranker::init_cache()`.
///
/// # Preloaded model categories
///
/// | Category | Why preloaded |
/// |---|---|
/// | Reranker (ONNX) | `TextRerank::try_new()` → `ApiBuilder::from_cache()` |
/// | Qwen3 (candle) | `Qwen3TextEmbedding::from_hf()` → `ApiBuilder::new()` |
///
/// Standard ONNX embedding models are **not** preloaded — fastembed's
/// `pull_from_hf()` already reads `HF_ENDPOINT` correctly.
pub fn preload_models(config: &ModelConfig) -> Result<()> {
    let has_endpoint = config.hf_endpoint.is_some();
    let has_token = config.hf_token.is_some();

    if !has_endpoint && !has_token {
        info!(
            "Neither HF_ENDPOINT nor HF_TOKEN set — skipping model preload (using default endpoint or local cache)"
        );
        return Ok(());
    }

    info!(
        hf_endpoint = config.hf_endpoint.as_deref().unwrap_or("<default>"),
        hf_token_set = has_token,
        "Pre-downloading models (HF_ENDPOINT and/or HF_TOKEN detected)"
    );

    // Build API that reads HF_ENDPOINT + HF_HOME + HF_TOKEN from env
    let api = ApiBuilder::from_env()
        .with_progress(true)
        .build()
        .context("Failed to build hf-hub API from environment")?;

    // ── Reranker models ─────────────────────────────────────────────────
    let rerank_models = reranker::get_models_to_load(config);
    if !rerank_models.is_empty() {
        info!(
            models = ?rerank_models,
            count = rerank_models.len(),
            "Pre-downloading reranker models"
        );
        for model_id in &rerank_models {
            preload_repo(&api, model_id);
        }
    }

    // ── Qwen3 embedding models ──────────────────────────────────────────
    let qwen3_models = get_qwen3_models_to_preload(config);
    if !qwen3_models.is_empty() {
        info!(
            models = ?qwen3_models,
            count = qwen3_models.len(),
            "Pre-downloading Qwen3 embedding models"
        );
        for model_id in &qwen3_models {
            preload_repo(&api, model_id);
        }
    }

    info!("Model preload complete");
    Ok(())
}

/// Collect Qwen3 model IDs that need preloading based on the allowed model config.
fn get_qwen3_models_to_preload(config: &ModelConfig) -> Vec<String> {
    if config.all_embedding_models {
        // All Qwen3 models
        embedding::get_all_available_embedding_models(config)
            .into_iter()
            .filter(|m| is_qwen3_model(&m.model_code))
            .map(|m| m.model_code)
            .collect()
    } else {
        config
            .allowed_embedding_models
            .iter()
            .filter(|id| is_qwen3_model(id))
            .cloned()
            .collect()
    }
}

/// Download all files in a HuggingFace model repository.
///
/// Uses `repo.info()` to discover files, then `repo.get()` for each.
/// `repo.get()` is a cache-aware no-op if the file is already present.
/// Errors are logged as warnings — startup continues so the downstream
/// loader can produce the definitive error message.
fn preload_repo(api: &Api, model_id: &str) {
    let repo = api.model(model_id.to_string());

    let info = match repo.info() {
        Ok(info) => info,
        Err(e) => {
            warn!(
                model_id = %model_id,
                error = %e,
                "Failed to fetch repo info — skipping preload for this model"
            );
            return;
        }
    };

    info!(
        model_id = %model_id,
        file_count = info.siblings.len(),
        "Downloading model files"
    );

    for sibling in &info.siblings {
        let filename = &sibling.rfilename;

        // Skip non-essential files to save bandwidth/time
        if should_skip_file(filename) {
            continue;
        }

        match repo.get(filename) {
            Ok(path) => {
                info!(
                    model_id = %model_id,
                    file = %filename,
                    path = %path.display(),
                    "Cached model file"
                );
            }
            Err(e) => {
                warn!(
                    model_id = %model_id,
                    file = %filename,
                    error = %e,
                    "Failed to download model file — downstream loader may fail"
                );
            }
        }
    }
}

/// Skip files that are not needed for inference.
fn should_skip_file(filename: &str) -> bool {
    let dominated = [
        ".gitattributes",
        "README.md",
        "LICENSE",
        "NOTICE",
        ".git/",
        "flax_model",        // Flax/JAX weights — not used by ONNX or candle
        "tf_model",          // TensorFlow weights
        "pytorch_model.bin", // Legacy PyTorch format; safetensors is preferred
        "openvino/",         // OpenVINO IR
    ];
    dominated.iter().any(|skip| filename.contains(skip))
}
