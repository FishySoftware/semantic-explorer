# Multi-stage build for Python visualization worker
FROM python:3.12-slim AS builder

WORKDIR /build

# Install system dependencies for building
RUN apt-get update && apt-get install -y \
    build-essential \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Update CA certificates to handle corporate proxies
RUN update-ca-certificates

# Copy requirements and install Python packages
# Use trusted-host flags to handle SSL inspection/self-signed certificates
COPY requirements.txt .
RUN pip install --user --no-cache-dir \
    --trusted-host pypi.org \
    --trusted-host pypi.python.org \
    --trusted-host files.pythonhosted.org \
    -r requirements.txt

# Download Google Fonts for offline use (our own font files)
COPY download_fonts.sh .
RUN chmod +x download_fonts.sh && ./download_fonts.sh

# Pre-cache fonts and JS in datamapplot's offline cache format
# CRITICAL: datamapplot uses platformdirs.user_data_dir() which resolves to ~/.local/share/datamapplot
# We need to set HOME and ensure fonts are cached there
ENV HOME=/build
ENV XDG_DATA_HOME=/build/.local/share

# Create the datamapplot data directory and cache fonts/JS
RUN mkdir -p /build/.local/share/datamapplot && \
    python3 -c "import sys; sys.path.insert(0, '/root/.local/lib/python3.12/site-packages'); from datamapplot.offline_mode_caching import cache_fonts, cache_js_files; print('Caching datamapplot fonts...'); cache_fonts(); print('Caching datamapplot JS files...'); cache_js_files(); print('Cache complete!')" && \
    echo "=== Datamapplot cache contents ===" && \
    ls -la /build/.local/share/datamapplot/ && \
    echo "=== Font cache size ===" && \
    du -h /build/.local/share/datamapplot/

# Runtime stage
FROM python:3.12-slim


ARG UID=10001
ARG GID=10001
RUN groupadd --gid ${GID} appgroup && \
    useradd --uid ${UID} --gid ${GID} --shell /bin/false --create-home appuser

WORKDIR /app

# Install runtime dependencies only
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy installed Python packages from builder
COPY --from=builder /root/.local/lib/python3.12/site-packages/ /usr/local/lib/python3.12/site-packages/
COPY --from=builder /root/.local/bin/ /usr/local/bin/
COPY --from=builder --chown=appuser:appgroup /build/fonts/ /app/fonts/

# Copy datamapplot cache from builder for offline font/JS support
# CRITICAL: datamapplot uses platformdirs.user_data_dir() which resolves to ~/.local/share/datamapplot
# We need to copy the cache to the appuser's home directory
RUN mkdir -p /home/appuser/.local/share/datamapplot && \
    chown -R appuser:appgroup /home/appuser/.local

# Copy the pre-built datamapplot cache files
COPY --from=builder --chown=appuser:appgroup /build/.local/share/datamapplot/ /home/appuser/.local/share/datamapplot/

# Verify cache was copied correctly
RUN echo "=== Verifying datamapplot cache in runtime image ===" && \
    ls -la /home/appuser/.local/share/datamapplot/ && \
    echo "=== Font cache file size ===" && \
    du -h /home/appuser/.local/share/datamapplot/datamapplot_fonts_encoded.json 2>/dev/null || echo "WARNING: Font cache not found!" && \
    echo "=== JS cache file size ===" && \
    du -h /home/appuser/.local/share/datamapplot/datamapplot_js_encoded.json 2>/dev/null || echo "WARNING: JS cache not found!"

RUN mkdir -p /tmp/matplotlib /tmp/numba_cache && chown -R appuser:appgroup /tmp

ENV TMPDIR=/tmp
ENV MPLCONFIGDIR=/tmp/matplotlib
# Set XDG_DATA_HOME so datamapplot finds the pre-cached fonts using platformdirs
ENV XDG_DATA_HOME=/home/appuser/.local/share
# Also set XDG_CACHE_HOME for any cache-based lookups
ENV XDG_CACHE_HOME=/home/appuser/.cache

# Copy application code
COPY --chown=appuser:appgroup src/ .

USER appuser

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD python -c "import socket; socket.create_connection(('localhost', 8081), timeout=5)" || exit 1

# Run the worker
CMD ["python", "main.py"]
