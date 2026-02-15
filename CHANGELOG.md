# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- HTTP server with Axum and Tokio
- PostgreSQL integration with Diesel ORM and async connection pooling
- JWT authentication with RSA key signing
- NATS messaging with JetStream and KV support
- OpenAPI documentation with Scalar UI
- Graceful shutdown and health checks
- TLS support via `tls` feature
- Generic worker framework for document processing pipeline
- RAG pipeline with document embeddings and semantic search

### Crates

- **nvisy-cli** - Server binary with CLI argument parsing
- **nvisy-core** - Shared types and utilities
- **nvisy-nats** - NATS client with JetStream support
- **nvisy-postgres** - PostgreSQL database layer
- **nvisy-server** - HTTP handlers, middleware, pipeline, and services
- **nvisy-webhook** - Webhook delivery with HTTP client

[Unreleased]: https://github.com/nvisycom/server/commits/main
