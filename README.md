# api.nvisy.com

[![rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![build](https://img.shields.io/github/actions/workflow/status/nvisycom/api/build.yml?branch=main&color=000000&style=flat-square)](https://github.com/nvisycom/api/actions/workflows/build.yml)
[![postgresql](https://img.shields.io/badge/PostgreSQL-17+-000000?style=flat-square&logo=postgresql&logoColor=white)](https://www.postgresql.org/)

Backend API server for the Nvisy document redaction platform.

## Features

- High-performance async HTTP server built with Axum
- Type-safe PostgreSQL integration with Diesel ORM
- JWT-based authentication with session management
- OpenAPI/Swagger documentation
- AWS S3 storage integration
- Stripe payment processing
- OpenRouter AI integration
- Comprehensive error handling and validation
- Production-ready with health checks and graceful shutdown

## Workspace Structure

```
api/
├── crates/
│   ├── nvisy-server/     # HTTP API server
│   └── nvisy-postgres/   # Database layer
└── Cargo.toml            # Workspace configuration
```

## Crates

### nvisy-server

High-performance HTTP API server for document processing.

**Key Dependencies:**
- `axum` - Web framework
- `tokio` - Async runtime
- `tower` - Middleware
- `serde` - Serialization
- `validator` - Input validation

### nvisy-postgres

Type-safe PostgreSQL database layer with connection pooling.

**Key Dependencies:**
- `diesel` - ORM and query builder
- `deadpool` - Async connection pooling
- `tokio` - Async runtime

## Quick Start

### Prerequisites

- Rust 1.89 or higher
- PostgreSQL 17 or higher
- AWS account (for S3 storage)
- Stripe account (for payments)
- OpenRouter API key (for AI features)

### Installation

```bash
# Clone the repository
git clone https://github.com/nvisycom/api.git
cd api

# Build all crates
cargo build --release

# Run tests
cargo test --workspace

# Run the server
cargo run --bin nvisy-server
```

### Environment Variables

Create a `.env` file in the workspace root:

```bash
# Server Configuration
HOST=127.0.0.1
EXPOSED_PORT=3000
REQUEST_TIMEOUT=30
SHUTDOWN_TIMEOUT=30

# Database
POSTGRES_CONN_URL=postgresql://user:password@localhost:5432/nvisy

# Authentication
AUTH_PUBLIC_PEM_FILEPATH=/path/to/public.pem
AUTH_PRIVATE_PEM_FILEPATH=/path/to/private.pem

# External Services
OPENROUTER_API_KEY=sk-or-v1-your-api-key
STRIPE_API_KEY=sk_test_your-key
AWS_ACCESS_KEY_ID=your-access-key
AWS_SECRET_ACCESS_KEY=your-secret-key
AWS_REGION=us-east-1
AWS_S3_BUCKET=your-bucket-name
```

### Docker

```bash
# Build image
docker build -t nvisy-api .

# Run with docker-compose
docker-compose up -d
```

## API Documentation

When the server is running, access the interactive API documentation:

- Swagger UI: `http://localhost:3000/api/swagger`
- Scalar UI: `http://localhost:3000/api/scalar`
- OpenAPI JSON: `http://localhost:3000/api/openapi.json`

## Development

### Code Quality

```bash
# Format code
cargo fmt --all

# Run linter
cargo clippy --all-targets --all-features

# Security audit
cargo audit

# Run tests with coverage
cargo tarpaulin --workspace --out Html
```

### Database Migrations

```bash
# Run migrations
diesel migration run

# Revert last migration
diesel migration revert

# Generate schema
diesel print-schema > crates/nvisy-postgres/src/schema.rs
```

## License

MIT License - see [LICENSE.txt](LICENSE.txt) for details.

## Support

- Documentation: [docs.nvisy.com](https://docs.nvisy.com)
- Issues: [GitHub Issues](https://github.com/nvisycom/api/issues)
- Email: [support@nvisy.com](mailto:support@nvisy.com)
