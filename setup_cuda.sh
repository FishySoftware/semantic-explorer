#!/bin/bash
set -e

# Root-level CUDA setup script for semantic-explorer
# Downloads ONNX Runtime with CUDA support for both inference APIs

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ORT_VERSION="1.23.2"

echo "=========================================="
echo "Semantic Explorer CUDA Setup"
echo "=========================================="
echo ""

# Check if CUDA is available
if ! command -v nvidia-smi &> /dev/null; then
    echo "⚠️  WARNING: nvidia-smi not found."
    echo "    CUDA drivers may not be installed or configured properly."
    echo "    The setup will continue, but CUDA acceleration may not work."
    echo ""
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Setup cancelled."
        exit 1
    fi
    echo ""
else
    echo "✅ CUDA drivers detected (nvidia-smi found)"
    if command -v nvcc &> /dev/null; then
        CUDA_VERSION=$(nvcc --version | grep "release" | awk '{print $5}' | cut -d',' -f1)
        echo "✅ CUDA toolkit detected: $CUDA_VERSION"
    else
        echo "⚠️  nvcc not found (CUDA toolkit may not be in PATH)"
    fi
    echo ""
fi

# Function to setup ONNX Runtime for a given directory
setup_onnx_runtime() {
    local TARGET_DIR="$1"
    local API_NAME="$2"
    local INSTALL_DIR="$TARGET_DIR/.onnxruntime-cuda"

    echo "=========================================="
    echo "Setting up $API_NAME..."
    echo "=========================================="
    echo ""

    mkdir -p "$INSTALL_DIR"
    cd "$INSTALL_DIR"

    if [ ! -f "libonnxruntime.so" ]; then
        echo "Downloading ONNX Runtime ${ORT_VERSION} with CUDA support..."
        wget --show-progress "https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-linux-x64-gpu-${ORT_VERSION}.tgz"
        
        echo ""
        echo "Extracting..."
        tar -xzf "onnxruntime-linux-x64-gpu-${ORT_VERSION}.tgz"
        
        mv "onnxruntime-linux-x64-gpu-${ORT_VERSION}"/lib/* .
        
        if [ -d "onnxruntime-linux-x64-gpu-${ORT_VERSION}/include" ]; then
            mkdir -p include
            mv "onnxruntime-linux-x64-gpu-${ORT_VERSION}"/include/* include/
        fi
        
        rm -rf "onnxruntime-linux-x64-gpu-${ORT_VERSION}" "onnxruntime-linux-x64-gpu-${ORT_VERSION}.tgz"
        
        echo "✅ ONNX Runtime with CUDA installed"
    else
        echo "✅ ONNX Runtime already installed"
    fi

    # Create pkg-config file
    mkdir -p "$INSTALL_DIR/pkgconfig"
    cat > "$INSTALL_DIR/pkgconfig/libonnxruntime.pc" << EOF
prefix=$INSTALL_DIR
bindir=\${prefix}/bin
mandir=\${prefix}/share/man
docdir=\${prefix}/share/doc/onnxruntime
libdir=\${prefix}
includedir=\${prefix}/include/onnxruntime

Name: onnxruntime
Description: ONNX runtime
URL: https://github.com/microsoft/onnxruntime
Version: $ORT_VERSION
Libs: -L\${libdir} -lonnxruntime
Cflags: -I\${includedir}
EOF

    echo "✅ Setup complete for $API_NAME"
    echo ""
}

# Setup embedding-inference-api
setup_onnx_runtime "$SCRIPT_DIR/crates/embedding-inference-api" "embedding-inference-api"

# Setup llm-inference-api
setup_onnx_runtime "$SCRIPT_DIR/crates/llm-inference-api" "llm-inference-api"

echo "=========================================="
echo "✅ CUDA Setup Complete!"
echo "=========================================="
echo ""
echo "Next steps:"
echo ""
echo "1. Build the project with CUDA support:"
echo "   ./cargo_cuda.sh build --release"
echo ""
echo "2. Or build specific packages:"
echo "   ./cargo_cuda.sh build -p embedding-inference-api --release"
echo "   ./cargo_cuda.sh build -p llm-inference-api --release"
echo ""
echo "3. Run tests with CUDA support:"
echo "   ./cargo_cuda.sh test --workspace"
echo ""
echo "The cargo_cuda.sh wrapper automatically sets the correct CUDA paths."
echo ""
