# Embedding Inference API

<div align="center">

![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)
![CUDA](https://img.shields.io/badge/CUDA-12.x_(optional)-76B900.svg)
![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)

**Local embedding and reranking server using FastEmbed with ONNX Runtime and Candle**

</div>

Provides on-premise inference without external API calls.

---

## Overview

Optional service for local embedding generation:

- **Privacy**: Data stays on-premise
- **Cost**: No per-token pricing
- **Latency**: No network round-trips
- **Air-gapped**: Works offline

---

## Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health/live` | Liveness probe |
| `GET` | `/health/ready` | Readiness probe |
| `GET` | `/api/embedders` | List available embedding models |
| `POST` | `/api/embed` | Generate embeddings |
| `POST` | `/api/embed/batch` | Batch embedding generation |
| `GET` | `/api/rerankers` | List available reranking models |
| `POST` | `/api/rerank` | Rerank documents |
| `GET` | `/swagger-ui` | Interactive API documentation |
| `GET` | `/metrics` | Prometheus metrics |

---

## API Examples

### Generate Embeddings

```bash
curl -X POST http://localhost:8090/api/embed \
  -H "Content-Type: application/json" \
  -d '{
    "input": ["hello world", "semantic search"],
    "model": "Qdrant/all-MiniLM-L6-v2-onnx"
  }'
```

### Rerank Documents

```bash
curl -X POST http://localhost:8090/api/rerank \
  -H "Content-Type: application/json" \
  -d '{
    "query": "machine learning",
    "documents": ["Deep learning is ML", "Weather is nice"],
    "model": "jinaai/jina-reranker-v2-base-multilingual",
    "top_n": 2
  }'
```

---

## Environment Variables

### Required

| Variable | Description |
|----------|-------------|
| `INFERENCE_ALLOWED_EMBEDDING_MODELS` | Comma-separated model list or `*` for all |

### Optional

| Variable | Default | Description |
|----------|---------|-------------|
| `INFERENCE_HOSTNAME` | `0.0.0.0` | Server bind address |
| `INFERENCE_PORT` | `8090` | Server port |
| `INFERENCE_ALLOWED_RERANK_MODELS` | - | Comma-separated reranker list or `*` |
| `INFERENCE_MAX_BATCH_SIZE` | `128` | Maximum batch size per request |
| `INFERENCE_MAX_CONCURRENT_REQUESTS` | `4` | Max concurrent embedding requests |
| `INFERENCE_QUEUE_TIMEOUT_MS` | `5000` | How long to queue requests before 503 |
| `INFERENCE_MODEL_PATH` | - | Custom ONNX model directory |
| `HF_HOME` | - | HuggingFace cache directory |
| `HF_ENDPOINT` | - | HuggingFace mirror URL (for air-gapped) |
| `CORS_ALLOWED_ORIGINS` | `*` | Comma-separated CORS origins |

### Observability

| Variable | Default | Description |
|----------|---------|-------------|
| `SERVICE_NAME` | `embedding-inference-api` | Service name for telemetry |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | OTLP collector endpoint |
| `LOG_FORMAT` | `json` | `json` or `pretty` |

### TLS (Optional)

| Variable | Default | Description |
|----------|---------|-------------|
| `SERVER_SSL_ENABLED` | `false` | Enable HTTPS |
| `TLS_SERVER_CERT_PATH` | - | Server certificate path |
| `TLS_SERVER_KEY_PATH` | - | Server private key path |

---

## Supported Models

Models from [FastEmbed](https://github.com/Anush008/fastembed-rs) are supported across two backends:

- **ONNX Runtime** — Traditional ONNX models with CUDA execution providers
- **Candle** — Native Rust transformer inference for Qwen3 models (SafeTensors weights, no ONNX)

### Model Filtering (ONNX)

The API automatically filters out certain ONNX model variants for GPU compatibility:

- **Quantized models** (models with `Q` suffix or `model_quantized.onnx` files)
- **Optimized models** (models with `model_optimized.onnx` files)
- **Chinese-specific models** (models containing `-zh-` in name)
- **Embedding Gemma** (`onnx-community/embeddinggemma-300m-ONNX`)

Only models with `onnx/model.onnx` file paths are included, ensuring GPU-friendly inference.

### Qwen3 Embedding Models (Candle Backend)

These models use the **Candle** transformer backend with SafeTensors weights. They support CUDA via Candle's native GPU kernels (not ONNX Runtime). Precision and context length are hardcoded per model.

| Model | Parameters | Dimensions | Context Length | Description |
|-------|------------|------------|---------------|-------------|
| `Qwen/Qwen3-Embedding-0.6B` | 0.6B | 1024 | 32768 | Lightweight Qwen3 embedding model, good balance of speed and quality |
| `Qwen/Qwen3-Embedding-4B` | 4B | 3584 | 32768 | Mid-size Qwen3 embedding model with strong multilingual performance |
| `Qwen/Qwen3-Embedding-8B` | 8B | 4096 | 32768 | Largest Qwen3 embedding model, highest quality embeddings |

### ONNX Embedding Models

| Model | Dimensions | Context Length | Modals | Description |
|-------|------------|---------------|-------------|-------------|
| `Qdrant/all-MiniLM-L6-v2-onnx` | 384 | 512 | Text | Sentence Transformer model, MiniLM-L6-v2 |
| `Xenova/all-MiniLM-L12-v2` | 384 | 512 | Text | Sentence Transformer model, MiniLM-L12-v2 |
| `Xenova/all-mpnet-base-v2` | 768 | 512 | Text | Sentence Transformer model, mpnet-base-v2 |
| `Xenova/bge-base-en-v1.5` | 768 | 512 | Text | v1.5 release of base English model |
| `Xenova/bge-large-en-v1.5` | 1024 | 512 | Text | v1.5 release of large English model |
| `Xenova/bge-small-en-v1.5` | 384 | 512 | Text | v1.5 release of fast and default English model |
| `nomic-ai/nomic-embed-text-v1` | 768 | 8192 | Text | 8192 context length text encoder |
| `nomic-ai/nomic-embed-text-v1.5` | 768 | 8192 | Text | v1.5 release of 8192 context length english model |
| `Xenova/paraphrase-multilingual-MiniLM-L12-v2` | 384 | 512 | Text | Multi-lingual model |
| `Xenova/paraphrase-multilingual-mpnet-base-v2` | 768 | 512 | Text | Sentence-transformers model for tasks like clustering or semantic search |
| `BAAI/bge-m3` | 1024 | 8192 | Text | Multilingual M3 model with 8192 context length, supports 100+ languages |
| `lightonai/modernbert-embed-large` | 1024 | 8192 | Text | Large model of ModernBert Text Embeddings |
| `intfloat/multilingual-e5-small` | 384 | 512 | Text | Small model of multilingual E5 Text Embeddings (100 languages) |
| `intfloat/multilingual-e5-base` | 768 | 512 | Text | Base model of multilingual E5 Text Embeddings (100 languages) |
| `Qdrant/multilingual-e5-large-onnx` | 1024 | 512 | Text | Large model of multilingual E5 Text Embeddings (100 languages) |
| `mixedbread-ai/mxbai-embed-large-v1` | 1024 | 512 | Text | Large English embedding model from MixedBreed.ai |
| `Alibaba-NLP/gte-base-en-v1.5` | 768 | 8192 | Text | Large multilingual embedding model from Alibaba |
| `Alibaba-NLP/gte-large-en-v1.5` | 1024 | 8192 | Text | Large multilingual embedding model from Alibaba |
| `Qdrant/clip-ViT-B-32-text` | 512 | 77 | Text, Image | CLIP text encoder based on ViT-B/32 (pairs with clip-ViT-B-32-vision for image-to-text search) |
| `jinaai/jina-embeddings-v2-base-code` | 768 | 8192 | Text | Jina embeddings v2 base code |
| `jinaai/jina-embeddings-v2-base-en` | 768 | 8192 | Text | Jina embeddings v2 base English |
| `snowflake/snowflake-arctic-embed-xs` | 384 | 2048 | Text | Snowflake Arctic embed model, xs |
| `snowflake/snowflake-arctic-embed-s` | 384 | 2048 | Text | Snowflake Arctic embed model, small |
| `Snowflake/snowflake-arctic-embed-m` | 768 | 2048 | Text | Snowflake Arctic embed model, medium |
| `snowflake/snowflake-arctic-embed-m-long` | 768 | 2048 | Text | Snowflake Arctic embed model, medium with 2048 context |
| `snowflake/snowflake-arctic-embed-l` | 1024 | 2048 | Text | Snowflake Arctic embed model, large |

### Reranking Models

| Model | Parameters | Context Length | Languages | Description |
|-------|------------|---------------|-----------|-------------|
| `BAAI/bge-reranker-base` | 278M | 512 | English, Chinese | Cross-encoder model optimized for reranking tasks, built on RetroMAE architecture |
| `rozgo/bge-reranker-v2-m3` | 568M | 8192 | Multilingual (100+) | Lightweight reranker model with strong multilingual capabilities, supports long documents |
| `jinaai/jina-reranker-v1-turbo-en` | 37.8M | 8192 | English | Fast reranker model with 6-layer architecture, optimized for speed and accuracy |
| `jinaai/jina-reranker-v2-base-multilingual` | 278M | 1024 | Multilingual (100+) | Listwise reranker for agentic RAG with function-calling and code search capabilities |

---

## GPU Acceleration

### ONNX Runtime (ONNX Models)

- Automatic GPU detection
- TF32 enabled for Ampere+ GPUs (faster compute)
- Mixed precision (FP16) on compatible GPUs
- CUDNN Flash Attention for improved performance
- Configurable CUDA memory arena (`CUDA_ARENA_SIZE`)

### Candle (Qwen3 Models)

- Native CUDA kernels via Candle with cuDNN support
- Configurable weight precision: `bf16` (default), `f16`, or `f32`
- SafeTensors weight loading (no ONNX conversion needed)
- Automatic CPU fallback when CUDA is unavailable

**Supported CUDA Compute Capabilities**: 7.5, 8.0, 8.6, 8.9, 9.0

---

## Building

### Local Development

For CUDA-accelerated builds (requires NVIDIA GPU):
```bash
# One-time setup (from repository root)
./setup_cuda.sh

# Build with CUDA support
./cargo_cuda.sh build -p embedding-inference-api --release
```

See the [root README](../../README.md#building-from-source) for more details on CUDA builds.

### Docker (with CUDA)

```bash
docker build -f crates/embedding-inference-api/Dockerfile -t embedding-inference-api:latest .
```

---

## Running

```bash
# ONNX model only
export INFERENCE_ALLOWED_EMBEDDING_MODELS="Qdrant/all-MiniLM-L6-v2-onnx"
cargo run -p embedding-inference-api

# Mix of ONNX and Qwen3 models
export INFERENCE_ALLOWED_EMBEDDING_MODELS="Qdrant/all-MiniLM-L6-v2-onnx,Qwen/Qwen3-Embedding-0.6B"
export QWEN3_DTYPE=bf16
cargo run -p embedding-inference-api
```

### With GPU (Local Development)

```bash
# After running setup_cuda.sh once (from repository root)
export CUDA_VISIBLE_DEVICES=0
./cargo_cuda.sh run -p embedding-inference-api
```

---

## Health Checks

```bash
curl http://localhost:8090/health/live
curl http://localhost:8090/health/ready
```

---

## License

Apache License 2.0
