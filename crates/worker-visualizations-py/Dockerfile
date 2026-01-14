# Multi-stage build for Python visualization worker
FROM python:3.12-slim AS builder

WORKDIR /build

# Install system dependencies for building
RUN apt-get update && apt-get install -y \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Copy requirements and install Python packages
COPY requirements.txt .
RUN pip install --user --no-cache-dir -r requirements.txt

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
