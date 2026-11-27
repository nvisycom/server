# syntax=docker/dockerfile:1.9

ARG RUST_VERSION=1.99.1
ARG DEBIAN_VERSION=bookworm

FROM lukemathwalker/cargo-chef:latest-rust-${RUST_VERSION}-${DEBIAN_VERSION} AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    libpq-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .

RUN cargo build --release --bin nvisy-cli
RUN strip target/release/nvisy-cli

FROM debian:${DEBIAN_VERSION}-slim AS runtime

# Apply the base image's pending security updates first, so a fix Debian has
# shipped since the base was published lands in every build.
RUN apt-get update && apt-get upgrade -y \
    && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libpq5 \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --shell /bin/bash --uid 1000 nvisy

WORKDIR /app

COPY --from=builder /app/target/release/nvisy-cli /usr/local/bin/nvisy-cli

RUN chown -R nvisy:nvisy /app

LABEL org.opencontainers.image.title="Nvisy API Server" \
    org.opencontainers.image.description="Document processing and AI services API" \
    org.opencontainers.image.vendor="Nvisy" \
    org.opencontainers.image.licenses="Apache-2.0" \
    org.opencontainers.image.source="https://github.com/nvisycom/server"

EXPOSE 8080

ENV HOST=0.0.0.0 \
    RUST_LOG=info \
    RUST_BACKTRACE=0

USER nvisy

HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD curl -sf http://localhost:8080/health/ready || exit 1

ENTRYPOINT ["/usr/local/bin/nvisy-cli"]
