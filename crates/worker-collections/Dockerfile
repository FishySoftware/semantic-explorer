FROM ubuntu:26.04 AS chef

ENV DEBIAN_FRONTEND=noninteractive
ENV PATH="/root/.cargo/bin:${PATH}"

# Install dependencies (including ca-certificates for TLS)
RUN apt-get update && apt-get upgrade -yq && apt-get install curl build-essential cmake musl-tools musl-dev ca-certificates -yq && apt-get clean
RUN ln -s "/usr/bin/g++" "/usr/bin/musl-g++"

# Install Rust and cargo-chef for dependency caching
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
    rustup target add x86_64-unknown-linux-musl && \
    cargo install cargo-chef

WORKDIR /usr/src/app

##### Planner stage - analyze dependencies
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo chef prepare --recipe-path recipe.json

##### Builder stage - build dependencies first, then app
FROM chef AS builder

# Copy recipe and build dependencies (cached if Cargo.toml/Cargo.lock unchanged)
COPY --from=planner /usr/src/app/recipe.json recipe.json
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json -p worker-collections

# Now copy source and build (only rebuilds if source changed)
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --target x86_64-unknown-linux-musl --release -p worker-collections

# Define a user
RUN useradd -u 10001 appuser

##### Runtime
FROM scratch

# Copy binary
COPY --from=builder /usr/src/app/target/x86_64-unknown-linux-musl/release/worker-collections /worker-collections
COPY --from=builder /etc/passwd /etc/passwd

USER appuser
ENTRYPOINT ["/worker-collections"]
