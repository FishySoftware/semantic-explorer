# Inference API

Local AI inference service for Semantic Explorer, providing CUDA-accelerated embedding and reranking capabilities.

## Overview

The Inference API uses [fastembed-rs](https://github.com/Anush008/fastembed-rs) with CUDA acceleration to run ONNX-based models, enabling:
- **Text Embeddings**: Generate vector embeddings for semantic search with GPU acceleration
- **Reranking**: Improve search result relevance with cross-encoder models
- **CUDA-Only Mode**: Optimized for NVIDIA GPUs (H100, A100, etc.) - requires CUDA
- **Airgapped Operation**: Run completely offline with pre-downloaded models

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `INFERENCE_PORT` | HTTP server port | `8090` |
| `INFERENCE_HOSTNAME` | Bind address | `0.0.0.0` |
| `HF_HOME` | HuggingFace cache directory for models | `~/.cache/huggingface` |
| `HF_ENDPOINT` | HuggingFace endpoint URL (for proxies/mirrors) | `https://huggingface.co` |
| `INFERENCE_MODEL_PATH` | Custom model directory for user-provided ONNX models | None |
| `INFERENCE_PRELOAD_MODELS` | Comma-separated list of model IDs to preload at startup | None |
| `INFERENCE_DEFAULT_EMBEDDING_MODEL` | Default embedding model | `BAAI/bge-small-en-v1.5` |
| `INFERENCE_DEFAULT_RERANKER_MODEL` | Default reranker model | `BAAI/bge-reranker-base` |
| `INFERENCE_MAX_BATCH_SIZE` | Maximum batch size for embedding requests | `256` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OpenTelemetry collector endpoint | `http://localhost:4317` |
| `SERVICE_NAME` | Service identifier for tracing | `inference-api` |
| `LOG_FORMAT` | Log format: `json` or `pretty` | `json` |

### GPU/CUDA Configuration (Required for Local Development)

**For Docker/Production**: CUDA libraries are bundled in the container image. Just use `--gpus all`.

**For Local Development**: Manual CUDA setup is required because:
- The `fastembed-rs` crate with `ort-download-binaries` feature downloads CPU-only ONNX Runtime binaries
- GPU-accelerated ONNX Runtime must be manually downloaded and linked to enable CUDA
- This setup is only needed once per machine and is **fully automated** with the provided script

#### Automated Setup (Recommended)

Run this script once to download and configure ONNX Runtime:

```bash
cd crates/inference-api
./setup_cuda_local.sh
```

This downloads ONNX Runtime to `.onnxruntime-cuda` (gitignored, project-local).

**Then use the wrapper script for all cargo commands:**

```bash
# Build with CUDA support
./run_with_cuda.sh build

# Run with CUDA support  
./run_with_cuda.sh run

# Any cargo command works
./run_with_cuda.sh test
```

The wrapper script automatically sets the correct paths relative to the project directory, so it works for **any user** without modifying `.env` or having home directory dependencies.

**Prerequisites:**
- NVIDIA GPU with CUDA 12.x installed
- NVIDIA drivers (version 535+ recommended)
- cuDNN 9.x libraries

#### Manual Configuration (Advanced)

If you need to customize the installation location or troubleshoot:

```bash
# Download GPU version
INSTALL_DIR="/your/custom/path/onnxruntime-cuda"
mkdir -p "$INSTALL_DIR"
wget https://github.com/microsoft/onnxruntime/releases/download/v1.20.1/onnxruntime-linux-x64-gpu-1.20.1.tgz
tar -xzf onnxruntime-linux-x64-gpu-1.20.1.tgz
mv onnxruntime-linux-x64-gpu-1.20.1/lib/* "$INSTALL_DIR/"
mv onnxruntime-linux-x64-gpu-1.20.1/include "$INSTALL_DIR/"

# Create pkg-config file for ort-sys linking
mkdir -p "$INSTALL_DIR/pkgconfig"
cat > "$INSTALL_DIR/pkgconfig/libonnxruntime.pc" << EOF
prefix=$INSTALL_DIR
libdir=\${prefix}
includedir=\${prefix}/include/onnxruntime

Name: onnxruntime
Description: ONNX runtime
Version: 1.20.1
Libs: -L\${libdir} -lonnxruntime
Cflags: -I\${includedir}
EOF

# Add to .env file
PKG_CONFIG_PATH="$INSTALL_DIR/pkgconfig:${PKG_CONFIG_PATH:-}"
ORT_LIB_LOCATION="$INSTALL_DIR"
ORT_DYLIB_PATH="$INSTALL_DIR/libonnxruntime.so"
LD_LIBRARY_PATH="$INSTALL_DIR:${LD_LIBRARY_PATH:-}"
CUDA_VISIBLE_DEVICES=0
```

**Important Notes:**
- ONNX Runtime 1.20.x requires **CUDA 12.x** and **cuDNN 9.x**
- Check compatibility: [ONNX Runtime Release Notes](https://onnxruntime.ai/docs/execution-providers/CUDA-ExecutionProvider.html)
- Verify GPU usage with `nvidia-smi` while the service is running

### TLS/SSL Configuration

For secure HTTPS connections:

| Variable | Description | Default |
|----------|-------------|---------|
| `SERVER_SSL_ENABLED` | Enable TLS/SSL | `false` |
| `SERVER_SSL_CERT_PATH` | Path to TLS certificate file (PEM format) | None |
| `SERVER_SSL_KEY_PATH` | Path to TLS private key file (PEM format) | None |

**Example TLS setup:**
```bash
export SERVER_SSL_ENABLED=true
export SERVER_SSL_CERT_PATH=/etc/ssl/certs/inference-api.crt
export SERVER_SSL_KEY_PATH=/etc/ssl/private/inference-api.key
```

### Airgapped / Offline Mode

For environments without network access:

1. **Pre-download models** on a connected machine:
   ```bash
   HF_HOME=/path/to/models python -c "from fastembed import TextEmbedding; TextEmbedding('BAAI/bge-small-en-v1.5')"
   ```

2. **Copy the cache** to your airgapped environment at `HF_HOME`

3. **Set environment variables**:
   ```bash
   export HF_HOME=/path/to/models
   export INFERENCE_PRELOAD_MODELS=BAAI/bge-small-en-v1.5,BAAI/bge-reranker-base
   ```

### Using an Artifactory Proxy

Set `HF_ENDPOINT` to your HuggingFace proxy:
```bash
export HF_ENDPOINT=https://artifactory.company.com/huggingface
```

## API Endpoints

### Health

- `GET /health/live` - Liveness probe
- `GET /health/ready` - Readiness probe (checks if models are loaded)

### Models

- `GET /api/models` - List available embedding and reranker models

### Embeddings

- `POST /api/embed` - Generate embedding for a single text
- `POST /api/embed/batch` - Generate embeddings for multiple texts

### Reranking

- `POST /api/rerank` - Rerank documents by relevance to a query

## Supported Models

### Embedding Models
- `BAAI/bge-small-en-v1.5` (384 dimensions)
- `BAAI/bge-base-en-v1.5` (768 dimensions)
- `BAAI/bge-large-en-v1.5` (1024 dimensions)
- `sentence-transformers/all-MiniLM-L6-v2` (384 dimensions)
- `nomic-ai/nomic-embed-text-v1.5` (768 dimensions)
- And many more via fastembed

### Reranker Models
- `BAAI/bge-reranker-base`
- `BAAI/bge-reranker-large`
- `jinaai/jina-reranker-v1-base-en`

## Development

```bash
# Run locally
cargo run -p inference-api

# Build release
cargo build --release -p inference-api
```

## Docker

### CPU-only (musl static binary)

```bash
# Build image
docker build -f crates/inference-api/Dockerfile -t inference-api .

# Run with model volume
docker run -p 8090:8090 -v /path/to/models:/models -e HF_HOME=/models inference-api
```

### GPU/CUDA (NVIDIA H100, A100)

```bash
# Build CUDA image
docker build -f crates/inference-api/Dockerfile.cuda -t inference-api:cuda .

# Run with GPU
docker run --gpus all -p 8090:8090 \
  -v /path/to/models:/models \
  -e HF_HOME=/models \
  -e INFERENCE_CUDA_ENABLED=true \
  -e INFERENCE_CUDA_DEVICE_ID=0 \
  -e INFERENCE_PRELOAD_MODELS=BAAI/bge-small-en-v1.5 \
  inference-api:cuda
```

### Kubernetes with H100 GPUs

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: inference-api
spec:
  replicas: 1
  selector:
    matchLabels:
      app: inference-api
  template:
    metadata:
      labels:
        app: inference-api
    spec:
      containers:
      - name: inference-api
        image: inference-api:cuda
        ports:
        - containerPort: 8090
        env:
        - name: INFERENCE_CUDA_ENABLED
          value: "true"
        - name: HF_HOME
          value: /models
        - name: INFERENCE_PRELOAD_MODELS
          value: "BAAI/bge-small-en-v1.5,BAAI/bge-reranker-base"
        resources:
          limits:
            nvidia.com/gpu: 1
            memory: "16Gi"
          requests:
            nvidia.com/gpu: 1
            memory: "8Gi"
        volumeMounts:
        - name: models
          mountPath: /models
        livenessProbe:
          httpGet:
            path: /health/live
            port: 8090
        readinessProbe:
          httpGet:
            path: /health/ready
            port: 8090
          initialDelaySeconds: 60
      volumes:
      - name: models
        persistentVolumeClaim:
          claimName: inference-models
      tolerations:
      - key: nvidia.com/gpu
        operator: Exists
        effect: NoSchedule
```
