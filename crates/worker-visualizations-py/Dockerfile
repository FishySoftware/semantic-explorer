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

# Download Google Fonts for offline use
COPY download_fonts.sh .
RUN chmod +x download_fonts.sh && ./download_fonts.sh

# Pre-cache fonts in datamapplot's offline cache to prevent runtime web requests
# Set HOME to /build so datamapplot caches in /build/.cache
ENV HOME=/build
RUN mkdir -p /build/.cache && \
    python3 -c "from datamapplot.offline_mode_caching import cache_fonts; cache_fonts()" \
    2>&1 || echo "Note: Font caching completed or was not needed"

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

# Copy datamapplot cache from builder for offline font support
# First, ensure the cache directory exists and has correct permissions
RUN mkdir -p /home/appuser/.cache/datamapplot && \
    chown -R appuser:appgroup /home/appuser/.cache

# Copy cache files
COPY --from=builder --chown=appuser:appgroup /build/.cache/ /home/appuser/.cache/

RUN mkdir -p /tmp/matplotlib /tmp/numba_cache && chown -R appuser:appgroup /tmp

ENV TMPDIR=/tmp
ENV MPLCONFIGDIR=/tmp/matplotlib
# Ensure datamapplot uses the pre-cached fonts
ENV XDG_CACHE_HOME=/home/appuser/.cache

# Copy application code
COPY --chown=appuser:appgroup src/ .

USER appuser

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD python -c "import socket; socket.create_connection(('localhost', 8081), timeout=5)" || exit 1

# Run the worker
CMD ["python", "main.py"]
