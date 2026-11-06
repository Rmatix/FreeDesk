# ---------------------------------------------------
# Stage 1: Build FreeDesk Relay Server in Rust
# ---------------------------------------------------
FROM rust:1-slim-bookworm AS builder

WORKDIR /usr/src/freedesk

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates/core ./crates/core
COPY crates/relay ./crates/relay

# Exclude client desktop GUI app from the cloud relay build (speeds up build 10x)
RUN sed -i '/"crates\/app",/d' Cargo.toml

# Compile only the relay server binary in Release mode
RUN cargo build --release --bin freedesk-relay

# ---------------------------------------------------
# Stage 2: Ultra-lightweight runtime image
# ---------------------------------------------------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/freedesk/target/release/freedesk-relay /usr/local/bin/freedesk-relay

ENV PORT=10000
EXPOSE 10000

ENTRYPOINT ["/usr/local/bin/freedesk-relay"]
