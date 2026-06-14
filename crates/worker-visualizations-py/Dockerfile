# Multi-stage build for Python visualization worker (uv-based)
FROM python:3.12-slim AS builder

# Bring in the uv binary from the official distroless image
COPY --from=ghcr.io/astral-sh/uv:0.11 /uv /uvx /bin/

WORKDIR /app

# Install system dependencies for building
RUN apt-get update && apt-get install -y \
    build-essential \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Update CA certificates to handle corporate proxies
RUN update-ca-certificates

# uv configuration:
# - compile bytecode for faster cold starts
# - copy (not hardlink) since the build cache and target live on different layers
# - use the system interpreter from the base image rather than downloading one
ENV UV_COMPILE_BYTECODE=1 \
    UV_LINK_MODE=copy \
    UV_PYTHON_DOWNLOADS=never

# Install dependencies into /app/.venv using the locked versions.
# Copy only the lock + manifest first so this layer is cached across code changes.
COPY pyproject.toml uv.lock ./
RUN --mount=type=cache,target=/root/.cache/uv \
    uv sync --frozen --no-install-project --no-dev

# Make the project venv the default interpreter for subsequent steps
ENV PATH="/app/.venv/bin:$PATH"

# Download Google Fonts for offline use (our own font files)
COPY download_fonts.sh .
RUN chmod +x download_fonts.sh && ./download_fonts.sh

# Pre-cache fonts and JS in datamapplot's offline cache format
# CRITICAL: datamapplot uses platformdirs.user_data_dir() which resolves to
# $XDG_DATA_HOME/datamapplot (falling back to ~/.local/share/datamapplot).
ENV HOME=/app
ENV XDG_DATA_HOME=/app/.local/share

RUN mkdir -p /app/.local/share/datamapplot && \
    python -c "from datamapplot.offline_mode_caching import cache_fonts, cache_js_files; print('Caching datamapplot fonts...'); cache_fonts(); print('Caching datamapplot JS files...'); cache_js_files(); print('Cache complete!')" && \
    echo "=== Datamapplot cache contents ===" && \
    ls -la /app/.local/share/datamapplot/ && \
    echo "=== Font cache size ===" && \
    du -h /app/.local/share/datamapplot/

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

# Copy the fully-resolved virtual environment from the builder.
# The venv lives at the same path (/app/.venv) in both stages so its
# interpreter symlinks and console scripts resolve correctly.
COPY --from=builder --chown=appuser:appgroup /app/.venv /app/.venv
COPY --from=builder --chown=appuser:appgroup /app/fonts/ /app/fonts/

# Copy datamapplot cache from builder for offline font/JS support
# CRITICAL: datamapplot resolves the cache via platformdirs (XDG_DATA_HOME),
# so we place it under the appuser's data home below.
RUN mkdir -p /home/appuser/.local/share/datamapplot && \
    chown -R appuser:appgroup /home/appuser/.local

# Copy the pre-built datamapplot cache files
COPY --from=builder --chown=appuser:appgroup /app/.local/share/datamapplot/ /home/appuser/.local/share/datamapplot/

# Verify cache was copied correctly
RUN echo "=== Verifying datamapplot cache in runtime image ===" && \
    ls -la /home/appuser/.local/share/datamapplot/ && \
    echo "=== Font cache file size ===" && \
    du -h /home/appuser/.local/share/datamapplot/datamapplot_fonts_encoded.json 2>/dev/null || echo "WARNING: Font cache not found!" && \
    echo "=== JS cache file size ===" && \
    du -h /home/appuser/.local/share/datamapplot/datamapplot_js_encoded.json 2>/dev/null || echo "WARNING: JS cache not found!"

RUN mkdir -p /tmp/matplotlib /tmp/numba_cache && chown -R appuser:appgroup /tmp

# Put the project venv first on PATH so `python` is the venv interpreter
ENV PATH="/app/.venv/bin:$PATH"
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
