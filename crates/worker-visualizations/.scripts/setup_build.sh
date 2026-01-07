#!/bin/bash
# Setup script for worker-visualizations build environment
# Installs and activates CUDA 12.4+ + cuML dependencies for building cuml-wrapper-rs
# 
# Usage: source setup_build.sh
#
# This script is for LOCAL DEVELOPMENT ONLY. Docker builds handle dependencies via RapidsAI base images.

BLUE='\033[0;34m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

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

# Activate build environment script
ACTIVATE_SCRIPT="./activate_build_env.sh"

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
}

# Install system dependencies if missing
ensure_system_deps() {
    local missing_deps=()
    local packages=(cmake curl wget git pkg-config libssl-dev ca-certificates)

    for cmd in "${packages[@]}"; do
        if ! dpkg -s "$cmd" &> /dev/null; then
            missing_deps+=("$cmd")
        fi
    done

    if [ ${#missing_deps[@]} -gt 0 ]; then
        log_info "Installing missing system dependencies: ${missing_deps[*]}"
        sudo apt update
        sudo apt install -y build-essential "${missing_deps[@]}"
        log_success "System dependencies installed"
    fi
}

# Ensure CUDA Toolkit 12.4 or later is installed
ensure_cuda() {
    local cuda_version

    if ! command -v nvcc &> /dev/null; then
        echo "[ERROR] nvcc not found in PATH. Please ensure CUDA is installed and nvcc is accessible."
        exit 1
    fi

    # Get CUDA version
    cuda_version=$(nvcc --version | grep -oP 'V\K[0-9.]+' | head -n 1)

    # Check if CUDA 12.4 or later is installed
    if [[ "$cuda_version" > 12.4* ]]; then
        log_success "CUDA 12.4 or later already installed and in PATH"
        exit 0
    else
        echo "[INFO] CUDA version $cuda_version detected. Expected 12.4 or later"
        exit 1
    fi


    log_warning "CUDA Toolkit 12.4 or later not found. Installing..."

    local temp_file="/tmp/cuda-keyring_1.1-1_all.deb"
    wget -q -O "$temp_file" https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2204/x86_64/cuda-keyring_1.1-1_all.deb || {
        log_error "Failed to download CUDA keyring file."
        return 1
    }

    sudo dpkg -i "$temp_file" || {
        log_error "Failed to install CUDA keyring file."
        rm "$temp_file"
        return 1
    }

    rm "$temp_file"
    log_success "CUDA keyring installed successfully."

    sudo apt update
    sudo apt install -y cuda-toolkit-12-4 || {
        log_error "Failed to install CUDA Toolkit 12.4."
        return 1
    }

    if ! grep -q "/usr/local/cuda/bin" ~/.bashrc; then
        echo 'export PATH=/usr/local/cuda/bin:$PATH' >> ~/.bashrc
        echo 'export LD_LIBRARY_PATH=/usr/local/cuda/lib64:$LD_LIBRARY_PATH' >> ~/.bashrc
        export PATH=/usr/local/cuda/bin:$PATH
        export LD_LIBRARY_PATH=/usr/local/cuda/lib64:$LD_LIBRARY_PATH
    fi

    log_success "CUDA Toolkit 12.4 or later installed"
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

    if [ "$(uname -m)" != "x86_64" ]; then
        log_error "Unsupported architecture: $(uname -m)"
        return 1
    fi

    curl -Ls https://micro.mamba.pm/api/micromamba/linux-64/latest | tar -xvj -C "$HOME/.local" bin/micromamba || {
        log_error "Failed to install Micromamba."
        return 1
    }

    export MAMBA_ROOT_PREFIX="$HOME/micromamba"
    "$HOME/.local/bin/micromamba" shell init -s bash -p "$MAMBA_ROOT_PREFIX"
    log_success "Micromamba installed"
}

# Ensure cuML environment exists
ensure_cuml_env() {
    export MAMBA_ROOT_PREFIX="$HOME/micromamba"

    if [ ! -f "$HOME/.local/bin/micromamba" ]; then
        log_error "Micromamba not found after installation attempt"
        return 1
    fi

    eval "$("$HOME/.local/bin/micromamba" shell hook -s bash)"

    if [ -d "$MAMBA_ROOT_PREFIX/envs/cuml" ]; then
        log_success "cuML environment already exists at $MAMBA_ROOT_PREFIX/envs/cuml"
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
        libcusparse-dev || {
        log_error "Failed to create cuML environment."
        return 1
    }

    log_success "cuML environment created"
}

# Main flow
main() {
    log_info "Setting up build environment for worker-visualizations..."
    log_info "This enables compilation of cuml-wrapper-rs dependency"
    echo ""

    check_os || return 1
    check_gpu || return 1
    ensure_system_deps || return 1
    ensure_cuda || return 1
    ensure_rust || return 1
    ensure_micromamba || return 1
    ensure_cuml_env || return 1

    # Activate environment using the separate script
    source "$ACTIVATE_SCRIPT"
}

main