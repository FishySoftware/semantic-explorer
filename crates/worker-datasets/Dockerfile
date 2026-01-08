FROM ubuntu:26.04 AS builder

ENV DEBIAN_FRONTEND=noninteractive
ENV PATH="/root/.cargo/bin:${PATH}"

# Install dependencies
RUN apt-get update && apt-get upgrade -yq && apt-get install curl build-essential cmake musl-tools musl-dev ca-certificates -yq && apt-get clean
RUN ln -s "/usr/bin/g++" "/usr/bin/musl-g++"

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

WORKDIR /usr/src/app

# Copy workspace
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build Worker
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --target x86_64-unknown-linux-musl --release -p worker-datasets

# Define a user
RUN useradd -u 10001 appuser

##### Runtime
FROM scratch

COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=builder /usr/src/app/target/x86_64-unknown-linux-musl/release/worker-datasets /worker-datasets
COPY --from=builder /etc/passwd /etc/passwd

USER appuser
ENTRYPOINT ["/worker-datasets"]

