//! Pre-download HuggingFace models using `hf_hub::api::sync::ApiBuilder::from_env()`.
//!
//! ## Why this module exists
//!
//! mistral.rs uses `hf_hub::ApiBuilder::from_cache()` internally (via the
//! `get_paths!` macro), which **hardcodes** the endpoint to `https://huggingface.co`
//! — ignoring `HF_ENDPOINT`.  Only `ApiBuilder::from_env()` reads `HF_ENDPOINT`.
//!
//! In air-gapped deployments behind an Artifactory/Nexus HF mirror, models cannot
//! be fetched from the public internet.  This module pre-downloads every file into
//! the standard `$HF_HOME/hub/` cache **before** mistral.rs tries to load them.
//! When mistral.rs later calls `ApiBuilder::from_cache()`, the files are already
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
//! If `HF_ENDPOINT` is not set, this module is a no-op — models are expected to
//! already exist on disk or be downloadable from the default public endpoint.

use anyhow::{Context, Result};
use hf_hub::api::sync::ApiBuilder;
use tracing::{info, warn};

use crate::config::ModelConfig;
use crate::llm::parse_gguf_model_id;

/// Pre-download model files for all allowed models using `HF_ENDPOINT`.
///
/// This must be called **before** `llm::init_cache()`.
///
/// # Model formats
///
/// | Format | Strategy |
/// |---|---|
/// | GGUF (`repo:file.gguf[@tok]`) | Download only the specific `.gguf` file + tokenizer files |
/// | Regular HF | `repo.info()` to discover files, then `repo.get()` each |
pub fn preload_models(config: &ModelConfig) -> Result<()> {
    let Some(ref _endpoint) = config.hf_endpoint else {
        info!(
            "HF_ENDPOINT not set — skipping model preload (using default endpoint or local cache)"
        );
        return Ok(());
    };

    info!(
        hf_endpoint = %_endpoint,
        "HF_ENDPOINT is set — pre-downloading LLM models that would otherwise ignore it"
    );

    // Build API that reads HF_ENDPOINT + HF_HOME + HF_TOKEN from env
    let api = ApiBuilder::from_env()
        .with_progress(true)
        .build()
        .context("Failed to build hf-hub API from environment")?;

    for model_id in &config.allowed_models {
        if is_gguf_model(model_id) {
            preload_gguf_model(&api, model_id);
        } else {
            preload_hf_model(&api, model_id);
        }
    }

    info!("LLM model preload complete");
    Ok(())
}

/// Check whether a model ID refers to a GGUF model.
fn is_gguf_model(model_id: &str) -> bool {
    let lower = model_id.to_lowercase();
    lower.contains("-gguf") || lower.ends_with(".gguf")
}

/// Pre-download a GGUF model.
///
/// Only downloads the specific `.gguf` file (not all variants in the repo)
/// plus tokenizer files from the tokenizer repo.
fn preload_gguf_model(api: &hf_hub::api::sync::Api, model_id: &str) {
    let (repo_id, gguf_filename, tokenizer_repo) = match parse_gguf_model_id(model_id) {
        Ok(parsed) => parsed,
        Err(e) => {
            warn!(
                model_id = %model_id,
                error = %e,
                "Failed to parse GGUF model ID — skipping preload"
            );
            return;
        }
    };

    info!(
        model_id = %model_id,
        repo = %repo_id,
        file = %gguf_filename,
        tokenizer_repo = %tokenizer_repo,
        "Pre-downloading GGUF model"
    );

    // Download the GGUF weights file
    let repo = api.model(repo_id.clone());
    match repo.get(&gguf_filename) {
        Ok(path) => {
            info!(
                model_id = %model_id,
                file = %gguf_filename,
                path = %path.display(),
                "Cached GGUF weights"
            );
        }
        Err(e) => {
            warn!(
                model_id = %model_id,
                file = %gguf_filename,
                error = %e,
                "Failed to download GGUF weights — downstream loader may fail"
            );
        }
    }

    // Download tokenizer files from the tokenizer repo
    let tokenizer_files = [
        "tokenizer.json",
        "tokenizer_config.json",
        "special_tokens_map.json",
        "generation_config.json",
    ];

    let tok_repo = api.model(tokenizer_repo.clone());
    for filename in &tokenizer_files {
        match tok_repo.get(filename) {
            Ok(path) => {
                info!(
                    model_id = %model_id,
                    tokenizer_repo = %tokenizer_repo,
                    file = %filename,
                    path = %path.display(),
                    "Cached tokenizer file"
                );
            }
            Err(e) => {
                // Some tokenizer files are optional (e.g. generation_config.json)
                warn!(
                    model_id = %model_id,
                    tokenizer_repo = %tokenizer_repo,
                    file = %filename,
                    error = %e,
                    "Failed to download tokenizer file (may be optional)"
                );
            }
        }
    }
}

/// Pre-download a regular HuggingFace model (safetensors, config, tokenizer).
///
/// Uses `repo.info()` to discover all files, then downloads each.
fn preload_hf_model(api: &hf_hub::api::sync::Api, model_id: &str) {
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
        "Pre-downloading HF model files"
    );

    for sibling in &info.siblings {
        let filename = &sibling.rfilename;

        // Skip non-essential files and alternative weight formats
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

/// Skip files that are not needed for LLM inference.
fn should_skip_file(filename: &str) -> bool {
    let dominated = [
        ".gitattributes",
        "README.md",
        "LICENSE",
        "NOTICE",
        ".git/",
        "flax_model",        // Flax/JAX weights
        "tf_model",          // TensorFlow weights
        "pytorch_model.bin", // Legacy PyTorch format; safetensors is preferred
        "openvino/",         // OpenVINO IR
        "onnx/",             // ONNX format — LLM service uses safetensors/GGUF
        ".gguf",             // Don't download ALL GGUF variants — only needed for GGUF path
    ];
    dominated.iter().any(|skip| filename.contains(skip))
}
