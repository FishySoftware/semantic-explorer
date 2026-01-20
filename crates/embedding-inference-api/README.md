# Embedding Inference API

![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)
![CUDA](https://img.shields.io/badge/CUDA-optional-green.svg)

Local embedding and reranking server using FastEmbed with ONNX Runtime. Provides on-premise inference without external API calls.

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
    "model": "sentence-transformers/all-MiniLM-L6-v2"
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
| `INFERENCE_MAX_BATCH_SIZE` | `256` | Maximum batch size |
| `INFERENCE_MAX_CONCURRENT_REQUESTS` | `2` | Concurrent request limit |
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

Models from [FastEmbed](https://github.com/Anush008/fastembed-rs) are supported. Examples:

### Embedding Models

| Model | Dimensions | Notes |
|-------|------------|-------|
| `sentence-transformers/all-MiniLM-L6-v2` | 384 | General purpose |
| `BAAI/bge-small-en-v1.5` | 384 | Fast, lightweight |
| `BAAI/bge-base-en-v1.5` | 768 | Balanced |
| `BAAI/bge-large-en-v1.5` | 1024 | Higher quality |
| `BAAI/bge-m3` | 1024 | Multilingual (100+ languages) |

### Reranking Models

| Model | Notes |
|-------|-------|
| `jinaai/jina-reranker-v2-base-multilingual` | Multilingual |
| `BAAI/bge-reranker-base` | English |
| `BAAI/bge-reranker-large` | Higher quality |

---

## GPU Acceleration

CUDA support via ONNX Runtime:

- Automatic GPU detection (falls back to CPU)
- Mixed precision (FP16) on compatible GPUs

**Supported CUDA Compute Capabilities**: 7.5, 8.0, 8.6, 8.9, 9.0

---

## Building

### Local Development

For CPU-only builds:
```bash
# Debug build
cargo build -p embedding-inference-api

# Release build
cargo build -p embedding-inference-api --release
```

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
export INFERENCE_ALLOWED_EMBEDDING_MODELS="sentence-transformers/all-MiniLM-L6-v2"
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
