# CUDA-enabled Dockerfile for inference-api
# Designed for NVIDIA GPUs with CUDA 12.x and cuDNN 9.x
#
# Requirements:
#   - ONNX Runtime 1.23.0 GPU (bundled in image)
#   - CUDA 12.9 (from base image)
#   - cuDNN 9.x (from base image)
#
# Build examples:
#   docker build -t inference-api:cuda12-sm89 .
#   docker build --build-arg CUDA_COMPUTE_CAP=80 -t inference-api:cuda12-sm80 .
# Run: docker run --gpus all inference-api:cuda12-sm89

# Use NVIDIA's official CUDA development image for building
# nvidia/cuda:12.9.1-cudnn-devel-ubuntu24.04 includes cuDNN 9.x
FROM nvidia/cuda:12.9.1-cudnn-devel-ubuntu24.04 AS builder

# Build argument for CUDA compute capability
# Defaults to 89 (Ada Lovelace: RTX 40xx, L40/L40S)
# Supported values: 75, 80, 86, 89, 90
ARG CUDA_COMPUTE_CAP=89

ENV DEBIAN_FRONTEND=noninteractive
ENV PATH="/root/.cargo/bin:${PATH}"

# Set CUDA compute capability from build arg
# This avoids nvidia-smi detection during build and allows per-architecture builds
ENV CUDA_COMPUTE_CAP=${CUDA_COMPUTE_CAP}

# Install build dependencies
RUN apt-get update && apt-get upgrade -yq && \
    apt-get install -yq \
        curl \
        build-essential \
        cmake \
        pkg-config \
        libssl-dev \
        git \
        wget \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Download ONNX Runtime GPU version manually
# The ort crate's download strategy only supports CPU binaries
ARG ORT_VERSION=1.23.0
RUN mkdir -p /opt/onnxruntime && cd /opt/onnxruntime \
    && wget -q https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-linux-x64-gpu-${ORT_VERSION}.tgz \
    && tar -xzf onnxruntime-linux-x64-gpu-${ORT_VERSION}.tgz \
    && mv onnxruntime-linux-x64-gpu-${ORT_VERSION}/lib/* . \
    && mkdir -p include \
    && mv onnxruntime-linux-x64-gpu-${ORT_VERSION}/include/* include/ \
    && rm -rf onnxruntime-linux-x64-gpu-${ORT_VERSION} onnxruntime-linux-x64-gpu-${ORT_VERSION}.tgz

# Configure ort-sys to use dynamic linking (matching local setup)
ENV ORT_LIB_LOCATION=/opt/onnxruntime
ENV ORT_PREFER_DYNAMIC_LINK=1
ENV ORT_DYLIB_PATH=/opt/onnxruntime/libonnxruntime.so
ENV LD_LIBRARY_PATH=/opt/onnxruntime:${LD_LIBRARY_PATH}

WORKDIR /usr/src/app

# Copy workspace
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build inference-api
RUN cargo build --release -p inference-api

# Define a non-root user
RUN useradd -u 10001 -m appuser

##### Runtime - Use CUDA runtime image (smaller than devel)
FROM nvidia/cuda:12.9.1-cudnn-runtime-ubuntu24.04

# Install minimal runtime dependencies
RUN apt-get update && apt-get install -yq \
        ca-certificates \
        curl \
        libssl3 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Copy ONNX Runtime libraries from builder
COPY --from=builder /opt/onnxruntime /opt/onnxruntime

# Set library path for ONNX Runtime (matching build environment)
ENV LD_LIBRARY_PATH=/opt/onnxruntime:${LD_LIBRARY_PATH}
ENV ORT_DYLIB_PATH=/opt/onnxruntime/libonnxruntime.so

# Copy binary
COPY --from=builder /usr/src/app/target/release/inference-api /usr/local/bin/inference-api
COPY --from=builder /etc/passwd /etc/passwd

# Create directories for models and TensorRT cache
RUN mkdir -p /models /tensorrt-cache && chown -R 10001:10001 /models /tensorrt-cache

USER appuser

EXPOSE 8090

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8090/health/live || exit 1

ENTRYPOINT ["inference-api"]
