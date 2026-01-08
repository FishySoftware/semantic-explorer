# Worker Visualizations

GPU-accelerated visualization worker for generating UMAP embeddings and cluster visualizations using RAPIDS cuML.

## Overview

The Visualization Worker consumes `VisualizationTransformJob` messages from NATS and processes embedded datasets to generate 3D visualizations:

1. **Dimensionality Reduction** - Reduces vector embeddings from high dimensions to 3D using UMAP
2. **Clustering** - Identifies topic clusters using HDBSCAN
3. **Topic Labeling** - Generates descriptive labels using TF-IDF
4. **Vector Storage** - Exports reduced vectors to `-reduced` Qdrant collection
5. **Topic Export** - Exports cluster centroids to `-reduced-topics` collection

### NATS Integration

- **Subject**: `workers.visualization-transform`
- **Consumer**: Durable consumer with pull-based message handling
- **Payload**: `VisualizationTransformJob` (includes embedded_dataset_id, source collection, and output collection names)

## Build Requirements

This crate depends on `cuml-wrapper-rs`, which requires:
- NVIDIA GPU with CUDA 12.4 support
- GPU runtime libraries (libcuml, libraft, libcublas, etc.)
- CUDA Toolkit 12.4

## Local Development Build

### Quick Start

```bash
# One-time setup: Install CUDA 12.4, Rust, micromamba, and cuML environment
source ./crates/worker-visualizations/.scripts/setup_build.sh

# Build (after setup script activates environment)
cargo build --release
```

### What `setup_build.sh` Does

This script is **idempotent** and will skip reinstalls if dependencies already exist:

1. **Checks for CUDA 12.4 or later** - Verifies NVIDIA drivers and CUDA Toolkit are installed
2. **Installs system packages** - Build essentials, cmake, curl, pkg-config, SSL libraries
3. **Sets up Rust** - Installs via rustup if not present
4. **Installs micromamba** - Package manager for GPU libraries
5. **Creates conda environment** - Persistent cuML environment at `$HOME/micromamba/envs/cuml` with:
   - cuml=24.12 (RAPIDS machine learning library)
   - CUDA 12.4 libraries (libcuml, libraft, libcublas, libcusolver, libcusparse)
   - Development tools (cmake, cuda-cudart-dev)
6. **Activates environment** - Sets `CUML_ROOT`, `CMAKE_PREFIX_PATH`, `LD_LIBRARY_PATH`, etc.

### Environment Variables

After running the setup script, the following are set in your shell:
- `CUML_ROOT` - Points to conda environment with GPU libraries
- `CMAKE_PREFIX_PATH` - For build system to find GPU development headers
- `LD_LIBRARY_PATH` - GPU runtime libraries for linking and execution
- `CPATH` - Include paths for cuML headers

### Rebuilding After First Setup

```bash
# Subsequent builds: Just activate the environment again
source ./crates/worker-visualizations/setup_build.sh
cargo build --release
```

The script will detect that CUDA, micromamba, and the cuML environment already exist and will skip reinstalls (~30s instead of 30+ minutes).

## Docker Build

### Architecture

The `Dockerfile` uses a two-stage build optimized for GPU deployments:

**Builder Stage** (RapidsAI base image):
- Base: `rapidsai/base:26.02a-cuda12-py3.13`
- Includes pre-installed CUDA 12.4 + all GPU development libraries and cuML
- Compiles worker-visualizations with dynamic linking (x86_64-unknown-linux-gnu)
- Includes full development toolchain (Rust, build essentials, cmake, etc.)

**Runtime Stage** (RapidsAI base image):
- Base: `rapidsai/base:26.02a-cuda12-py3.13`
- Same as builder to ensure runtime compatibility with compiled binary
- Contains GPU runtime libraries and minimal runtime dependencies
- Drops to non-root user (appuser) for security
- ~2-3 GB image size (smaller than full development image)

### Building the Docker Image

```bash
# From project root
docker build -f crates/worker-visualizations/Dockerfile -t worker-visualizations:latest .
```

### Why Dynamic Linking (gnu) Instead of Static (musl)?

The RAPIDS cuML libraries are built with dynamic linking and are incompatible with musl static linking. The Dockerfile uses `x86_64-unknown-linux-gnu` to link against the GPU libraries.

## CUDA Version

**Current Locked Version**: CUDA 12.4
- Matches production deployment requirements
- `setup_build.sh` validates CUDA 12.4 is installed
- RapidsAI image `25.02-cuda12.4-devel-ubuntu24.04` is pinned

**Future**: CUDA 13 upgrade attempted after stabilizing build process (previous attempts encountered errors).

## Troubleshooting

### "CUDA Toolkit not found"
- Verify NVIDIA drivers: `nvidia-smi`
- If missing, install: `sudo apt install nvidia-driver-535 && sudo reboot`

### "cuML headers not found"
- Run setup script again: `source ./crates/worker-visualizations/setup_build.sh`
- The script will reinstall missing cuML development headers

### "Build fails with linker errors"
- Ensure the conda environment is activated: `source ./crates/worker-visualizations/setup_build.sh`
- The build must run in a shell with the environment activated; running cargo in a new shell won't work

### Docker build fails on non-GPU systems
- The build stage requires downloading large RapidsAI images
- Ensure sufficient disk space (~10-15 GB for all layers)
- The final image requires GPU hardware to run

## References

- [RAPIDS cuML Documentation](https://docs.rapids.ai/api/cuml/)
- [NVIDIA CUDA Toolkit 12.4](https://developer.nvidia.com/cuda-12-4-0)
- [RapidsAI Docker Images](https://hub.docker.com/r/nvcr.io/nvidia/rapidsai/rapidsai)