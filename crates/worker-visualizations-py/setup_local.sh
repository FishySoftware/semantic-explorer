#!/bin/bash
# Local development setup script
# Downloads fonts for local testing without Docker

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Setting up worker-visualizations-py for local development..."
echo ""

# Check if virtual environment exists
if [ ! -d "$SCRIPT_DIR/venv" ]; then
    echo "Creating Python virtual environment..."
    python3 -m venv "$SCRIPT_DIR/venv"
fi

# Activate virtual environment
source "$SCRIPT_DIR/venv/bin/activate"

# Install dependencies
echo "Installing Python dependencies..."
pip install --upgrade pip > /dev/null
pip install -r "$SCRIPT_DIR/requirements.txt"

# Download fonts
echo ""
echo "Downloading fonts for offline use..."
cd "$SCRIPT_DIR"
./download_fonts.sh

# Cache JS files for datamapplot offline mode
echo ""
echo "Caching JS dependencies for datamapplot offline mode..."
python3 -c "from datamapplot.offline_mode_caching import cache_js_files; cache_js_files(); print('  ✓ JS dependencies cached')"

echo ""
echo "✓ Development environment ready!"
echo ""
echo "To activate the environment:"
echo "  source venv/bin/activate"
echo ""
echo "To run the worker:"
echo "  python src/main.py"
