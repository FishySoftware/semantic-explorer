#!/bin/bash
# Root-level build wrapper for semantic-explorer with CUDA support
# Sets up environment variables for ONNX Runtime with CUDA for both inference APIs

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Paths to CUDA ONNX Runtime installations
EMBEDDING_ONNX_DIR="$SCRIPT_DIR/crates/embedding-inference-api/.onnxruntime-cuda"
LLM_ONNX_DIR="$SCRIPT_DIR/crates/llm-inference-api/.onnxruntime-cuda"

# Check which CUDA directories exist
EMBEDDING_CUDA_READY=false
LLM_CUDA_READY=false

if [ -d "$EMBEDDING_ONNX_DIR" ]; then
    EMBEDDING_CUDA_READY=true
fi

if [ -d "$LLM_ONNX_DIR" ]; then
    LLM_CUDA_READY=true
fi

# Display status
if [ "$EMBEDDING_CUDA_READY" = false ] && [ "$LLM_CUDA_READY" = false ]; then
    echo "⚠️  WARNING: No CUDA ONNX Runtime installations found!"
    echo ""
    echo "Expected directories:"
    echo "  - $EMBEDDING_ONNX_DIR"
    echo "  - $LLM_ONNX_DIR"
    echo ""
    echo "Run './setup_cuda.sh' first to download and configure ONNX Runtime with CUDA."
    echo ""
    echo "Continuing with standard build (CPU-only)..."
    echo ""
fi

# Build combined PKG_CONFIG_PATH
PKG_CONFIG_PATHS=()
if [ "$EMBEDDING_CUDA_READY" = true ]; then
    PKG_CONFIG_PATHS+=("$EMBEDDING_ONNX_DIR/pkgconfig")
fi
if [ "$LLM_CUDA_READY" = true ]; then
    PKG_CONFIG_PATHS+=("$LLM_ONNX_DIR/pkgconfig")
fi

# Build combined LD_LIBRARY_PATH
LD_LIBRARY_PATHS=()
if [ "$EMBEDDING_CUDA_READY" = true ]; then
    LD_LIBRARY_PATHS+=("$EMBEDDING_ONNX_DIR")
fi
if [ "$LLM_CUDA_READY" = true ]; then
    LD_LIBRARY_PATHS+=("$LLM_ONNX_DIR")
fi

# Set environment variables if any CUDA installation exists
if [ ${#PKG_CONFIG_PATHS[@]} -gt 0 ]; then
    # Join paths with :
    COMBINED_PKG_CONFIG=$(IFS=:; echo "${PKG_CONFIG_PATHS[*]}")
    export PKG_CONFIG_PATH="$COMBINED_PKG_CONFIG:${PKG_CONFIG_PATH:-}"
    
    COMBINED_LD_LIBRARY=$(IFS=:; echo "${LD_LIBRARY_PATHS[*]}")
    export LD_LIBRARY_PATH="$COMBINED_LD_LIBRARY:${LD_LIBRARY_PATH:-}"
    
    # These variables help ort-sys find the CUDA-enabled ONNX Runtime
    # Note: If building both APIs, this uses the embedding API path as primary
    # but both paths are in PKG_CONFIG_PATH and LD_LIBRARY_PATH
    if [ "$EMBEDDING_CUDA_READY" = true ]; then
        export ORT_LIB_LOCATION="$EMBEDDING_ONNX_DIR"
        export ORT_DYLIB_PATH="$EMBEDDING_ONNX_DIR/libonnxruntime.so"
    elif [ "$LLM_CUDA_READY" = true ]; then
        export ORT_LIB_LOCATION="$LLM_ONNX_DIR"
        export ORT_DYLIB_PATH="$LLM_ONNX_DIR/libonnxruntime.so"
    fi
    
    export ORT_PREFER_DYNAMIC_LINK=1
    export CUDA_VISIBLE_DEVICES="${CUDA_VISIBLE_DEVICES:-0}"
    
    echo "✅ CUDA environment configured:"
    [ "$EMBEDDING_CUDA_READY" = true ] && echo "   - embedding-inference-api: CUDA enabled"
    [ "$LLM_CUDA_READY" = true ] && echo "   - llm-inference-api: CUDA enabled"
    echo ""
fi

# Run cargo with all arguments passed to this script
exec cargo "$@"
