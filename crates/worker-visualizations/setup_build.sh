#!/bin/bash
# Unified setup script for cuml-wrapper-rs
# Checks for and installs missing dependencies, then activates the build environment
# Usage: source setup_build.sh

BLUE='\033[0;34m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running on Ubuntu
check_os() {
    if [ ! -f /etc/os-release ]; then
        log_error "Cannot determine OS. /etc/os-release not found."
        return 1
    fi
    
    . /etc/os-release
    if [[ "$ID" != "ubuntu" ]]; then
        log_warning "This script is designed for Ubuntu. You're running: $ID"
    fi
}

# Check if NVIDIA GPU is available
check_gpu() {
    if ! command -v nvidia-smi &> /dev/null; then
        log_error "nvidia-smi not found. NVIDIA GPU drivers may not be installed."
        log_info "Install NVIDIA drivers first:"
        log_info "  sudo apt update && sudo apt install nvidia-driver-535 && sudo reboot"
        return 1
    fi
    return 0
}

# Install system dependencies if missing
ensure_system_deps() {
    local missing_deps=()
    
    for cmd in cmake curl wget git pkg-config; do
        if ! command -v $cmd &> /dev/null; then
            missing_deps+=($cmd)
        fi
    done
    
    if [ ${#missing_deps[@]} -gt 0 ]; then
        log_info "Installing missing system dependencies: ${missing_deps[*]}"
        sudo apt update
        sudo apt install -y build-essential cmake curl wget git pkg-config libssl-dev ca-certificates
        log_success "System dependencies installed"
    fi
}

# Ensure CUDA Toolkit is installed
ensure_cuda() {
    if command -v nvcc &> /dev/null; then
        return 0
    fi
    
    log_warning "CUDA Toolkit not found. Installing CUDA 12.x..."
    
    # Download and install CUDA keyring
    wget -q https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2204/x86_64/cuda-keyring_1.1-1_all.deb
    sudo dpkg -i cuda-keyring_1.1-1_all.deb
    rm cuda-keyring_1.1-1_all.deb
    
    sudo apt update
    sudo apt install -y cuda-toolkit-12-4
    
    # Add to PATH if not already there
    if ! grep -q "/usr/local/cuda/bin" ~/.bashrc; then
        echo 'export PATH=/usr/local/cuda/bin:$PATH' >> ~/.bashrc
        echo 'export LD_LIBRARY_PATH=/usr/local/cuda/lib64:$LD_LIBRARY_PATH' >> ~/.bashrc
    fi
    
    export PATH=/usr/local/cuda/bin:$PATH
    export LD_LIBRARY_PATH=/usr/local/cuda/lib64:$LD_LIBRARY_PATH
    
    log_success "CUDA Toolkit installed"
}

# Ensure Rust is installed
ensure_rust() {
    if command -v cargo &> /dev/null; then
        return 0
    fi
    
    log_info "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    log_success "Rust installed"
}

# Ensure Micromamba is installed
ensure_micromamba() {
    if [ -f "$HOME/.local/bin/micromamba" ]; then
        return 0
    fi
    
    log_info "Installing Micromamba..."
    
    mkdir -p "$HOME/.local/bin"
    
    if [ "$(uname -m)" = "x86_64" ]; then
        curl -Ls https://micro.mamba.pm/api/micromamba/linux-64/latest | tar -xvj -C "$HOME/.local" bin/micromamba
    else
        log_error "Unsupported architecture: $(uname -m)"
        return 1
    fi
    
    # Initialize micromamba
    export MAMBA_ROOT_PREFIX="$HOME/micromamba"
    "$HOME/.local/bin/micromamba" shell init -s bash -p "$HOME/micromamba"
    
    log_success "Micromamba installed"
}

# Ensure cuML environment exists
ensure_cuml_env() {
    export MAMBA_ROOT_PREFIX="$HOME/micromamba"
    
    if [ ! -f "$HOME/.local/bin/micromamba" ]; then
        log_error "Micromamba not found after installation attempt"
        return 1
    fi
    
    # Initialize micromamba for this session
    eval "$("$HOME/.local/bin/micromamba" shell hook -s bash)"
    
    # Check if environment exists
    if micromamba env list | grep -q "^cuml "; then
        return 0
    fi
    
    log_info "Creating cuML environment..."
    log_info "This may take several minutes..."
    
    micromamba create -n cuml -y -c rapidsai -c conda-forge -c nvidia \
        python=3.11 \
        cuml=24.12 \
        cuda-version=12.4 \
        libcuml \
        libraft \
        libraft-headers \
        librmm \
        cmake \
        cuda-cudart-dev \
        libcublas-dev \
        libcusolver-dev \
        libcusparse-dev
    
    log_success "cuML environment created"
}

# Activate the build environment
activate_build_env() {
    export MAMBA_ROOT_PREFIX="$HOME/micromamba"
    
    if [ ! -f "$HOME/.local/bin/micromamba" ]; then
        log_error "Micromamba not found at $HOME/.local/bin/micromamba"
        return 1
    fi
    
    # Initialize shell hook
    eval "$("$HOME/.local/bin/micromamba" shell hook -s bash)"
    
    # Activate cuML environment
    micromamba activate cuml
    
    # Set environment variables for building
    export CUML_ROOT=${CONDA_PREFIX}
    export CMAKE_PREFIX_PATH=${CONDA_PREFIX}
    export LD_LIBRARY_PATH=${CONDA_PREFIX}/lib:${LD_LIBRARY_PATH}
    export CPATH=${CONDA_PREFIX}/include:${CPATH}
    export PATH=/usr/local/cuda/bin:${PATH}
    export LD_LIBRARY_PATH=/usr/local/cuda/lib64:${LD_LIBRARY_PATH}
    
    log_success "Build environment activated!"
    echo "  Environment: cuml"
    echo "  CONDA_PREFIX: ${CONDA_PREFIX}"
    echo "  CUML_ROOT: ${CUML_ROOT}"
    echo ""
    
    # Verify cuML headers
    if [ -d "${CONDA_PREFIX}/include/cuml" ]; then
        echo "✓ cuML headers found"
    else
        log_warning "cuML headers not found at ${CONDA_PREFIX}/include/cuml"
        echo "  Installing libcuml..."
        micromamba install -y -c rapidsai -c conda-forge -c nvidia libcuml
    fi
    
    echo ""
    echo "You can now build with: cargo build --release"
    echo ""
}

# Main flow
main() {
    log_info "Setting up build environment..."
    echo ""
    
    # Run checks and installations
    check_os || return 1
    
    if ! check_gpu; then
        return 1
    fi
    
    ensure_system_deps || return 1
    ensure_cuda || return 1
    ensure_rust || return 1
    ensure_micromamba || return 1
    ensure_cuml_env || return 1
    
    echo ""
    
    # Activate environment
    activate_build_env || return 1
}

main
