# CUDA-enabled Dockerfile for llm-inference-api
# Designed for NVIDIA GPUs with CUDA 12.x/13.x and cuDNN 9.x
# Uses mistral.rs v0.7.0 with FP8 optimizations for H100/H200 GPUs
#
# This crate is excluded from the main workspace to isolate its candle-core
# dependency (mistralrs v0.7.0 uses candle 0.9.2, while fastembed uses 0.9.1)
#
# Requirements:
#   - mistral.rs v0.7.0+ with CUDA support
#   - CUDA 12.9.1 or 13.1.1 (from base image)
#   - cuDNN 9.x (from base image)
#
# Build examples:
#   docker build -t llm-inference-api:cuda13-sm90 .                                       # H100/H200 (default)
#   docker build --build-arg CUDA_COMPUTE_CAP=89 -t llm-inference-api:cuda13-sm89 .       # Ada (RTX 40xx, L40)
#   docker build --build-arg CUDA_VERSION=12.9.1 -t llm-inference-api:cuda12-sm90 .       # CUDA 12.9.1 compat
# Run: docker run --gpus all llm-inference-api:cuda13-sm90

# Build arguments for CUDA version and compute capability
ARG CUDA_VERSION=13.1.1

# Use NVIDIA's official CUDA development image for building
FROM nvidia/cuda:${CUDA_VERSION}-cudnn-devel-ubuntu24.04 AS chef

# Build argument for CUDA compute capability
# Defaults to 90 (Hopper: H100, H200) for FP8 optimizations
# Supported values: 89 (Ada: RTX 40xx, L40/L40S), 90 (Hopper: H100/H200)
ARG CUDA_COMPUTE_CAP=90

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

# Install Rust and cargo-chef for dependency caching
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
    cargo install cargo-chef

WORKDIR /usr/src/app

##### Planner stage - analyze dependencies
# Note: llm-inference-api is excluded from the main workspace but depends on crates/core
# It has its own Cargo.lock for isolated dependency resolution
FROM chef AS planner
# Copy the core crate (dependency) and llm-inference-api crate
COPY crates/core ./crates/core
COPY crates/llm-inference-api ./crates/llm-inference-api
# Run chef prepare from the llm-inference-api directory
WORKDIR /usr/src/app/crates/llm-inference-api
RUN cargo chef prepare --recipe-path /usr/src/app/recipe.json

##### Builder stage - build dependencies first, then app
FROM chef AS builder
ARG CUDA_COMPUTE_CAP=90
ENV CUDA_COMPUTE_CAP=${CUDA_COMPUTE_CAP}

# Copy recipe and build dependencies (cached if Cargo.toml/Cargo.lock unchanged)
COPY --from=planner /usr/src/app/recipe.json recipe.json
WORKDIR /usr/src/app/crates/llm-inference-api
RUN cargo chef cook --release --recipe-path /usr/src/app/recipe.json

# Now copy source and build (only rebuilds if source changed)
WORKDIR /usr/src/app
COPY crates/core ./crates/core
COPY crates/llm-inference-api ./crates/llm-inference-api
WORKDIR /usr/src/app/crates/llm-inference-api
RUN cargo build --release

# Define a non-root user with home directory
RUN useradd -u 10001 -m -d /home/appuser appuser

##### Runtime - Use CUDA runtime image (smaller than devel)
ARG CUDA_VERSION=13.1.1
FROM nvidia/cuda:${CUDA_VERSION}-cudnn-runtime-ubuntu24.04

# Install minimal runtime dependencies
RUN apt-get update && apt-get install -yq \
        ca-certificates \
        curl \
        libssl3 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Copy binary (built from crates/llm-inference-api, target is in that directory)
COPY --from=builder /usr/src/app/crates/llm-inference-api/target/release/llm-inference-api /usr/local/bin/llm-inference-api
COPY --from=builder /etc/passwd /etc/passwd

# Create directories for models cache and HuggingFace cache
# LLMs are large - ensure sufficient space is allocated
# mistral.rs also uses ~/.cache/huggingface for token and other files
RUN mkdir -p /models /home/appuser/.cache/huggingface && \
    chown -R 10001:10001 /models /home/appuser

# Set environment variables for proper cache directory usage
ENV HOME=/home/appuser
ENV HF_HOME=/models

USER appuser

EXPOSE 8091

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=120s --retries=3 \
    CMD curl -f http://localhost:8091/health/live || exit 1

ENTRYPOINT ["llm-inference-api"]
