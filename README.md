# Nvisy Server

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)
[![Crates.io](https://img.shields.io/crates/v/nvisy-server?style=flat-square)](https://crates.io/crates/nvisy-server)
[![Docs](https://img.shields.io/docsrs/nvisy-server?style=flat-square&label=docs)](https://docs.rs/nvisy-server)

High-performance backend server for the Nvisy document processing platform.

## Features

- **High-Performance** - Async HTTP server with Axum and Tokio
- **LLM Annotations** - AI-driven document edits via structured annotations
- **RAG Pipeline** - Build knowledge bases with document embeddings and semantic search
- **Real-Time Updates** - AI streaming via SSE and job processing via NATS
- **Interactive Docs** - Auto-generated OpenAPI with Scalar UI

## Architecture

```
server/
├── crates/
│   ├── nvisy-cli/         # Server binary with CLI and configuration
│   ├── nvisy-core/        # Shared types, errors, and utilities
│   ├── nvisy-nats/        # NATS client (streams, KV, object storage, jobs)
│   ├── nvisy-postgres/    # PostgreSQL database layer with Diesel ORM
│   ├── nvisy-rig/         # AI services (chat, RAG, embeddings)
│   ├── nvisy-server/      # HTTP handlers, middleware, pipeline, and OpenAPI
│   └── nvisy-webhook/     # Webhook delivery with HTTP client
├── migrations/            # PostgreSQL database migrations
└── Cargo.toml             # Workspace configuration
```

## Quick Start

```bash
# Install tools and make scripts executable
make install-all

# Generate keys, env and migration files
make generate-all

# Start the server with dotenv feature
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

Apache 2.0 License - see [LICENSE.txt](LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/server/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
- **API Status**: [nvisy.openstatus.dev](https://nvisy.openstatus.dev)
