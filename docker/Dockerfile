# syntax=docker/dockerfile:1.9

ARG RUST_VERSION=1.83
ARG DEBIAN_VERSION=bookworm

FROM lukemathwalker/cargo-chef:latest-rust-${RUST_VERSION} AS chef
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

RUN cargo build --release --bin nvisy-server
RUN strip target/release/nvisy-server

FROM debian:${DEBIAN_VERSION}-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libpq5 \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --shell /bin/bash --uid 1000 nvisy

WORKDIR /app

COPY --from=builder /app/target/release/nvisy-server /usr/local/bin/nvisy-server

RUN chown -R nvisy:nvisy /app

LABEL org.opencontainers.image.title="Nvisy API Server" \
    org.opencontainers.image.description="Document processing and AI services API" \
    org.opencontainers.image.vendor="Nvisy" \
    org.opencontainers.image.licenses="MIT" \
    org.opencontainers.image.source="https://github.com/nvisycom/server"

EXPOSE 8080

ENV RUST_LOG=info \
    RUST_BACKTRACE=0

USER nvisy

HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD curl -sf http://localhost:8080/health || exit 1

ENTRYPOINT ["/usr/local/bin/nvisy-server"]
