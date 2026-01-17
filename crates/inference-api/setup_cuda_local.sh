#!/bin/bash
set -e

# Script to set up ONNX Runtime with CUDA for local development
# This is REQUIRED because the ort-download-binaries Cargo feature downloads CPU-only binaries
# The GPU version must be manually downloaded and configured for local CUDA acceleration

ORT_VERSION="1.23.0"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="$SCRIPT_DIR/.onnxruntime-cuda"

echo "=========================================="
echo "ONNX Runtime CUDA Setup for Local Development"
echo "=========================================="
echo ""
echo "Setting up ONNX Runtime ${ORT_VERSION} with CUDA 12.x support..."
echo ""

# Check if CUDA is available
if ! command -v nvidia-smi &> /dev/null; then
    echo "⚠️  WARNING: nvidia-smi not found. Make sure CUDA drivers are installed."
    echo ""
fi

# Check CUDA version
if command -v nvcc &> /dev/null; then
    CUDA_VERSION=$(nvcc --version | grep "release" | awk '{print $5}' | cut -d',' -f1)
    echo "Found CUDA version: $CUDA_VERSION"
    if [[ ! "$CUDA_VERSION" =~ ^12\. ]]; then
        echo "⚠️  WARNING: ONNX Runtime 1.20.1 requires CUDA 12.x, but found $CUDA_VERSION"
    fi
    echo ""
fi

# Create installation directory
mkdir -p "$INSTALL_DIR"
cd "$INSTALL_DIR"

# Download ONNX Runtime with CUDA support
if [ ! -f "libonnxruntime.so" ]; then
    echo "Downloading ONNX Runtime ${ORT_VERSION} with CUDA support..."
    wget --show-progress "https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/onnxruntime-linux-x64-gpu-${ORT_VERSION}.tgz"
    
    echo ""
    echo "Extracting..."
    tar -xzf "onnxruntime-linux-x64-gpu-${ORT_VERSION}.tgz"
    
    # Move libraries to install directory
    mv "onnxruntime-linux-x64-gpu-${ORT_VERSION}"/lib/* .
    
    # Also move include files if they exist
    if [ -d "onnxruntime-linux-x64-gpu-${ORT_VERSION}/include" ]; then
        mkdir -p include
        mv "onnxruntime-linux-x64-gpu-${ORT_VERSION}"/include/* include/
    fi
    
    # Cleanup
    rm -rf "onnxruntime-linux-x64-gpu-${ORT_VERSION}" "onnxruntime-linux-x64-gpu-${ORT_VERSION}.tgz"
    
    echo "✅ ONNX Runtime with CUDA installed to ${INSTALL_DIR}"
else
    echo "✅ ONNX Runtime already installed at ${INSTALL_DIR}"
fi

# Create pkg-config file for ort-sys to find the library
echo ""
echo "Creating pkg-config file..."
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

echo "✅ pkg-config file created"

echo ""
echo "=========================================="
echo "✅ Setup Complete!"
echo "=========================================="
echo ""
echo "ONNX Runtime with CUDA installed to: ${INSTALL_DIR}"
echo ""
echo "To build with CUDA support, use the wrapper script:"
echo "  ./run_with_cuda.sh build"
echo ""
echo "To run with CUDA support:"
echo "  ./run_with_cuda.sh run"
echo ""
echo "The wrapper automatically sets the correct paths (works for all users)."
echo "Check nvidia-smi while running to verify GPU usage."
echo ""
