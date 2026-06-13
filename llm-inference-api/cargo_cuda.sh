#!/bin/bash
# Build wrapper for llm-inference-api with CUDA support.
# Sets up environment variables for the app-local ONNX Runtime with CUDA.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ONNX_DIR="$SCRIPT_DIR/.onnxruntime-cuda"

if [ ! -d "$ONNX_DIR" ]; then
    echo "⚠️  WARNING: CUDA ONNX Runtime installation not found!"
    echo ""
    echo "Expected directory:"
    echo "  - $ONNX_DIR"
    echo ""
    echo "Run './setup_cuda.sh' first to download and configure ONNX Runtime with CUDA."
    echo ""
    echo "Continuing with standard build (CPU-only)..."
    echo ""
else
    export PKG_CONFIG_PATH="$ONNX_DIR/pkgconfig:${PKG_CONFIG_PATH:-}"
    export LD_LIBRARY_PATH="$ONNX_DIR:${LD_LIBRARY_PATH:-}"

    # These variables help ort-sys find the CUDA-enabled ONNX Runtime
    export ORT_LIB_LOCATION="$ONNX_DIR"
    export ORT_DYLIB_PATH="$ONNX_DIR/libonnxruntime.so"
    export ORT_PREFER_DYNAMIC_LINK=1
    export CUDA_VISIBLE_DEVICES="${CUDA_VISIBLE_DEVICES:-0}"

    echo "✅ CUDA environment configured for llm-inference-api"
    echo ""
fi

# Run cargo with all arguments passed to this script
exec cargo "$@"
