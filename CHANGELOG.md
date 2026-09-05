# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- HTTP server with Axum and Tokio
- PostgreSQL integration with Diesel ORM and async connection pooling
- JWT authentication with Ed25519 (EdDSA) signing
- NATS messaging with JetStream and KV support
- S3-compatible blob storage for first-party files, encrypted at rest
- OpenAPI documentation with Scalar UI
- Graceful shutdown and health checks
- TLS support via `tls` feature
- Detect/redact pipeline over a transactional Postgres work queue

### Crates

- **nvisy-cli** - Server binary with CLI argument parsing
- **nvisy-core** - Shared types and utilities
- **nvisy-nats** - NATS client (messaging, job queues, KV)
- **nvisy-s3** - First-party blob storage over an S3-compatible backend
- **nvisy-object** - Client for external tenant object stores
- **nvisy-inference** - LLM inference provider configuration and clients
- **nvisy-postgres** - PostgreSQL database layer
- **nvisy-server** - HTTP handlers, middleware, pipeline, and services
- **nvisy-webhook** - Webhook delivery with HTTP client

[Unreleased]: https://github.com/nvisycom/server/commits/main
