# Nvisy.com API Server

[![rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![build](https://img.shields.io/github/actions/workflow/status/nvisycom/api/build.yml?branch=main&color=000000&style=flat-square)](https://github.com/nvisycom/api/actions/workflows/build.yml)
[![postgresql](https://img.shields.io/badge/PostgreSQL-17+-000000?style=flat-square&logo=postgresql&logoColor=white)](https://www.postgresql.org/)
[![axum](https://img.shields.io/badge/Axum-0.8+-000000?style=flat-square&logo=rust&logoColor=white)](https://github.com/tokio-rs/axum)

High-performance backend API server for the Nvisy document redaction platform, built with Rust and modern async technologies.

## Features

- **High-Performance Architecture** - Built with Axum and Tokio for exceptional async performance
- **Type-Safe Database Layer** - PostgreSQL integration with Diesel ORM and strict typing
- **Comprehensive Security** - JWT authentication, session management, and input validation
- **MinIO Storage Integration** - S3-compatible object storage for document management
- **AI-Powered Processing** - OpenRouter integration for intelligent document analysis
- **Production Ready** - Health checks, graceful shutdown, connection pooling, and observability
- **Auto-Generated Documentation** - OpenAPI/Swagger specs with interactive UI
- **Workspace Architecture** - Modular crate design for optimal code organization

## Architecture

```
api/
├── crates/
│   ├── nvisy-cli/          # HTTP server CLI
│   ├── nvisy-minio/        # MinIO/S3-compatible storage client
│   ├── nvisy-nats/         # NATS messaging integration
│   ├── nvisy-openrouter/   # OpenRouter AI client
│   ├── nvisy-postgres/     # PostgreSQL database layer
│   └── nvisy-server/       # Core HTTP API server
├── migrations/             # Database migrations
└── Cargo.toml              # Workspace configuration
```

## Quick Start

### Prerequisites

- Rust 1.89 or higher
- PostgreSQL 17 or higher
- MinIO instance or S3-compatible storage
- OpenRouter API key (for AI features)

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

| Variable                    | Description                           | Required | Default                            |
| --------------------------- | ------------------------------------- | -------- | ---------------------------------- |
| `HOST`                      | Server host address                   | No       | `127.0.0.1`                        |
| `PORT`                      | Server port number                    | No       | `3000`                             |
| `REQUEST_TIMEOUT`           | Request timeout in seconds            | No       | `30`                               |
| `SHUTDOWN_TIMEOUT`          | Graceful shutdown timeout             | No       | `30`                               |
| `POSTGRES_URL`              | PostgreSQL connection string          | Yes      | -                                  |
| `AUTH_PUBLIC_PEM_FILEPATH`  | JWT public key file path              | No       | `./public.pem`                     |
| `AUTH_PRIVATE_PEM_FILEPATH` | JWT private key file path             | No       | `./private.pem`                    |
| `OPENROUTER_API_KEY`        | OpenRouter API key for AI features   | Yes      | -                                  |
| `OPENROUTER_BASE_URL`       | OpenRouter API base URL               | No       | `https://openrouter.ai/api/v1/`    |
| `MINIO_ENDPOINT`            | MinIO server endpoint                 | No       | `localhost:9000`                   |
| `MINIO_ACCESS_KEY`          | MinIO access key                      | No       | `minioadmin`                       |
| `MINIO_SECRET_KEY`          | MinIO secret key                      | No       | `minioadmin`                       |
| `CORS_ALLOWED_ORIGINS`      | Comma-separated CORS origins          | No       | Empty (allows localhost)           |
| `CORS_MAX_AGE`              | CORS preflight cache duration        | No       | `3600`                             |
| `CORS_ALLOW_CREDENTIALS`    | Allow credentials in CORS             | No       | `true`                             |

### TLS Configuration (Optional)

When built with the `tls` feature:

| Variable         | Description                    | Required |
| ---------------- | ------------------------------ | -------- |
| `TLS_CERT_PATH`  | Path to TLS certificate (PEM)  | No       |
| `TLS_KEY_PATH`   | Path to TLS private key (PEM)  | No       |

### Configuration File

Create a `.env` file in the project root:

```bash
# Server Configuration
HOST=0.0.0.0
PORT=3000
REQUEST_TIMEOUT=30
SHUTDOWN_TIMEOUT=30

# Database
POSTGRES_URL=postgresql://postgres:postgres@localhost:5432/nvisy

# Authentication (generate with: make generate-keys)
AUTH_PUBLIC_PEM_FILEPATH=./public.pem
AUTH_PRIVATE_PEM_FILEPATH=./private.pem

# External Services
OPENROUTER_API_KEY=sk-or-v1-your-api-key
OPENROUTER_BASE_URL=https://openrouter.ai/api/v1/

# MinIO Storage
MINIO_ENDPOINT=localhost:9000
MINIO_ACCESS_KEY=minioadmin
MINIO_SECRET_KEY=minioadmin

# CORS Configuration
CORS_ALLOWED_ORIGINS=http://localhost:3000,https://app.nvisy.com
CORS_MAX_AGE=3600
CORS_ALLOW_CREDENTIALS=true
```

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

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines and contribution process.

## License

MIT License - see [LICENSE.txt](LICENSE.txt) for details.

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/api/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
- **API Status**: [status.nvisy.com](https://status.nvisy.com)
