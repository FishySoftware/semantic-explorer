# Embedding Inference API

<div align="center">

![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)
![CUDA](https://img.shields.io/badge/CUDA-12.x_(optional)-76B900.svg)
![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)

**Local embedding and reranking server using FastEmbed with ONNX Runtime**

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
| `INFERENCE_MAX_BATCH_SIZE` | `256` | Maximum batch size per request |
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

- Automatic GPU detection
- TF32 enabled for Ampere+ GPUs (faster compute)
- Mixed precision (FP16) on compatible GPUs

**Supported CUDA Compute Capabilities**: 7.5, 8.0, 8.6, 8.9, 9.0

---

## Performance Tuning

### Understanding Throughput

The embedding API uses a semaphore-based backpressure system:

1. **Request arrives** → tries to acquire a permit (waits up to `QUEUE_TIMEOUT_MS`)
2. **Permit acquired** → request is processed by the model
3. **Timeout** → returns 503 with Retry-After header

**Key insight**: With GPU inference, the GPU can only process one batch at a time. Multiple concurrent requests are serialized at the model level. The semaphore prevents memory exhaustion from too many queued requests.

### Tuning Parameters

| Scenario | `MAX_CONCURRENT_REQUESTS` | `QUEUE_TIMEOUT_MS` | Notes |
|----------|---------------------------|--------------------| ------|
| Low-latency | 2-4 | 1000-2000 | Fail fast, client retries |
| High-throughput | 4-8 | 5000-10000 | Allow queuing |
| Batch processing | 8-16 | 30000 | Deep queue, maximize GPU utilization |

### Monitoring Performance

Watch these log patterns:
- `"Embedding service at capacity after queue timeout"` → Increase `MAX_CONCURRENT_REQUESTS` or `QUEUE_TIMEOUT_MS`
- Consistently slow batches (>2s for 256 items) → Check GPU utilization, consider smaller batch size
- Variable latency → Normal for GPU; first request may be slower (CUDA warmup)

### Batch Size Considerations

- **256** (default): Good for most models, saturates GPU compute
- **128**: Lower latency, useful for smaller models
- **512**: Higher throughput for larger models with sufficient VRAM

### Client-Side Best Practices

1. **Use batch endpoint** (`/api/embed/batch`) instead of single embed
2. **Respect Retry-After** header on 503 responses
3. **Implement exponential backoff** for transient failures
4. **Pre-batch texts** client-side to match `MAX_BATCH_SIZE`

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
