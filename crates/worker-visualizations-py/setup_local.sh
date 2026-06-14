#!/bin/bash
# Local development setup script (uv-based)
# Installs dependencies and downloads fonts for local testing without Docker

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "Setting up worker-visualizations-py for local development..."
echo ""

# Ensure uv is available
if ! command -v uv >/dev/null 2>&1; then
    echo "Error: 'uv' is not installed. Install it from https://docs.astral.sh/uv/ and re-run." >&2
    exit 1
fi

# Create the virtual environment and install locked dependencies into .venv
echo "Installing Python dependencies with uv..."
uv sync --frozen

# Download fonts
echo ""
echo "Downloading fonts for offline use..."
./download_fonts.sh

# Cache JS files for datamapplot offline mode
echo ""
echo "Caching JS dependencies for datamapplot offline mode..."
uv run python -c "from datamapplot.offline_mode_caching import cache_js_files; cache_js_files(); print('  ✓ JS dependencies cached')"

echo ""
echo "✓ Development environment ready!"
echo ""
echo "To run the worker:"
echo "  uv run python src/main.py"
echo ""
echo "Or activate the environment first:"
echo "  source .venv/bin/activate"
echo "  python src/main.py"
