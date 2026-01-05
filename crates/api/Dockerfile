FROM ubuntu:26.04 AS builder

ENV DEBIAN_FRONTEND=noninteractive
ENV PATH="/root/.cargo/bin:${PATH}"

# Install dependencies
RUN apt-get update && apt-get upgrade -yq && apt-get install curl build-essential cmake musl-tools musl-dev -yq && apt-get clean
RUN ln -s "/usr/bin/g++" "/usr/bin/musl-g++"

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

WORKDIR /usr/src/app

# Copy workspace
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build API
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --target x86_64-unknown-linux-musl --release -p semantic-explorer

# Define a user
RUN useradd -u 10001 appuser

##### Node Builder
FROM node:25-bookworm AS node_builder

WORKDIR /usr/src

COPY semantic-explorer-ui/ ./

RUN npm install -g npm
RUN npm install
RUN npm run build

##### Runtime
FROM scratch

# Copy binary
COPY --from=builder /usr/src/app/target/x86_64-unknown-linux-musl/release/semantic-explorer /semantic-explorer
COPY --from=builder /etc/passwd /etc/passwd

# Copy UI assets
COPY --from=node_builder /usr/src/dist /semantic-explorer-ui

USER appuser
ENTRYPOINT ["/semantic-explorer"]
