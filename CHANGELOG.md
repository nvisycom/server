# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **nvisy-paddle** - PaddleX HTTP API client for comprehensive document
  processing and OCR
- **nvisy-mistral** - Mistral AI OCR client for high-accuracy document
  understanding
- Added convenience functions `is_valid_nats_url()` and `dev_config()` to
  nvisy-nats
- Added comprehensive error classification methods to nvisy-nats Error type
- Added user-friendly error messages and network error detection in nvisy-nats

### Changed

- Updated nvisy-mistral README to match project documentation standards
- Improved nvisy-nats Error type with more granular retry logic and better
  categorization
- Enhanced nvisy-nats error handling with severity levels and client error
  detection

### Fixed

- Fixed nvisy-nats test compilation errors with serde_json::Error construction
- Improved retry logic in nvisy-nats to be more context-aware for different
  error types

### Removed

## [0.1.0] - 2025-01-15

### Added

- Initial release of the Nvisy API server
- High-performance HTTP server built with Axum and Tokio
- PostgreSQL database integration with Diesel ORM and connection pooling
- JWT-based authentication with RSA key pair signing
- MinIO/S3-compatible object storage integration
- OpenRouter AI service integration for document processing
- NATS messaging system integration
- Comprehensive CLI with configuration management
- Docker support with multi-stage builds
- Database migration system with embedded migrations
- OpenAPI/Swagger documentation generation
- CORS middleware with configurable origins
- Structured logging with tracing
- Graceful shutdown handling
- Health check endpoints
- TLS support (optional feature)
- Telemetry support (optional feature)

### Features

- **nvisy-server** - Core HTTP API server with Axum framework
- **nvisy-postgres** - Type-safe database layer with async connection pooling
- **nvisy-minio** - MinIO client with S3-compatible operations
- **nvisy-openrouter** - OpenRouter AI integration client
- **nvisy-paddlex** - PaddleX HTTP API client for document processing and OCR
- **nvisy-nats** - NATS messaging client
- **nvisy-cli** - Server CLI with enhanced lifecycle management

### Configuration

- Environment variable based configuration
- CLI argument parsing with clap
- Production-ready defaults
- Comprehensive validation
- Support for .env files

### Security

- JWT authentication with Ed25519 keys
- Input validation and sanitization
- SQL injection prevention through Diesel ORM
- CORS protection
- Security headers middleware
- Rate limiting support

[Unreleased]: https://github.com/nvisycom/api/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/nvisycom/api/releases/tag/v0.1.0
