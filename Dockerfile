# Build stage
FROM rust:slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create a new empty shell project
WORKDIR /app

# Copy the workspace configuration
COPY Cargo.toml Cargo.lock ./

# Copy all crate manifests
COPY crates/hl-types/Cargo.toml crates/hl-types/
COPY crates/hl-ingestion/Cargo.toml crates/hl-ingestion/
COPY crates/hl-builder-data/Cargo.toml crates/hl-builder-data/
COPY crates/hl-indexer/Cargo.toml crates/hl-indexer/
COPY crates/hl-api/Cargo.toml crates/hl-api/
COPY crates/hl-server/Cargo.toml crates/hl-server/

# Create dummy source files to build dependencies
RUN mkdir -p crates/hl-types/src && echo "pub fn dummy() {}" > crates/hl-types/src/lib.rs
RUN mkdir -p crates/hl-ingestion/src && echo "pub fn dummy() {}" > crates/hl-ingestion/src/lib.rs
RUN mkdir -p crates/hl-builder-data/src && echo "pub fn dummy() {}" > crates/hl-builder-data/src/lib.rs
RUN mkdir -p crates/hl-indexer/src && echo "pub fn dummy() {}" > crates/hl-indexer/src/lib.rs
RUN mkdir -p crates/hl-api/src && echo "pub fn dummy() {}" > crates/hl-api/src/lib.rs
RUN mkdir -p crates/hl-server/src && echo "fn main() {}" > crates/hl-server/src/main.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release --package hl-server || true

# Remove the dummy source files and cached build artifacts
RUN rm -rf crates/*/src && rm -f target/release/hl-server target/release/deps/hl_server* target/release/deps/libhl_*

# Copy the actual source code
COPY crates/ crates/

# Touch source files to ensure rebuild and build the actual binary
RUN touch crates/*/src/*.rs crates/hl-server/src/main.rs && cargo build --release --package hl-server

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN useradd -m -u 1000 appuser

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/hl-server /app/hl-server

# Change ownership
RUN chown -R appuser:appuser /app

# Switch to non-root user
USER appuser

# Expose the default port
EXPOSE 3000

# Set default environment variables
ENV NETWORK=mainnet
ENV HOST=0.0.0.0
ENV PORT=3000
ENV FILL_SOURCE=api
ENV RUST_LOG=info

# Run the binary
CMD ["./hl-server"]
