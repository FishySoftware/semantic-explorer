#!/bin/bash
# Lightweight script to activate the cuML build environment
# Run setup_build.sh ONCE to install dependencies, then use this for subsequent runs

BLUE='\033[0;34m'
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

# Check if environment exists
if [ ! -d "$HOME/micromamba/envs/cuml" ]; then
    echo -e "${RED}[ERROR]${NC} cuML environment not found at $HOME/micromamba/envs/cuml"
    echo "Run 'source setup_build.sh' first to set up the environment"
    return 1
fi

# Set up micromamba
export MAMBA_ROOT_PREFIX="$HOME/micromamba"

if [ ! -f "$HOME/.local/bin/micromamba" ]; then
    echo -e "${RED}[ERROR]${NC} Micromamba not found"
    echo "Run 'source setup_build.sh' first to install dependencies"
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

echo -e "${GREEN}[SUCCESS]${NC} Build environment activated (cuML at ${CONDA_PREFIX})"
