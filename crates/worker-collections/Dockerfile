FROM ubuntu:26.04 AS builder

ENV DEBIAN_FRONTEND=noninteractive
ENV PATH="/root/.cargo/bin:${PATH}"

# Install dependencies
RUN apt-get update && apt-get upgrade -yq && apt-get install curl build-essential cmake musl-tools musl-dev -yq && apt-get clean
RUN ln -s "/usr/bin/g++" "/usr/bin/musl-g++"

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /usr/src/app

# Copy only dependency files first for better caching
COPY Cargo.toml Cargo.lock ./
COPY crates/core/Cargo.toml ./crates/core/
COPY crates/api/Cargo.toml ./crates/api/
COPY crates/worker-collections/Cargo.toml ./crates/worker-collections/
COPY crates/worker-datasets/Cargo.toml ./crates/worker-datasets/

# Create dummy source files to build dependencies
RUN mkdir -p crates/core/src && echo "fn main() {}" > crates/core/src/lib.rs
RUN mkdir -p crates/api/src && echo "fn main() {}" > crates/api/src/main.rs
RUN mkdir -p crates/worker-collections/src && echo "fn main() {}" > crates/worker-collections/src/main.rs
RUN mkdir -p crates/worker-datasets/src && echo "fn main() {}" > crates/worker-datasets/src/main.rs

# Build dependencies only (this layer will be cached)
RUN cargo build --target x86_64-unknown-linux-musl --release -p worker-collections || true

# Copy actual source code
COPY crates ./crates

# Build Worker with real source (only this rebuilds when source changes)
RUN touch crates/worker-collections/src/main.rs && \
    cargo build --target x86_64-unknown-linux-musl --release -p worker-collections

# Define a user
RUN useradd -u 10001 appuser

##### Runtime
FROM scratch

# Copy binary
COPY --from=builder /usr/src/app/target/x86_64-unknown-linux-musl/release/worker-collections /worker-collections
COPY --from=builder /etc/passwd /etc/passwd

USER appuser
ENTRYPOINT ["/worker-collections"]
