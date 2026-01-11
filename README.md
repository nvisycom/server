# Nvisy Server

[![Rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&color=000000&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)
[![Axum](https://img.shields.io/badge/Axum-0.8+-000000?style=flat-square&logo=rust&logoColor=white)](https://github.com/tokio-rs/axum)

High-performance backend server for the Nvisy document processing platform.

## Features

- **High-Performance** - Async HTTP server with Axum and Tokio
- **LLM Annotations** - AI-driven document edits via structured annotations
- **RAG Pipeline** - Build knowledge bases with document embeddings and semantic search
- **Real-Time Updates** - Live collaboration via NATS pub/sub and WebSocket
- **Interactive Docs** - Auto-generated OpenAPI with Scalar UI

## Architecture

```
server/
├── crates/
│   ├── nvisy-cli/         # Server binary with CLI and configuration
│   ├── nvisy-core/        # Shared types, errors, and utilities
│   ├── nvisy-nats/        # NATS client (streams, KV, object storage, jobs)
│   ├── nvisy-ollama/      # Ollama provider implementation
│   ├── nvisy-postgres/    # PostgreSQL database layer with Diesel ORM
│   ├── nvisy-rig/         # AI services (chat, RAG, embeddings)
│   ├── nvisy-server/      # HTTP handlers, middleware, pipeline, and OpenAPI
│   └── nvisy-webhook/     # Webhook delivery with HTTP client
├── migrations/            # PostgreSQL database migrations
└── Cargo.toml             # Workspace configuration
```

## Quick Start

```bash
# Install tools and generate keys
make install-all
make generate-keys

# Run database migrations
make generate-migrations

# Start the server
cargo run --features dotenv
```

## Configuration

See [.env.example](.env.example) for all available environment variables.

## API Documentation

- Scalar UI: `http://localhost:8080/api/scalar`
- OpenAPI JSON: `http://localhost:8080/api/openapi.json`
- Health Check: `POST http://localhost:8080/health`

## Docker

```bash
cd docker

# Development (with hot reload)
docker-compose -f docker-compose.dev.yml up -d

# Production
docker-compose up -d
```

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for release notes and version history.

## License

MIT License - see [LICENSE.txt](LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/server/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
- **API Status**: [nvisy.openstatus.dev](https://nvisy.openstatus.dev)
