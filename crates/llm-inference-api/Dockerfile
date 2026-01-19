# CUDA-enabled Dockerfile for llm-inference-api
# Designed for NVIDIA GPUs with CUDA 12.x and cuDNN 9.x
#
# Requirements:
#   - mistral.rs with CUDA support
#   - CUDA 12.9 (from base image)
#   - cuDNN 9.x (from base image)
#
# Build examples:
#   docker build -t llm-inference-api:cuda12-sm89 .
#   docker build --build-arg CUDA_COMPUTE_CAP=80 -t llm-inference-api:cuda12-sm80 .
# Run: docker run --gpus all llm-inference-api:cuda12-sm89

# Use NVIDIA's official CUDA development image for building
# nvidia/cuda:12.9.1-cudnn-devel-ubuntu24.04 includes cuDNN 9.x
FROM nvidia/cuda:12.9.1-cudnn-devel-ubuntu24.04 AS chef

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

# Install Rust and cargo-chef for dependency caching
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
    cargo install cargo-chef

WORKDIR /usr/src/app

##### Planner stage - analyze dependencies
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo chef prepare --recipe-path recipe.json

##### Builder stage - build dependencies first, then app
FROM chef AS builder
ARG CUDA_COMPUTE_CAP=89
ENV CUDA_COMPUTE_CAP=${CUDA_COMPUTE_CAP}

# Copy recipe and build dependencies (cached if Cargo.toml/Cargo.lock unchanged)
COPY --from=planner /usr/src/app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json -p llm-inference-api

# Now copy source and build (only rebuilds if source changed)
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p llm-inference-api

# Define a non-root user with home directory
RUN useradd -u 10001 -m -d /home/appuser appuser

##### Runtime - Use CUDA runtime image (smaller than devel)
FROM nvidia/cuda:12.9.1-cudnn-runtime-ubuntu24.04

# Install minimal runtime dependencies
RUN apt-get update && apt-get install -yq \
        ca-certificates \
        curl \
        libssl3 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Copy binary
COPY --from=builder /usr/src/app/target/release/llm-inference-api /usr/local/bin/llm-inference-api
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
