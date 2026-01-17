## Overview
The inference service fronts fastembed/ONNX models for embeddings and reranking. The API surface is clean, but the current cache implementation serializes all requests and assumes CUDA is always available.

## High
- **Global mutex holds the model lock during inference** – `generate_embeddings` locks the global `HashMap` of models, loads the model, and then calls `TextEmbedding::embed` while still holding the mutex guard. See [crates/inference-api/src/embedding.rs#L119-L146](crates/inference-api/src/embedding.rs#L119-L146). Long GPU ops therefore block every other caller, even for different models. Extract the model out of the map (e.g., store `Arc<tokio::sync::Mutex<...>>` per model) so concurrent requests can progress.
- **Same bottleneck exists for rerankers** – `rerank_documents` follows the identical pattern, locking the entire cache while running `TextRerank::rerank`. See [crates/inference-api/src/reranker.rs#L78-L103](crates/inference-api/src/reranker.rs#L78-L103). Address both caches together.

## Medium
- **CUDA execution provider is mandatory with no fallback** – Both `create_text_embedding` and `create_text_rerank` immediately call `CUDA::default().build()` and pass it to fastembed. See [crates/inference-api/src/embedding.rs#L90-L103](crates/inference-api/src/embedding.rs#L90-L103) and [crates/inference-api/src/reranker.rs#L46-L60](crates/inference-api/src/reranker.rs#L46-L60). On CPU-only nodes, this panics before serving traffic. Probe `CUDA_VISIBLE_DEVICES` or config flags and add a CPU execution provider fallback.
- **Cache initialization is lazy with no health gating** – `init_cache` only seeds empty `HashMap`s. `is_ready` returns `false` until a model was loaded once, which means readiness checks will fail forever unless a warmup request hits the service. See [crates/inference-api/src/embedding.rs#L148-L155](crates/inference-api/src/embedding.rs#L148-L155) and [crates/inference-api/src/reranker.rs#L106-L113](crates/inference-api/src/reranker.rs#L106-L113). Either preload allowed models on startup or adjust readiness semantics.

## Low
- **CORS configuration lacks credentials handling** – When `cors_allowed_origins` is set, the builder never calls `supports_credentials()`. See [crates/inference-api/src/main.rs#L82-L118](crates/inference-api/src/main.rs#L82-L118). If the UI makes authenticated requests, add `supports_credentials()` in the restricted branch for parity with the wildcard path.
