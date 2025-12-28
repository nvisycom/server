# Nvisy Server

[![Rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&color=000000&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)
[![Axum](https://img.shields.io/badge/Axum-0.8+-000000?style=flat-square&logo=rust&logoColor=white)](https://github.com/tokio-rs/axum)

High-performance backend server for the Nvisy document processing platform.

## Features

- **High-Performance** - Async HTTP server with Axum and Tokio
- **RAG Pipeline** - Build knowledge bases with document embeddings and semantic search
- **LLM Annotations** - AI-driven document edits via structured annotations
- **Real-Time Updates** - Live collaboration via NATS pub/sub and WebSocket
- **Interactive Docs** - Auto-generated OpenAPI with Scalar UI

## Optional Features

| Feature | Description |
|---------|-------------|
| **tls** | HTTPS support with rustls |
| **otel** | OpenTelemetry log filtering |
| **dotenv** | Load config from `.env` files |
| **ollama** | Ollama AI backend |
| **mock** | Mock AI services for testing |

## Architecture

```
server/
├── crates/
│   ├── nvisy-cli/       # Server binary with CLI
│   ├── nvisy-core/      # Shared types and AI service traits
│   ├── nvisy-nats/      # NATS client (streams, KV, queues)
│   ├── nvisy-ollama/    # Ollama client (embeddings, OCR, VLM)
│   ├── nvisy-postgres/  # PostgreSQL database layer
│   └── nvisy-server/    # HTTP handlers and middleware
├── migrations/          # PostgreSQL database migrations
└── Cargo.toml           # Workspace configuration
```

## Quick Start

```bash
# Install tools and generate keys
make install-all
make generate-keys

# Run database migrations
make generate-migrations

# Start the server
cargo run --features ollama,dotenv
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
