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
| `POST` | `/api/generate` | **Completion-style** text generation (raw prompt) |
| `POST` | `/api/generate/stream` | **Completion-style** streaming generation (SSE) |
| `POST` | `/api/chat` | **Chat-style** completion (conversation messages) |
| `POST` | `/api/chat/stream` | **Chat-style** streaming completion (SSE) |
| `GET` | `/swagger-ui` | Interactive API documentation |
| `GET` | `/metrics` | Prometheus metrics |

---

## API Endpoints

### Completion vs Chat

- **`/api/generate`**: Uses **completion-style** requests with a single prompt string (OpenAI-compatible completion endpoint)
- **`/api/chat`**: Uses **chat-style** requests with structured message arrays (OpenAI-compatible chat endpoint)

Both work with instruction-tuned models, but use different internal request formats.

---

## API Examples

### Text Generation (Completion)

Completion-style generation with a raw text prompt:

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

### Streaming Generation (Completion)

Streaming completion-style generation:

```bash
curl -X POST http://localhost:8091/api/generate/stream \
  -H "Content-Type: application/json" \
  -d '{
    "prompt": "Write a haiku about coding.",
    "model": "TinyLlama/TinyLlama-1.1B-Chat-v1.0"
  }'
```

### Chat Completion

Chat-style generation with message history:

```bash
curl -X POST http://localhost:8091/api/chat \
  -H "Content-Type: application/json" \
  -d '{
    "model": "TinyLlama/TinyLlama-1.1B-Chat-v1.0",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
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
| `LLM_ENABLE_ISQ` | `false` | Enable runtime quantization (slow, see [QUANTIZATION.md](QUANTIZATION.md)) |
| `LLM_ISQ_TYPE` | - | ISQ quantization type (Q4_K, Q8_0, etc.) |

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

### Pre-quantized Models (RECOMMENDED)

Pre-quantized models load faster and use less memory. See [QUANTIZATION.md](QUANTIZATION.md) for details.

**GGUF Models** (CPU, CUDA, Metal):

| Model | Notes |
|-------|-------|
| `TheBloke/Mistral-7B-Instruct-v0.2-GGUF` | 7B instruction-tuned, quantized |
| `TheBloke/Llama-2-7B-Chat-GGUF` | Meta Llama 2, quantized |
| `TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF` | Small, fast, quantized |
| `microsoft/Phi-3-mini-4k-instruct-gguf` | Microsoft Phi-3, quantized |

**Specifying GGUF Files**:
- Always use explicit format: `repo:filename.gguf`
- Optional tokenizer override: `repo:filename.gguf@tokenizer-repo`
- Browse files at: `https://huggingface.co/{repo}/tree/main`

Examples:
- `TheBloke/Mistral-7B-Instruct-v0.2-GGUF:mistral-7b-instruct-v0.2.Q4_K_M.gguf`
- `microsoft/Phi-3-mini-4k-instruct-gguf:Phi-3-mini-4k-instruct-q4.gguf`
- `bartowski/Llama-3-8B-GGUF:Llama-3-8B-Q8_0.gguf@meta-llama/Meta-Llama-3-8B`

**GPTQ Models** (CUDA only):

| Model | Notes |
|-------|-------|
| `TheBloke/Mistral-7B-Instruct-v0.2-GPTQ` | 7B instruction-tuned, GPTQ quantized |
| `kaitchup/Phi-3-mini-4k-instruct-gptq-4bit` | Phi-3 mini, 4-bit GPTQ |
| `TheBloke/Llama-2-7B-Chat-GPTQ` | Meta Llama 2, GPTQ quantized |

### Standard HuggingFace Models

| Model | Notes |
|-------|-------|
| `TinyLlama/TinyLlama-1.1B-Chat-v1.0` | Small, fast |
| `mistralai/Mistral-7B-Instruct-v0.2` | 7B instruction-tuned |
| `meta-llama/Llama-2-7b-chat-hf` | Meta Llama 2 |

**💡 Tip**: Use pre-quantized models for faster loading and lower memory usage:
- **GGUF models**: Work on any device (CPU, CUDA, Metal) from [TheBloke](https://huggingface.co/TheBloke)
- **GPTQ models**: CUDA-only, optimized for NVIDIA GPUs from [TheBloke](https://huggingface.co/TheBloke) and [kaitchup](https://huggingface.co/kaitchup)

See [QUANTIZATION.md](QUANTIZATION.md) for a complete guide.

See mistral.rs documentation for full model compatibility.

---

## GPU Acceleration

CUDA support via mistral.rs:

- Automatic GPU detection (falls back to CPU)
- Multiple GPU support

---

## Building

### Local Development

For CPU-only builds:
```bash
# Debug build
cargo build -p llm-inference-api

# Release build
cargo build -p llm-inference-api --release
```

For CUDA-accelerated builds (requires NVIDIA GPU):
```bash
# One-time setup (from repository root)
./setup_cuda.sh

# Build with CUDA support
./cargo_cuda.sh build -p llm-inference-api --release
```

See the [root README](../../README.md#building-from-source) for more details on CUDA builds.

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

### With GPU (Local Development)

```bash
# After running setup_cuda.sh once (from repository root)
export CUDA_VISIBLE_DEVICES=0
./cargo_cuda.sh run -p llm-inference-api
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
