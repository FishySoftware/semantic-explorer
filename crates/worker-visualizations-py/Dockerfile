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
COPY --from=builder /root/.local/lib/python3.12/site-packages/ /usr/local/lib/python3.12/site-packages
COPY --from=builder /root/.local/bin /usr/local/bin
COPY --from=builder --chown=appuser:appgroup /build/fonts /app/fonts

RUN mkdir -p /app/tmp && chown appuser:appgroup /app/tmp

ENV TMPDIR=/app/tmp
ENV MPLCONFIGDIR=/app/tmp/matplotlib

# Copy application code
COPY --chown=appuser:appgroup src/ .

USER appuser

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD python -c "import socket; socket.create_connection(('localhost', 8081), timeout=5)" || exit 1

# Run the worker
CMD ["python", "main.py"]
