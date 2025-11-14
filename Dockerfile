# syntax=docker/dockerfile:1.7

# Build stage with cargo-chef for better dependency caching
FROM lukemathwalker/cargo-chef:latest-rust-1.75 AS chef
WORKDIR /app

# Plan stage - analyze dependencies
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Build stage
FROM chef AS builder

# Install system dependencies for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    musl-tools \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Add musl target for smaller, static binaries
RUN rustup target add x86_64-unknown-linux-musl

# Build dependencies first (this layer will be cached)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

# Build the application
COPY . .

# Set build-time environment variables
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc
ENV CC_x86_64_unknown_linux_musl=musl-gcc
ENV CXX_x86_64_unknown_linux_musl=musl-g++

# Build the binary
RUN cargo build --release --target x86_64-unknown-linux-musl --bin nvisy-server

# Strip the binary to reduce size
RUN strip target/x86_64-unknown-linux-musl/release/nvisy-server

# Copy config files if they exist
RUN mkdir -p /tmp/config && \
    (cp -r config* /tmp/config/ 2>/dev/null || true)

# Runtime stage - use distroless for security
FROM gcr.io/distroless/static-debian12:nonroot

# Create necessary directories
WORKDIR /app

# Copy CA certificates for HTTPS requests
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Copy the binary from builder stage
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/nvisy-server /usr/local/bin/nvisy-server

# Copy configuration files from builder if they exist
COPY --from=builder --chown=nonroot:nonroot /tmp/config /app/config

# Set labels for metadata
LABEL org.opencontainers.image.title="Nvisy API Server" \
    org.opencontainers.image.description="High-performance API server for document processing" \
    org.opencontainers.image.vendor="Nvisy" \
    org.opencontainers.image.licenses="MIT" \
    org.opencontainers.image.source="https://github.com/nvisy/api" \
    org.opencontainers.image.documentation="https://github.com/nvisy/api/blob/main/README.md" \
    org.opencontainers.image.version="0.1.0"

# Expose ports
EXPOSE 8080

# Set runtime environment variables
ENV RUST_LOG=info \
    RUST_BACKTRACE=0 \
    APP_ENV=production \
    HTTP_PORT=8080

# Switch to non-root user (distroless nonroot user)
USER nonroot

# Health check using the binary itself
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/nvisy-server", "--health-check"]

# Use exec form for better signal handling
ENTRYPOINT ["/usr/local/bin/nvisy-server"]
