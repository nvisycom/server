# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - Unreleased

### Added

- Initial release of the Nvisy API server
- High-performance HTTP server built with Axum and Tokio
- PostgreSQL database integration with Diesel ORM and connection pooling
- JWT-based authentication with RSA key pair signing
- OpenRouter AI service integration for document processing
- NATS messaging system integration with JetStream and KV support
- PaddleX HTTP API client for document processing and OCR
- OpenAPI/Swagger documentation generation
- CORS middleware with configurable origins
- Structured logging with tracing
- Graceful shutdown handling and health check endpoints

### Features

- **nvisy-server** - Core HTTP API server with Axum framework
- **nvisy-postgres** - Type-safe database layer with async connection pooling
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

[0.1.0]: https://github.com/nvisycom/api/releases/tag/v0.1.0
