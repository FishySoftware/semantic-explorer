#!/bin/bash
# Wrapper script to run cargo with CUDA-enabled ONNX Runtime
# This uses project-relative paths, so it works for any user

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ONNX_DIR="$SCRIPT_DIR/.onnxruntime-cuda"

# Only set CUDA paths if the directory exists
if [ -d "$ONNX_DIR" ]; then
    export PKG_CONFIG_PATH="$ONNX_DIR/pkgconfig:${PKG_CONFIG_PATH:-}"
    export ORT_LIB_LOCATION="$ONNX_DIR"
    export ORT_PREFER_DYNAMIC_LINK=1
    export ORT_DYLIB_PATH="$ONNX_DIR/libonnxruntime.so"
    export LD_LIBRARY_PATH="$ONNX_DIR:${LD_LIBRARY_PATH:-}"
    export CUDA_VISIBLE_DEVICES="${CUDA_VISIBLE_DEVICES:-0}"
fi

# Load .env file if it exists
if [ -f "$SCRIPT_DIR/.env" ]; then
    set -a
    source "$SCRIPT_DIR/.env"
    set +a
fi

# Run cargo with whatever arguments were passed
exec cargo "$@"
