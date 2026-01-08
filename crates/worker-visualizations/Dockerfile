FROM rapidsai/base:24.12-cuda12.5-py3.12 AS builder

USER root

ENV DEBIAN_FRONTEND=noninteractive
ENV CARGO_HOME=/opt/cargo
ENV RUSTUP_HOME=/opt/rustup
ENV PATH=$PATH:$CARGO_HOME/bin

RUN apt-get update && \
    apt-get install -y build-essential pkg-config libssl-dev cmake curl clang libclang-dev && \
    rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path

# Set environment variables for cuML compilation
ENV CUML_ROOT=/opt/conda
ENV CMAKE_PREFIX_PATH=/opt/conda
ENV LD_LIBRARY_PATH=/opt/conda/lib:${LD_LIBRARY_PATH}
ENV CPATH=/opt/conda/include


WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN cargo build --target x86_64-unknown-linux-gnu --release

##### Runtime stage
FROM rapidsai/base:24.12-cuda12.5-py3.12

USER root

ENV DEBIAN_FRONTEND=noninteractive
ENV LD_LIBRARY_PATH=/opt/conda/lib:${LD_LIBRARY_PATH}

RUN useradd -m -u 10001 -G conda appuser

COPY --from=builder /usr/src/app/target/x86_64-unknown-linux-gnu/release/worker-visualizations /usr/local/bin/worker-visualizations

RUN chown appuser:appuser /usr/local/bin/worker-visualizations && \
    chmod 755 /usr/local/bin/worker-visualizations

USER appuser
WORKDIR /home/appuser

ENTRYPOINT ["/usr/local/bin/worker-visualizations"]