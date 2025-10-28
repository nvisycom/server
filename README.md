# Nvisy.com API Server

[![rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&color=000000&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)
[![axum](https://img.shields.io/badge/Axum-0.8+-000000?style=flat-square&logo=rust&logoColor=white)](https://github.com/tokio-rs/axum)

High-performance backend API server for the Nvisy document redaction platform,
built with Rust and modern async technologies.

## Features

- **High-Performance Architecture** - Built with Axum and Tokio for exceptional
  async performance
- **Type-Safe Database Layer** - PostgreSQL integration with Diesel ORM and
  strict typing
- **Comprehensive Security** - JWT authentication, session management, and input
  validation
- **OCR Processing** - PaddleX HTTP API integration for high-accuracy document
  understanding
- **LLM Chat Assistant** - OpenRouter integration for intelligent document
  analysis and AI-powered features (optional)
- **NATS Messaging** - Real-time updates, job queues, sessions, and caching via
  NATS with JetStream and KV
- **Production Ready** - Health checks, graceful shutdown, connection pooling,
  and observability
- **Auto-Generated Documentation** - OpenAPI/Swagger specs with interactive UI
- **Workspace Architecture** - Modular crate design for optimal code
  organization

## Architecture

```
api/
├── crates/
│   ├── nvisy-cli/          # HTTP server CLI
│   ├── nvisy-nats/         # NATS client (messaging, KV, streams, queues)
│   ├── nvisy-openrouter/   # OpenRouter AI client (assistant chatbot)
│   ├── nvisy-paddlex/      # PaddleX HTTP API client (OCR)
│   ├── nvisy-postgres/     # PostgreSQL database layer
│   └── nvisy-server/       # Core HTTP API server
├── migrations/             # Database migrations
└── Cargo.toml              # Workspace configuration
```

## Quick Start

### Prerequisites

- Rust 1.89 or higher
- PostgreSQL 17 or higher
- NATS server with JetStream enabled
- PaddleX server (for OCR processing)
- OpenRouter API key (for LLM chat assistant - optional)

### Installation

```bash
# Clone the repository
git clone https://github.com/nvisycom/api.git
cd api

# Install required tools
make install-all

# Generate auth keys
make generate-keys

# Build the workspace
cargo build --release

# Run database migrations
make generate-migrations

# Start the server
cargo run --bin nvisy-cli
```

### Docker

```bash
# Build and run with Docker
docker build -t nvisy-api .
docker run -p 3000:3000 nvisy-api

# Or use docker-compose
docker-compose up -d
```

## Configuration

### Environment Variables

Configure the API server using these environment variables:

| Variable                    | Description                   | Required | Default                         |
| --------------------------- | ----------------------------- | -------- | ------------------------------- |
| `HOST`                      | Server host address           | No       | `127.0.0.1`                     |
| `PORT`                      | Server port number            | No       | `3000`                          |
| `REQUEST_TIMEOUT`           | Request timeout in seconds    | No       | `30`                            |
| `SHUTDOWN_TIMEOUT`          | Graceful shutdown timeout     | No       | `30`                            |
| `POSTGRES_URL`              | PostgreSQL connection string  | Yes      | -                               |
| `AUTH_PUBLIC_PEM_FILEPATH`  | JWT public key file path      | No       | `./public.pem`                  |
| `AUTH_PRIVATE_PEM_FILEPATH` | JWT private key file path     | No       | `./private.pem`                 |
| `PADDLEX_API_KEY`           | PaddleX service API key       | No       | -                               |
| `PADDLEX_BASE_URL`          | PaddleX service base URL      | No       | `http://localhost:8080/api/v1/` |
| `OPENROUTER_API_KEY`        | OpenRouter API key for LLM    | No       | -                               |
| `OPENROUTER_BASE_URL`       | OpenRouter API base URL       | No       | `https://openrouter.ai/api/v1/` |
| `NATS_URL`                  | NATS server URL               | No       | `nats://127.0.0.1:4222`         |
| `NATS_CLIENT_NAME`          | NATS client name              | No       | `nvisy-api`                     |
| `CORS_ALLOWED_ORIGINS`      | Comma-separated CORS origins  | No       | Empty (allows localhost)        |
| `CORS_MAX_AGE`              | CORS preflight cache duration | No       | `3600`                          |
| `CORS_ALLOW_CREDENTIALS`    | Allow credentials in CORS     | No       | `true`                          |

### TLS Configuration (Optional)

When built with the `tls` feature:

| Variable        | Description                   | Required |
| --------------- | ----------------------------- | -------- |
| `TLS_CERT_PATH` | Path to TLS certificate (PEM) | No       |
| `TLS_KEY_PATH`  | Path to TLS private key (PEM) | No       |

## API Documentation

When the server is running, access the interactive API documentation:

- **Swagger UI**: `http://localhost:3000/api/swagger`
- **Scalar UI**: `http://localhost:3000/api/scalar`
- **OpenAPI JSON**: `http://localhost:3000/api/openapi.json`
- **Health Check**: `http://localhost:3000/health`

## Testing

```bash
# Run all tests
cargo test --workspace

# Run tests with coverage
cargo tarpaulin --workspace --out Html

# Run specific crate tests
cargo test --package nvisy-server
```

### Code Quality

```bash
# Format code
cargo fmt --all

# Run linter
cargo clippy --all-targets --all-features -- -D warnings

# Security audit
cargo audit

# Check dependencies
cargo deny check
```

### Database Operations

```bash
# Generate auth keys
make generate-keys

# Run migrations and update schema
make generate-migrations

# Revert all migrations
make clear-migrations

# Create new migration
diesel migration generate migration_name
```

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for release notes and version history.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines and
contribution process.

## License

MIT License - see [LICENSE.txt](LICENSE.txt) for details.

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/api/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
- **API Status**: [status.nvisy.com](https://status.nvisy.com)
