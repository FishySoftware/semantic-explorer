# LLM Inference API

<div align="center">

![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)
![CUDA](https://img.shields.io/badge/CUDA-12.x%2F13.x-76B900.svg)
![mistral.rs](https://img.shields.io/badge/mistral.rs-v0.7.0-blue.svg)
![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)

**Local LLM inference server using [mistral.rs v0.7.0](https://github.com/EricLBuehler/mistral.rs)**

</div>

Provides on-premise text generation with FP8 optimizations for H100/H200 GPUs.

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
| `POST` | `/api/generate` | Text generation from a prompt |
| `POST` | `/api/chat` | Chat with message history |
| `POST` | `/api/chat/stream` | Streaming chat (SSE) |
| `POST` | `/api/completions` | Text completion |
| `POST` | `/api/completions/stream` | Streaming text completion (SSE) |
| `GET` | `/swagger-ui` | Interactive API documentation |
| `GET` | `/metrics` | Prometheus metrics |

---

## API Endpoints

### Text Generation vs Chat vs Completions

- **`/api/generate`**: Generates a response from a provided prompt
- **`/api/chat`**: Chat with message history (structured message arrays)
- **`/api/completions`**: Text completion (OpenAI-compatible format)

All work with instruction-tuned models, but use different request formats. Chat and completions endpoints also support streaming via their `/stream` variants.

---

## API Examples

### Text Generation

Generate a response from a prompt:

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

### Streaming Text Generation

Stream a response token-by-token via chat:

```bash
curl -X POST http://localhost:8091/api/chat/stream \
  -H "Content-Type: application/json" \
  -d '{
    "model": "TinyLlama/TinyLlama-1.1B-Chat-v1.0",
    "messages": [
      {"role": "user", "content": "Write a haiku about coding."}
    ]
  }'
```

### Chat

Chat with message history:

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
| `LLM_ALLOWED_MODELS` | Comma-separated list of allowed model IDs |

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
| `LLM_ENABLE_ISQ` | `false` | Enable in-situ runtime quantization (slow, not cached) |
| `LLM_ISQ_TYPE` | - | ISQ quantization type (Q4_K, Q8_0, etc.) |

### Optional - Paged Attention (v0.7.0+)

| Variable | Default | Description |
|----------|---------|-------------|
| `LLM_PAGED_ATTENTION_BLOCK_SIZE` | `32` | Paged attention block size |
| `LLM_PAGED_ATTENTION_CONTEXT_SIZE` | `1024` | GPU memory context size |
| `LLM_PAGED_CACHE_TYPE` | `auto` | KV cache type: `auto` (native dtype) or `f8e4m3` (FP8, H100/H200 optimized) |
| `LLM_ENABLE_PREFIX_CACHING` | `false` | Enable prefix caching for multi-turn/RAG acceleration |

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

## Supported Models (mistral.rs v0.7.0)

This service uses [mistral.rs v0.7.0](https://github.com/EricLBuehler/mistral.rs) as the inference engine with FP8 optimizations for H100/H200 GPUs.

### Supported Model Architectures

| Architecture | New in v0.7.0 | Examples |
|-------------|---------------|----------|
| **Llama** | | Llama 2, Llama 3.x, Code Llama, TinyLlama |
| **Mistral/Mixtral** | | Mistral 7B, Mixtral 8x7B/8x22B (MoE) |
| **Phi** | | Phi-2, Phi-3, Phi-3.5, Phi-4 |
| **Qwen** | ✅ Qwen 3 | Qwen, Qwen 2.5, Qwen 3 |
| **Gemma** | ✅ Gemma 3n | Gemma, Gemma 2, Gemma 3n (vision) |
| **GLM** | ✅ GLM-4 | GLM-4, GLM-4.7 Flash |
| **DeepSeek** | | DeepSeek v2, DeepSeek v3 |
| **SmolLM** | ✅ SmolLM3 | SmolLM, SmolLM3 |
| **Ministral** | ✅ Ministral 3 | Ministral 3 |
| **Granite** | ✅ Hybrid MoE | Granite Hybrid MoE |
| **StableLM** | | StableLM, StableLM 2 |

### Copy-Paste Ready Model IDs

Below are HuggingFace model IDs you can directly use in `LLM_ALLOWED_MODELS`:

#### GGUF Models (Recommended - Fast Loading)

**Format**: `repo:filename.gguf` or `repo:filename.gguf@tokenizer-repo`

```bash
# Llama 3.x (Meta)
bartowski/Meta-Llama-3.1-8B-Instruct-GGUF:Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf
bartowski/Meta-Llama-3.1-70B-Instruct-GGUF:Meta-Llama-3.1-70B-Instruct-Q4_K_M.gguf
bartowski/Llama-3.2-3B-Instruct-GGUF:Llama-3.2-3B-Instruct-Q4_K_M.gguf

# Mistral/Mixtral
bartowski/Mistral-7B-Instruct-v0.3-GGUF:Mistral-7B-Instruct-v0.3-Q4_K_M.gguf
bartowski/Mixtral-8x7B-Instruct-v0.1-GGUF:Mixtral-8x7B-Instruct-v0.1-Q4_K_M.gguf

# Qwen 2.5/3
Qwen/Qwen2.5-7B-Instruct-GGUF:qwen2.5-7b-instruct-q4_k_m.gguf
Qwen/Qwen2.5-32B-Instruct-GGUF:qwen2.5-32b-instruct-q4_k_m.gguf
Qwen/Qwen2.5-72B-Instruct-GGUF:qwen2.5-72b-instruct-q4_k_m.gguf

# Phi-3/4 (Microsoft)
microsoft/Phi-3-mini-4k-instruct-gguf:Phi-3-mini-4k-instruct-q4.gguf
bartowski/Phi-3.5-mini-instruct-GGUF:Phi-3.5-mini-instruct-Q4_K_M.gguf
bartowski/phi-4-GGUF:phi-4-Q4_K_M.gguf

# Gemma 2/3 (Google)
bartowski/gemma-2-9b-it-GGUF:gemma-2-9b-it-Q4_K_M.gguf
bartowski/gemma-2-27b-it-GGUF:gemma-2-27b-it-Q4_K_M.gguf

# GLM-4 (NEW in v0.7.0)
bartowski/glm-4-9b-chat-GGUF:glm-4-9b-chat-Q4_K_M.gguf

# DeepSeek
bartowski/DeepSeek-V2.5-GGUF:DeepSeek-V2.5-Q4_K_M.gguf
bartowski/DeepSeek-R1-Distill-Qwen-7B-GGUF:DeepSeek-R1-Distill-Qwen-7B-Q4_K_M.gguf

# SmolLM (Small & Fast)
bartowski/SmolLM2-1.7B-Instruct-GGUF:SmolLM2-1.7B-Instruct-Q4_K_M.gguf

# TinyLlama (Development/Testing)
TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF:tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf
```

#### Standard HuggingFace Models (Full Precision)

```bash
# Llama 3.x
meta-llama/Meta-Llama-3.1-8B-Instruct
meta-llama/Meta-Llama-3.1-70B-Instruct

# Mistral
mistralai/Mistral-7B-Instruct-v0.3
mistralai/Mixtral-8x7B-Instruct-v0.1

# Qwen 2.5/3
Qwen/Qwen2.5-7B-Instruct
Qwen/Qwen2.5-72B-Instruct

# Phi-3/4
microsoft/Phi-3-mini-4k-instruct
microsoft/Phi-3.5-mini-instruct
microsoft/phi-4

# Gemma
google/gemma-2-9b-it
google/gemma-2-27b-it

# GLM-4 (NEW in v0.7.0)
THUDM/glm-4-9b-chat

# DeepSeek
deepseek-ai/DeepSeek-V2.5
deepseek-ai/DeepSeek-R1-Distill-Qwen-7B

# Development/Testing
TinyLlama/TinyLlama-1.1B-Chat-v1.0
```

#### Vision Models (NEW in v0.7.0)

```bash
# Qwen3 VL (Vision-Language)
Qwen/Qwen2.5-VL-7B-Instruct

# Gemma 3n (Vision)
google/gemma-3n-E4B-it

# LLaVA
llava-hf/llava-v1.6-mistral-7b-hf

# Phi-3.5 Vision
microsoft/Phi-3.5-vision-instruct
```

### Quantization Formats

| Format | Hardware | Description |
|--------|----------|-------------|
| **GGUF** | CPU, CUDA, Metal | Universal format, many quantization levels (Q4_K_M, Q5_K_M, Q8_0, etc.) |
| **GPTQ** | CUDA only | GPU-optimized 4-bit quantization |
| **AWQ** | CUDA only | Activation-aware weight quantization |
| **Standard** | CPU, CUDA | Full precision HuggingFace models (FP16/BF16) |

### Model Selection Guide

| Use Case | Recommended Model | VRAM Required |
|----------|------------------|---------------|
| **Development/Testing** | TinyLlama-1.1B, SmolLM2-1.7B | 2-4 GB |
| **General Chat** | Llama-3.1-8B, Qwen2.5-7B, Mistral-7B | 6-8 GB (Q4) |
| **High Quality** | Llama-3.1-70B, Qwen2.5-72B | 40+ GB (Q4) |
| **Reasoning** | DeepSeek-R1-Distill, Phi-4 | 8-16 GB |
| **Vision Tasks** | Qwen2.5-VL, Phi-3.5-vision | 8-16 GB |
| **Low VRAM (< 8GB)** | Q4_K_M quantized 7B models | 4-6 GB |
| **CPU Only** | GGUF Q4_K_M or Q4_K_S | N/A |

### v0.7.0 Performance Features

| Feature | Description | How to Enable |
|---------|-------------|---------------|
| **FP8 KV Cache** | ~50% memory reduction on Hopper+ GPUs | `LLM_PAGED_CACHE_TYPE=f8e4m3` |
| **Prefix Caching** | Faster multi-turn & RAG via KV cache reuse | `LLM_ENABLE_PREFIX_CACHING=true` |
| **Fused Kernels** | GEMV, GLU, MoE fusion for ~2x speedup | Automatic on CUDA |
| **MLA Decode** | Optimized for DeepSeek v2/v3, GLM-4.7 | Automatic |

**💡 Tips**:
- **GGUF models** work on any device (CPU, CUDA, Metal) - most flexible
- **GPTQ/AWQ models** require NVIDIA GPU but are faster for inference
- Use **instruction-tuned** models (names contain "Instruct", "Chat") for chat
- Enable **FP8 + prefix caching** on H100/H200 for best performance
- Check model card on HuggingFace for license and usage restrictions

For full model compatibility, see [mistral.rs v0.7.0 release notes](https://github.com/EricLBuehler/mistral.rs/releases/tag/v0.7.0).

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
