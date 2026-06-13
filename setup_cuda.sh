#!/bin/bash
set -e

# Root-level CUDA setup convenience wrapper for semantic-explorer.
#
# The inference APIs are standalone apps at the repository root, each excluded
# from the main cargo workspace and each carrying its own setup_cuda.sh /
# cargo_cuda.sh. This wrapper just runs both apps' setup scripts so you can
# provision ONNX Runtime with CUDA for everything in one command.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "=========================================="
echo "Semantic Explorer CUDA Setup"
echo "=========================================="
echo ""
echo "Delegating to the per-app setup scripts..."
echo ""

"$SCRIPT_DIR/embedding-inference-api/setup_cuda.sh"
"$SCRIPT_DIR/llm-inference-api/setup_cuda.sh"

echo "=========================================="
echo "✅ CUDA Setup Complete for both inference APIs!"
echo "=========================================="
echo ""
echo "Build each app with CUDA support from its own directory:"
echo ""
echo "   ( cd embedding-inference-api && ./cargo_cuda.sh build --release )"
echo "   ( cd llm-inference-api      && ./cargo_cuda.sh build --release )"
echo ""
echo "Each app's cargo_cuda.sh wrapper sets the correct CUDA paths."
echo ""
