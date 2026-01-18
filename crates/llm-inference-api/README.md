# LLM Inference API

Local AI inference service for **Semantic Explorer** providing text generation, streaming, and chat capabilities using [mistral.rs](https://github.com/EricLBuehler/mistral.rs) with CUDA GPU acceleration.

## Features

- **Text Generation**: Generate text from prompts with configurable parameters
- **Streaming**: Server-Sent Events (SSE) streaming for token-by-token generation
- **Chat Completions**: Multi-turn conversations with message history
- **GPU Acceleration**: CUDA-optimized inference via mistral.rs
- **Model Management**: Lazy loading with configurable model allowlists
- **OpenAPI Documentation**: Auto-generated Swagger UI at `/swagger-ui/`
- **Observability**: OpenTelemetry tracing, Prometheus metrics, structured logging
- **Production Ready**: TLS support, health checks, CORS configuration

## Quick Start

### Prerequisites

- NVIDIA GPU with CUDA 12.x support
- Docker with NVIDIA Container Toolkit (for containerized deployment)
- OR Rust 1.75+ and CUDA 12.x installed locally

### Running Locally

1. **Configure environment**:
```bash
cd crates/llm-inference-api
cp .env.example .env
# Edit .env to configure models and settings
```

2. **Run the service**:
```bash
cargo run --release
```

3. **Access the API**:
- Swagger UI: http://localhost:8091/swagger-ui/
- Health check: http://localhost:8091/health/ready
- Metrics: http://localhost:8091/metrics

### Running with Docker

```bash
# Build
docker build -f crates/llm-inference-api/Dockerfile -t llm-inference-api:cuda .

# Run
docker run --gpus all \
  -p 8091:8091 \
  -e LLM_DEFAULT_MODEL="mistralai/Mistral-7B-Instruct-v0.2" \
  -v $(pwd)/models:/models \
  llm-inference-api:cuda
```

## API Endpoints

### Generation

**POST /api/generate** - Generate text from a prompt

```bash
curl -X POST http://localhost:8091/api/generate \
  -H "Content-Type: application/json" \
  -d '{
    "model": "mistralai/Mistral-7B-Instruct-v0.2",
    "prompt": "Explain quantum computing in simple terms:",
    "temperature": 0.7,
    "max_tokens": 200
  }'
```

Response:
```json
{
  "text": "Quantum computing is...",
  "model": "mistralai/Mistral-7B-Instruct-v0.2",
  "tokens_generated": 156,
  "finish_reason": "length"
}
```

### Streaming

**POST /api/generate/stream** - Stream text generation

```bash
curl -X POST http://localhost:8091/api/generate/stream \
  -H "Content-Type: application/json" \
  -d '{
    "model": "mistralai/Mistral-7B-Instruct-v0.2",
    "prompt": "Count from 1 to 10:",
    "max_tokens": 50
  }'
```

Returns Server-Sent Events stream:
```
data: 1
data: , 2
data: , 3
...
```

### Chat Completions

**POST /api/chat** - Multi-turn conversation

```bash
curl -X POST http://localhost:8091/api/chat \
  -H "Content-Type: application/json" \
  -d '{
    "model": "mistralai/Mistral-7B-Instruct-v0.2",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "What is the capital of France?"}
    ],
    "temperature": 0.7,
    "max_tokens": 100
  }'
```

Response:
```json
{
  "message": {
    "role": "assistant",
    "content": "The capital of France is Paris."
  },
  "model": "mistralai/Mistral-7B-Instruct-v0.2",
  "tokens_generated": 8,
  "finish_reason": "eos"
}
```

### Model Discovery

**GET /api/models** - List available models

```bash
curl http://localhost:8091/api/models
```

Response:
```json
{
  "models": [
    {
      "id": "mistralai/Mistral-7B-Instruct-v0.2",
      "name": "Mistral 7B Instruct v0.2",
      "description": "Mistral AI's 7 billion parameter instruction-tuned model",
      "size": "7B",
      "capabilities": ["text-generation", "chat"]
    }
  ]
}
```

### Health Checks

**GET /health/live** - Liveness probe (always returns OK)

**GET /health/ready** - Readiness probe (returns OK when models loaded)

```bash
curl http://localhost:8091/health/ready
```

Response:
```json
{
  "status": "ok",
  "llm_models_loaded": true
}
```

## Configuration

All configuration is via environment variables. See [`.env.example`](.env.example) for full documentation.

### Key Settings

| Variable | Default | Description |
|----------|---------|-------------|
| `LLM_INFERENCE_PORT` | 8091 | HTTP server port |
| `LLM_DEFAULT_MODEL` | mistralai/Mistral-7B-Instruct-v0.2 | Default model |
| `LLM_ALLOWED_MODELS` | - | Comma-separated allowlist |
| `LLM_DEFAULT_TEMPERATURE` | 0.7 | Default sampling temperature |
| `LLM_DEFAULT_MAX_TOKENS` | 512 | Default max tokens to generate |
| `LLM_MAX_TOKENS_LIMIT` | 4096 | Hard limit on max tokens |
| `HF_HOME` | /models | HuggingFace model cache directory |
| `CUDA_VISIBLE_DEVICES` | - | GPU device selection |

### Generation Parameters

- **temperature** (0.0-2.0): Controls randomness. Lower = more deterministic, higher = more creative
- **top_p** (0.0-1.0): Nucleus sampling threshold. Controls diversity
- **max_tokens**: Maximum number of tokens to generate
- **stop**: Array of stop sequences to end generation

## Supported Models

The service supports models compatible with mistral.rs:

- **Mistral**: Mistral-7B, Mixtral-8x7B (instruction and base variants)
- **Llama 2**: 7B, 13B (chat variants)
- **Zephyr**: 7B beta
- **GGUF models**: Quantized models for efficient inference

See [`src/models.rs`](src/models.rs) for the full list.

## Architecture

### Model Management

- **Global Cache**: Models are cached in memory after first load
- **Lazy Loading**: Models load on first request if not pre-loaded
- **Per-Model Locking**: Concurrent requests to different models execute in parallel
- **Allowlist Filtering**: Restrict available models via `LLM_ALLOWED_MODELS`

### Concurrency

- **Max Concurrent Requests**: Configurable via `LLM_MAX_CONCURRENT_REQUESTS`
- **Thread Pool**: CPU-intensive generation runs in blocking thread pool
- **Async I/O**: Non-blocking HTTP with Actix-web

### Observability

- **Tracing**: OpenTelemetry OTLP export to Jaeger/Tempo
- **Metrics**: Prometheus metrics at `/metrics`
- **Logging**: Structured JSON logs (or pretty for development)

## Deployment

### Kubernetes/Helm

Helm charts included in `deployment/helm/semantic-explorer/`:

```bash
helm install semantic-explorer deployment/helm/semantic-explorer \
  --set llmInferenceApi.enabled=true \
  --set llmInferenceApi.image.tag=cuda-89
```

### Docker Compose

```yaml
services:
  llm-inference-api:
    image: jofish89/llm-inference-api:cuda
    ports:
      - "8091:8091"
    environment:
      - LLM_DEFAULT_MODEL=mistralai/Mistral-7B-Instruct-v0.2
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              capabilities: [gpu]
```

## Performance

### GPU Requirements

- **Minimum**: 8GB VRAM for 7B models
- **Recommended**: 16GB+ VRAM for 13B models or batching
- **Optimal**: 24GB+ VRAM for larger models (Mixtral 8x7B)

### Optimization Tips

1. **Model Selection**: Use quantized GGUF models for lower memory usage
2. **Batch Size**: Adjust `LLM_MAX_CONCURRENT_REQUESTS` based on GPU memory
3. **Token Limits**: Set `LLM_MAX_TOKENS_LIMIT` to prevent OOM
4. **Model Pre-loading**: Pre-load frequently used models at startup

## Development

### Building

```bash
cargo build -p llm-inference-api
```

### Testing

```bash
cargo test -p llm-inference-api
```

### Running Tests

```bash
# Unit tests
cargo test -p llm-inference-api

# Integration tests (requires GPU)
cargo test -p llm-inference-api --features integration-tests
```

## Troubleshooting

### CUDA Not Available

```
Error: CUDA device not found
```

**Solution**: Verify CUDA installation and GPU visibility:
```bash
nvidia-smi
echo $CUDA_VISIBLE_DEVICES
```

### Out of Memory

```
Error: CUDA out of memory
```

**Solutions**:
- Use smaller model or quantized variant
- Reduce `LLM_MAX_CONCURRENT_REQUESTS`
- Lower `LLM_DEFAULT_MAX_TOKENS`
- Set `CUDA_VISIBLE_DEVICES` to use specific GPU

### Model Download Fails

```
Error: Failed to download model
```

**Solutions**:
- Check internet connection
- For airgapped deployments, pre-download models to `HF_HOME`
- Use `HF_ENDPOINT` to point to internal mirror

## License

See LICENSE file in repository root.

## Contributing

See CONTRIBUTING.md in repository root.

## Support

- Issues: https://github.com/anthropics/semantic-explorer/issues
- Documentation: https://docs.semantic-explorer.ai
