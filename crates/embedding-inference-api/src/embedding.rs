use arc_swap::ArcSwap;
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use futures::stream::StreamExt;
use once_cell::sync::OnceCell;
use ort::ep::CUDA;
use semantic_explorer_core::observability::{
    gpu_monitor, init_embedding_session_reset_metric, record_embedding_session_metrics,
    record_embedding_session_reset,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Notify, RwLock, Semaphore};
use tracing::{debug, error, info, warn};

use crate::config::ModelConfig;
use crate::errors::InferenceError;

// ============================================================================
// Model Memory Thresholds
// ============================================================================

/// Model size category for memory-based thresholds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSize {
    /// Small models (~90-150MB VRAM): all-MiniLM, bge-small
    Small,
    /// Medium models (~400-500MB VRAM): bge-base, mpnet-base
    Medium,
    /// Large models (~1.3-1.5GB VRAM): bge-large, e5-large
    Large,
    /// Unknown models - use conservative defaults
    Unknown,
}

impl ModelSize {
    /// Get approximate VRAM usage in MB for this model size category
    pub fn vram_mb(&self) -> usize {
        match self {
            ModelSize::Small => 120,
            ModelSize::Medium => 450,
            ModelSize::Large => 1400,
            ModelSize::Unknown => 500,
        }
    }

    /// Get request limit before session reset (inversely proportional to size)
    /// Larger models accumulate fragmentation faster, so reset more often
    /// Can be overridden by EMBEDDING_REQUEST_LIMIT env var for testing
    pub fn request_limit(&self) -> u64 {
        // Allow env override for testing
        if let Ok(limit) = std::env::var("EMBEDDING_REQUEST_LIMIT") {
            if let Ok(n) = limit.parse::<u64>() {
                return n;
            }
        }
        match self {
            ModelSize::Small => 2000,
            ModelSize::Medium => 1000,
            ModelSize::Large => 500,
            ModelSize::Unknown => 750,
        }
    }

    /// Get pre-warm threshold as percentage of request limit
    /// Start pre-warming standby when active reaches this percentage
    pub fn prewarm_threshold_percent(&self) -> f64 {
        match self {
            ModelSize::Small => 0.70,  // Start at 70% for small models
            ModelSize::Medium => 0.60, // Start at 60% for medium models
            ModelSize::Large => 0.50,  // Start at 50% for large models (more time needed)
            ModelSize::Unknown => 0.60,
        }
    }

    /// Get per-model concurrency limit
    /// Since the model mutex only allows one request at a time anyway,
    /// we set this to 1 to avoid blocking thread pool saturation.
    /// The semaphore still provides queueing for fairness.
    pub fn max_concurrent(&self) -> usize {
        // All models can only process one batch at a time due to model mutex
        1
    }
}

/// Get the model size category for a given model code
pub fn get_model_size(model_code: &str) -> ModelSize {
    let code_lower = model_code.to_lowercase();

    // Small models
    if code_lower.contains("minilm")
        || code_lower.contains("bge-small")
        || code_lower.contains("paraphrase-minilm")
        || code_lower.contains("gte-small")
    {
        return ModelSize::Small;
    }

    // Large models
    if code_lower.contains("bge-large")
        || code_lower.contains("e5-large")
        || code_lower.contains("gte-large")
        || code_lower.contains("instructor-large")
    {
        return ModelSize::Large;
    }

    // Medium models (base variants and others)
    if code_lower.contains("bge-base")
        || code_lower.contains("mpnet-base")
        || code_lower.contains("e5-base")
        || code_lower.contains("gte-base")
        || code_lower.contains("instructor-base")
        || code_lower.contains("bge-m3")
    {
        return ModelSize::Medium;
    }

    ModelSize::Unknown
}

// ============================================================================
// Double-Buffer Cache Structures
// ============================================================================

/// A single model slot containing the TextEmbedding and lifecycle metadata
struct ModelSlot {
    /// The actual embedding model
    model: std::sync::Mutex<TextEmbedding>,
    /// Number of requests processed by this slot
    request_count: AtomicU64,
    /// When this slot was created
    created_at: Instant,
    /// The EmbeddingModel enum for recreation
    embedding_model: EmbeddingModel,
}

impl ModelSlot {
    fn new(text_embedding: TextEmbedding, embedding_model: EmbeddingModel) -> Self {
        Self {
            model: std::sync::Mutex::new(text_embedding),
            request_count: AtomicU64::new(0),
            created_at: Instant::now(),
            embedding_model,
        }
    }

    fn increment_request_count(&self) -> u64 {
        self.request_count.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn get_request_count(&self) -> u64 {
        self.request_count.load(Ordering::Relaxed)
    }

    fn age_seconds(&self) -> f64 {
        self.created_at.elapsed().as_secs_f64()
    }
}

/// Double-buffer entry for a model with active and standby slots
struct DoubleBufferEntry {
    /// Currently active slot serving requests (ArcSwap for atomic swapping)
    active: ArcSwap<ModelSlot>,
    /// Pre-warmed standby slot ready for swap (None if not yet warmed)
    standby: Mutex<Option<Arc<ModelSlot>>>,
    /// Model size category for thresholds
    model_size: ModelSize,
    /// Per-model semaphore for concurrency control
    semaphore: Arc<Semaphore>,
    /// Notification for when standby becomes ready
    standby_ready: Notify,
    /// Flag indicating if pre-warming is in progress
    prewarming: AtomicU64, // 0 = not prewarming, 1 = prewarming
}

impl DoubleBufferEntry {
    fn new(
        text_embedding: TextEmbedding,
        embedding_model: EmbeddingModel,
        model_size: ModelSize,
    ) -> Self {
        let active = ArcSwap::from_pointee(ModelSlot::new(text_embedding, embedding_model));
        let semaphore = Arc::new(Semaphore::new(model_size.max_concurrent()));

        Self {
            active,
            standby: Mutex::new(None),
            model_size,
            semaphore,
            standby_ready: Notify::new(),
            prewarming: AtomicU64::new(0),
        }
    }

    /// Check if we should start pre-warming the standby slot
    fn should_prewarm(&self) -> bool {
        let current = self.active.load().get_request_count();
        let limit = self.model_size.request_limit();
        let threshold = (limit as f64 * self.model_size.prewarm_threshold_percent()) as u64;

        // Only prewarm if we haven't started yet
        current >= threshold && self.prewarming.load(Ordering::Relaxed) == 0
    }

    /// Check if we should swap to standby (threshold exceeded)
    fn should_swap(&self) -> bool {
        self.active.load().get_request_count() >= self.model_size.request_limit()
    }

    /// Mark pre-warming as started
    fn start_prewarm(&self) -> bool {
        self.prewarming
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
    }

    /// Reset pre-warming flag (after swap or failure)
    fn reset_prewarm(&self) {
        self.prewarming.store(0, Ordering::Relaxed);
    }
}

type EmbeddingCache = Arc<RwLock<HashMap<String, Arc<DoubleBufferEntry>>>>;

/// Global embedding model cache with double-buffering
static EMBEDDING_MODELS: OnceCell<EmbeddingCache> = OnceCell::new();

/// Queue timeout for acquiring semaphore permits (allows brief queuing before 503)
static SEMAPHORE_QUEUE_TIMEOUT: OnceCell<Duration> = OnceCell::new();

/// Mapping from model_code to EmbeddingModel enum
type ModelCodeToEnum = HashMap<String, EmbeddingModel>;

/// Global model code to enum mapping
static MODEL_CODE_MAP: OnceCell<ModelCodeToEnum> = OnceCell::new();

/// Global config for background tasks
static GLOBAL_CONFIG: OnceCell<ModelConfig> = OnceCell::new();

/// Cached GPU memory pressure state (updated by background poller)
static GPU_MEMORY_PRESSURE_HIGH: AtomicBool = AtomicBool::new(false);

/// GPU memory pressure threshold (percentage) for triggering reset
const GPU_MEMORY_PRESSURE_THRESHOLD: f64 = 80.0;

/// Metrics sampling rate (record metrics every N requests to reduce overhead)
const METRICS_SAMPLE_RATE: u64 = 10;

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the embedding semaphore queue timeout
/// Per-model semaphores are now used instead of global semaphore
pub fn init_semaphore(_max_concurrent: usize, queue_timeout_ms: u64) {
    let timeout = Duration::from_millis(queue_timeout_ms);

    SEMAPHORE_QUEUE_TIMEOUT.get_or_init(|| {
        info!(
            queue_timeout_ms = queue_timeout_ms,
            "Initialized per-model semaphore queue timeout"
        );
        timeout
    });
}

/// Get current available permits for a specific model (for monitoring/health checks)
pub async fn available_permits_for_model(model_id: &str) -> usize {
    let models = match EMBEDDING_MODELS.get() {
        Some(m) => m,
        None => return 0,
    };
    let cache = models.read().await;
    cache
        .get(model_id)
        .map(|e| e.semaphore.available_permits())
        .unwrap_or(0)
}

/// Get total available permits across all models (for monitoring/health checks)
pub async fn total_available_permits() -> usize {
    let models = match EMBEDDING_MODELS.get() {
        Some(m) => m,
        None => return 0,
    };
    let cache = models.read().await;
    cache
        .values()
        .map(|e| e.semaphore.available_permits())
        .sum()
}

/// Build the model code to enum mapping from FastEmbed's supported models
fn build_model_code_map() -> ModelCodeToEnum {
    TextEmbedding::list_supported_models()
        .iter()
        .map(|m| (m.model_code.clone(), m.model.clone()))
        .collect()
}

/// Resolve a model code string to a fastembed EmbeddingModel enum
fn resolve_embedding_model(model_code: &str) -> Result<EmbeddingModel, InferenceError> {
    MODEL_CODE_MAP
        .get_or_init(build_model_code_map)
        .get(model_code)
        .cloned()
        .ok_or_else(|| InferenceError::UnsupportedModel(format!("Unknown model: {}", model_code)))
}

/// Spawn background task that monitors GPU pressure and updates cached state
/// This avoids expensive NVML calls on the hot path
fn spawn_gpu_pressure_monitor() {
    tokio::spawn(async move {
        if !gpu_monitor::init() {
            warn!("GPU monitoring disabled - NVML not available");
            return;
        }

        info!(
            device_count = gpu_monitor::device_count(),
            threshold = GPU_MEMORY_PRESSURE_THRESHOLD,
            "Starting GPU pressure monitor"
        );

        let mut ticker = tokio::time::interval(Duration::from_secs(5));
        loop {
            ticker.tick().await;

            // Collect metrics (this updates Prometheus gauges)
            gpu_monitor::collect_metrics();

            // Update cached pressure state (checked on hot path)
            let is_high = gpu_monitor::is_memory_pressure_high(GPU_MEMORY_PRESSURE_THRESHOLD);
            GPU_MEMORY_PRESSURE_HIGH.store(is_high, Ordering::Relaxed);

            if is_high {
                warn!(
                    "GPU memory pressure is HIGH (>{}%)",
                    GPU_MEMORY_PRESSURE_THRESHOLD
                );
            }
        }
    });
}

/// Check cached GPU memory pressure (fast, no NVML call)
#[inline]
fn is_gpu_memory_pressure_high() -> bool {
    GPU_MEMORY_PRESSURE_HIGH.load(Ordering::Relaxed)
}

/// Initialize the embedding model cache and pre-load allowed models
pub async fn init_cache(config: &ModelConfig) {
    // Store config for background tasks
    let _ = GLOBAL_CONFIG.get_or_init(|| config.clone());

    let cache = EMBEDDING_MODELS.get_or_init(|| Arc::new(RwLock::new(HashMap::new())));

    // Initialize the model code map
    let _ = MODEL_CODE_MAP.get_or_init(build_model_code_map);

    // Start GPU monitoring with pressure callback
    spawn_gpu_pressure_monitor();

    // Get list of models to load
    let models_to_load = get_models_to_load(config);

    if models_to_load.is_empty() {
        info!("No embedding models to pre-load");
        return;
    }

    info!(
        models = ?models_to_load,
        count = models_to_load.len(),
        "Pre-loading embedding models with double-buffer support"
    );

    let concurrency_limit = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    info!("Using concurrency limit: {}", concurrency_limit);

    // Parallel loading
    let results = futures::stream::iter(models_to_load)
        .map(|model_id| {
            let config = config.clone();
            async move {
                let model_id_clone = model_id.clone();
                let res = tokio::task::spawn_blocking(move || {
                    match resolve_embedding_model(&model_id_clone) {
                        Ok(embedding_model) => {
                            match create_text_embedding(embedding_model.clone(), &config) {
                                Ok(text_embedding) => {
                                    let model_size = get_model_size(&model_id_clone);
                                    Ok((
                                        model_id_clone,
                                        text_embedding,
                                        embedding_model,
                                        model_size,
                                    ))
                                }
                                Err(e) => Err((model_id_clone, e)),
                            }
                        }
                        Err(e) => Err((model_id_clone, e)),
                    }
                })
                .await;

                match res {
                    Ok(inner_res) => inner_res,
                    Err(join_err) => {
                        Err((model_id, InferenceError::ModelLoad(join_err.to_string())))
                    }
                }
            }
        })
        .buffer_unordered(concurrency_limit)
        .collect::<Vec<_>>()
        .await;

    let mut cache_guard = cache.write().await;

    for result in results {
        match result {
            Ok((model_id, text_embedding, embedding_model, model_size)) => {
                let entry = Arc::new(DoubleBufferEntry::new(
                    text_embedding,
                    embedding_model,
                    model_size,
                ));
                cache_guard.insert(model_id.clone(), entry);

                // Initialize the reset counter so it shows in Prometheus immediately
                init_embedding_session_reset_metric(&model_id);

                info!(
                    model_id = %model_id,
                    model_size = ?model_size,
                    vram_mb = model_size.vram_mb(),
                    request_limit = model_size.request_limit(),
                    max_concurrent = model_size.max_concurrent(),
                    "Pre-loaded embedding model with double-buffer support"
                );
            }
            Err((model_id, e)) => {
                error!(
                    model_id = %model_id,
                    error = %e,
                    "Failed to load embedding model during initialization"
                );
            }
        }
    }

    info!(
        loaded_models = cache_guard.len(),
        "Embedding model cache initialization complete with double-buffer support"
    );
}

/// Get the list of embedding models to load based on configuration
fn get_models_to_load(config: &ModelConfig) -> Vec<String> {
    if config.all_embedding_models {
        TextEmbedding::list_supported_models()
            .iter()
            .map(|m| m.model_code.clone())
            .collect()
    } else {
        config.allowed_embedding_models.clone()
    }
}

/// Create a TextEmbedding instance with proper configuration
///
/// IMPORTANT: ONNX Runtime's CUDA execution provider can silently fall back to CPU
/// if CUDA initialization fails. This function now logs detailed information about
/// which execution provider is actually being used.
fn create_text_embedding(
    model: EmbeddingModel,
    config: &ModelConfig,
) -> Result<TextEmbedding, InferenceError> {
    let cuda_devices = std::env::var("CUDA_VISIBLE_DEVICES").ok();
    let use_cuda = cuda_devices.is_some();

    let mut options = if use_cuda {
        let devices = cuda_devices.as_deref().unwrap_or("0");
        info!(
            cuda_visible_devices = %devices,
            "Attempting to initialize CUDA execution provider for embeddings"
        );

        // Configure CUDA EP with error_on_failure() to detect if CUDA fails
        // Note: fastembed/ort may still fall back silently in some cases
        let cuda_provider = CUDA::default()
            .build()
            .error_on_failure(); // This makes registration return an error if CUDA fails

        TextInitOptions::new(model).with_execution_providers(vec![cuda_provider])
    } else {
        warn!(
            "CUDA_VISIBLE_DEVICES not set - using CPU execution provider. \
             Set CUDA_VISIBLE_DEVICES=0 in .env to enable GPU acceleration."
        );
        TextInitOptions::new(model)
    };

    if let Some(ref hf_home) = config.hf_home {
        options = options.with_cache_dir(hf_home.clone());
    }

    let start = std::time::Instant::now();
    let mut text_embedding = TextEmbedding::try_new(options).map_err(|e| {
        // This error might indicate CUDA EP failed to register
        if use_cuda {
            error!(
                error = %e,
                "Failed to initialize embedding model with CUDA. \
                 This may indicate a CUDA/cuDNN version mismatch or driver issue. \
                 Check: nvidia-smi, nvcc --version, and cuDNN installation."
            );
        } else {
            error!(error = %e, "Failed to initialize embedding model on CPU");
        }
        InferenceError::ModelLoad(e.to_string())
    })?;

    let init_time = start.elapsed();

    // Log the execution provider status
    // Fast init (<2s) typically indicates GPU, slow init (>5s) may indicate CPU fallback
    if use_cuda {
        if init_time.as_secs_f64() > 3.0 {
            warn!(
                init_time_secs = init_time.as_secs_f64(),
                "Model initialization took longer than expected. \
                 This MAY indicate CUDA EP failed silently and fell back to CPU. \
                 Expected GPU init: <2s, Got: {:.2}s",
                init_time.as_secs_f64()
            );
        } else {
            info!(
                init_time_secs = init_time.as_secs_f64(),
                "Model initialized with CUDA execution provider (init time suggests GPU is active)"
            );
        }
    } else {
        info!(
            init_time_secs = init_time.as_secs_f64(),
            "Model initialized with CPU execution provider"
        );
    }

    // Run a warm-up benchmark to definitively detect GPU vs CPU execution
    // GPU should process a small batch in <100ms, CPU takes 500ms+
    if use_cuda {
        // Create benchmark texts that match real workload: 500 chars each
        let sample_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. Curabitur pretium.".to_string();
        
        // Use 32 texts (1/8 of typical batch) for benchmark - enough to be representative
        let warmup_texts: Vec<String> = (0..32).map(|_| sample_text.clone()).collect();
        let bench_batch_size = 32;

        info!(
            text_count = warmup_texts.len(),
            chars_per_text = sample_text.len(),
            total_chars = sample_text.len() * warmup_texts.len(),
            "Running GPU benchmark with realistic text sizes"
        );

        // Warmup run (first inference has extra overhead)
        let warmup_start = std::time::Instant::now();
        if let Err(e) = text_embedding.embed(warmup_texts.clone(), Some(bench_batch_size)) {
            error!(error = %e, "Warmup embedding failed");
            return Err(InferenceError::ModelLoad(format!(
                "Warmup failed: {}",
                e
            )));
        }
        let warmup_time = warmup_start.elapsed();

        // Benchmark run (steady-state performance)
        let bench_start = std::time::Instant::now();
        if let Err(e) = text_embedding.embed(warmup_texts, Some(bench_batch_size)) {
            error!(error = %e, "Benchmark embedding failed");
            return Err(InferenceError::ModelLoad(format!(
                "Benchmark failed: {}",
                e
            )));
        }
        let bench_time = bench_start.elapsed();

        let bench_ms = bench_time.as_millis();
        let chars_per_sec = (sample_text.len() * bench_batch_size) as f64 / bench_time.as_secs_f64();

        // GPU threshold: 32 texts × 500 chars should complete in <500ms on GPU
        // CPU would take 2-5 seconds for this workload
        if bench_ms > 800 {
            error!(
                warmup_ms = warmup_time.as_millis(),
                bench_ms = bench_ms,
                chars_per_sec = chars_per_sec,
                "CUDA EXECUTION PROVIDER FALLBACK DETECTED! \
                 Benchmark took {}ms (expected <500ms for GPU). \
                 ONNX Runtime likely fell back to CPU silently. \
                 Try: 1) Restart the service, 2) Check nvidia-smi for GPU memory, \
                 3) Verify CUDA/cuDNN versions match ORT requirements.",
                bench_ms
            );
            // Return an error to prevent starting with slow CPU execution
            return Err(InferenceError::ModelLoad(format!(
                "GPU benchmark failed: {}ms (threshold: 800ms). \
                 CUDA EP likely fell back to CPU. Restart the service.",
                bench_ms
            )));
        } else {
            info!(
                warmup_ms = warmup_time.as_millis(),
                bench_ms = bench_ms,
                chars_per_sec = chars_per_sec,
                per_text_ms = bench_ms as f64 / bench_batch_size as f64,
                "GPU execution confirmed: benchmark passed"
            );
        }
    }

    Ok(text_embedding)
}

// ============================================================================
// Background Pre-warming
// ============================================================================

/// Spawn a background task to pre-warm a standby slot for a model
fn spawn_prewarm_task(model_id: String, entry: Arc<DoubleBufferEntry>) {
    let config = match GLOBAL_CONFIG.get() {
        Some(c) => c.clone(),
        None => {
            warn!(model_id = %model_id, "Cannot prewarm: config not available");
            entry.reset_prewarm();
            return;
        }
    };

    tokio::spawn(async move {
        info!(
            model_id = %model_id,
            "Starting background pre-warm for standby slot"
        );

        let embedding_model = entry.active.load().embedding_model.clone();

        // Create new TextEmbedding in blocking task
        let result = tokio::task::spawn_blocking(move || {
            create_text_embedding(embedding_model.clone(), &config).map(|te| (te, embedding_model))
        })
        .await;

        match result {
            Ok(Ok((text_embedding, embedding_model))) => {
                let new_slot = Arc::new(ModelSlot::new(text_embedding, embedding_model));

                // Install the standby slot
                {
                    let mut standby_guard = entry.standby.lock().await;
                    *standby_guard = Some(new_slot);
                }

                entry.standby_ready.notify_waiters();
                info!(
                    model_id = %model_id,
                    "Standby slot pre-warmed and ready for swap"
                );
            }
            Ok(Err(e)) => {
                error!(
                    model_id = %model_id,
                    error = %e,
                    "Failed to pre-warm standby slot"
                );
                entry.reset_prewarm();
            }
            Err(e) => {
                error!(
                    model_id = %model_id,
                    error = %e,
                    "Pre-warm task panicked"
                );
                entry.reset_prewarm();
            }
        }
    });
}

/// Try to swap active and standby slots, returning the new active slot
async fn try_swap_slots(model_id: &str, entry: &Arc<DoubleBufferEntry>) -> Option<Arc<ModelSlot>> {
    let mut standby_guard = entry.standby.lock().await;

    if let Some(new_active) = standby_guard.take() {
        // Get old slot info before swap for logging
        let old_slot = entry.active.load();
        let old_request_count = old_slot.get_request_count();
        let old_age_seconds = old_slot.age_seconds();

        // Actually swap the active slot! This is the critical atomic operation.
        entry.active.store(new_active.clone());

        // Record the reset
        record_embedding_session_reset(model_id, "request_threshold");

        info!(
            model_id = %model_id,
            old_request_count = old_request_count,
            old_age_seconds = old_age_seconds,
            "Swapped to fresh standby slot"
        );

        // Reset prewarm flag so next cycle can start
        entry.reset_prewarm();

        Some(new_active)
    } else {
        // Standby not ready yet
        debug!(
            model_id = %model_id,
            "Standby not ready for swap, continuing with current slot"
        );
        None
    }
}

// ============================================================================
// Embedding Generation
// ============================================================================

/// Generate embeddings using the double-buffer cache
pub async fn generate_embeddings(
    model_id: &str,
    config: &ModelConfig,
    texts: Vec<String>,
) -> Result<Vec<Vec<f32>>, InferenceError> {
    // Check if model is allowed
    if !config.is_embedding_model_allowed(model_id) {
        return Err(InferenceError::UnsupportedModel(format!(
            "Model {} is not in the allowed models list",
            model_id
        )));
    }

    let models = EMBEDDING_MODELS
        .get()
        .ok_or_else(|| InferenceError::Internal("Embedding cache not initialized".to_string()))?;

    // Get the model entry
    let entry = {
        let cache = models.read().await;
        cache.get(model_id).cloned().ok_or_else(|| {
            warn!(model_id = %model_id, "Model not found in preloaded cache");
            InferenceError::UnsupportedModel(format!(
                "Model {} not preloaded. Please check configuration.",
                model_id
            ))
        })?
    };

    // Acquire per-model permit with timeout
    let timeout = SEMAPHORE_QUEUE_TIMEOUT
        .get()
        .copied()
        .unwrap_or(Duration::from_millis(5000));

    let _permit = match tokio::time::timeout(timeout, entry.semaphore.clone().acquire_owned()).await
    {
        Ok(Ok(permit)) => permit,
        Ok(Err(_)) => {
            return Err(InferenceError::Internal("Semaphore closed".to_string()));
        }
        Err(_) => {
            warn!(
                model_id = %model_id,
                available_permits = entry.semaphore.available_permits(),
                "Model queue congested, returning 503"
            );
            return Err(InferenceError::ServiceUnavailable(format!(
                "Model {} queue congested, try again later",
                model_id
            )));
        }
    };

    // Get current active slot (fast path - ArcSwap load is very cheap)
    // Clone the Arc so we have an owned reference that can be moved to spawn_blocking
    let active_slot: Arc<ModelSlot> = Arc::clone(&entry.active.load());

    // Increment request count (single atomic op)
    let request_count = active_slot.increment_request_count();

    // Record session metrics only on sample (reduce overhead)
    if request_count % METRICS_SAMPLE_RATE == 0 {
        record_embedding_session_metrics(model_id, request_count, active_slot.age_seconds());
    }

    // Check lifecycle management only periodically (not every request)
    let slot_to_use: Arc<ModelSlot> = if request_count % 100 == 0 {
        // Check GPU memory pressure using cached state (no NVML call)
        if is_gpu_memory_pressure_high() && entry.should_swap() {
            let standby_guard = entry.standby.lock().await;
            if standby_guard.is_none() {
                warn!(
                    model_id = %model_id,
                    "GPU memory pressure high and no standby available, shedding load"
                );
                return Err(InferenceError::ServiceUnavailable(
                    "GPU memory pressure high, try again later".to_string(),
                ));
            }
        }

        // Check if we should start pre-warming
        if entry.should_prewarm() && entry.start_prewarm() {
            spawn_prewarm_task(model_id.to_string(), entry.clone());
        }

        // Check if we should swap (and standby is ready)
        if entry.should_swap() {
            if let Some(new_active) = try_swap_slots(model_id, &entry).await {
                new_active
            } else {
                active_slot
            }
        } else {
            active_slot
        }
    } else {
        active_slot
    };

    // Generate embeddings in a blocking task
    let texts_clone = texts.clone();
    let texts_count = texts_clone.len();
    // Estimate total characters (rough proxy for tokens)
    let total_chars: usize = texts_clone.iter().map(|t| t.len()).sum();
    let avg_chars = if texts_count > 0 {
        total_chars / texts_count
    } else {
        0
    };
    let batch_size = Some(config.max_batch_size);
    let model_id_owned = model_id.to_string();

    tokio::task::spawn_blocking(move || {
        let lock_start = std::time::Instant::now();
        let mut text_embedding = slot_to_use.model.lock().map_err(|e| {
            InferenceError::Internal(format!("Failed to acquire model lock: {}", e))
        })?;
        let lock_time = lock_start.elapsed();

        let embed_start = std::time::Instant::now();
        let result = text_embedding.embed(texts_clone, batch_size).map_err(|e| {
            error!(error = %e, "Embedding generation failed");
            InferenceError::Embedding(e.to_string())
        });
        let embed_time = embed_start.elapsed();

        // Log timing breakdown for debugging slow requests
        if embed_time.as_millis() > 500 {
            warn!(
                model_id = %model_id_owned,
                texts_count = texts_count,
                total_chars = total_chars,
                avg_chars_per_text = avg_chars,
                lock_time_ms = lock_time.as_millis(),
                embed_time_ms = embed_time.as_millis(),
                per_text_ms = embed_time.as_millis() as f64 / texts_count as f64,
                chars_per_sec = total_chars as f64 / embed_time.as_secs_f64(),
                "Slow embedding detected"
            );
        } else {
            debug!(
                model_id = %model_id_owned,
                texts_count = texts_count,
                total_chars = total_chars,
                avg_chars_per_text = avg_chars,
                lock_time_ms = lock_time.as_millis(),
                embed_time_ms = embed_time.as_millis(),
                per_text_ms = embed_time.as_millis() as f64 / texts_count as f64,
                chars_per_sec = total_chars as f64 / embed_time.as_secs_f64(),
                "Embedding timing"
            );
        }

        result
    })
    .await
    .map_err(|e| InferenceError::Internal(format!("Blocking task join error: {}", e)))?
}

/// Check if models are loaded and ready
pub fn is_ready() -> bool {
    EMBEDDING_MODELS.get().is_some()
}
