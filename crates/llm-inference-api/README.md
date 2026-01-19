# LLM Inference API

![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)
![CUDA](https://img.shields.io/badge/CUDA-optional-green.svg)

Local LLM inference server using [mistral.rs](https://github.com/EricLBuehler/mistral.rs). Provides on-premise text generation without external API calls.

---

## Overview

Optional service for local LLM inference:

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
| `GET` | `/api/llms` | List available models |
| `POST` | `/api/generate` | Text generation |
| `POST` | `/api/generate/stream` | Streaming text generation (SSE) |
| `POST` | `/api/chat` | Chat completion |
| `POST` | `/api/chat/stream` | Streaming chat completion (SSE) |
| `GET` | `/swagger-ui` | Interactive API documentation |
| `GET` | `/metrics` | Prometheus metrics |

---

## API Examples

### Text Generation

```bash
curl -X POST http://localhost:8091/api/generate \
  -H "Content-Type: application/json" \
  -d '{
    "prompt": "Explain machine learning in simple terms.",
    "model": "TinyLlama/TinyLlama-1.1B-Chat-v1.0",
    "max_tokens": 256,
    "temperature": 0.7
  }'
```

### Streaming Generation

```bash
curl -X POST http://localhost:8091/api/generate/stream \
  -H "Content-Type: application/json" \
  -d '{
    "prompt": "Write a haiku about coding.",
    "model": "TinyLlama/TinyLlama-1.1B-Chat-v1.0"
  }'
```

### Chat Completion

```bash
curl -X POST http://localhost:8091/api/chat \
  -H "Content-Type: application/json" \
  -d '{
    "model": "TinyLlama/TinyLlama-1.1B-Chat-v1.0",
    "messages": [
      {"role": "user", "content": "Hello!"}
    ]
  }'
```

---

## Environment Variables

### Required

| Variable | Description |
|----------|-------------|
| `LLM_ALLOWED_MODELS` | Comma-separated model list or `*` for all |

### Optional - Server

| Variable | Default | Description |
|----------|---------|-------------|
| `LLM_INFERENCE_HOSTNAME` | `0.0.0.0` | Server bind address |
| `LLM_INFERENCE_PORT` | `8091` | Server port |
| `CORS_ALLOWED_ORIGINS` | `*` | Comma-separated CORS origins |

### Optional - Generation

| Variable | Default | Description |
|----------|---------|-------------|
| `LLM_DEFAULT_TEMPERATURE` | `0.7` | Default sampling temperature (0.0-2.0) |
| `LLM_DEFAULT_TOP_P` | `0.9` | Default nucleus sampling (0.0-1.0) |
| `LLM_DEFAULT_MAX_TOKENS` | `512` | Default max tokens to generate |
| `LLM_MAX_TOKENS_LIMIT` | `4096` | Hard limit on max tokens |
| `LLM_MAX_CONCURRENT_REQUESTS` | `10` | Concurrent request limit |

### Optional - Model Loading

| Variable | Default | Description |
|----------|---------|-------------|
| `LLM_MODEL_PATH` | - | Custom model directory |
| `HF_HOME` | - | HuggingFace cache directory |
| `HF_ENDPOINT` | - | HuggingFace mirror URL (for air-gapped) |

### Observability

| Variable | Default | Description |
|----------|---------|-------------|
| `SERVICE_NAME` | `llm-inference-api` | Service name for telemetry |
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

Models compatible with [mistral.rs](https://github.com/EricLBuehler/mistral.rs):

| Model | Notes |
|-------|-------|
| `TinyLlama/TinyLlama-1.1B-Chat-v1.0` | Small, fast |
| `mistralai/Mistral-7B-Instruct-v0.2` | 7B instruction-tuned |
| `meta-llama/Llama-2-7b-chat-hf` | Meta Llama 2 |

See mistral.rs documentation for full model compatibility.

---

## GPU Acceleration

CUDA support via mistral.rs:

- Automatic GPU detection (falls back to CPU)
- Multiple GPU support

---

## Building

```bash
# Debug build
cargo build -p llm-inference-api

# Release build
cargo build -p llm-inference-api --release
```

### Docker (with CUDA)

```bash
docker build -f crates/llm-inference-api/Dockerfile -t llm-inference-api:latest .
```

---

## Running

```bash
export LLM_ALLOWED_MODELS="TinyLlama/TinyLlama-1.1B-Chat-v1.0"
cargo run -p llm-inference-api
```

---

## Health Checks

```bash
curl http://localhost:8091/health/live
curl http://localhost:8091/health/ready
```

---

## License

Apache License 2.0
