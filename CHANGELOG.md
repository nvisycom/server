# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- HTTP API server built on Axum and Tokio, with OpenAPI documentation and Scalar UI
- PostgreSQL data layer via Diesel (async) with deadpool connection pooling
- NATS JetStream messaging, job queues, and KV
- First-party S3-compatible blob storage for documents, audits, and avatars, encrypted at rest
- Multi-tenant workspace hierarchy (accounts → workspaces → documents)
- JWT authentication with Ed25519 (EdDSA) signing; password and OIDC (Google, Microsoft) sign-in
- Document detect/redact pipeline over a transactional Postgres work queue
- Review threads, comments, and file review assignments
- Immutable, cursor-paginated workspace activity feed with CSV/JSON export
- Configurable per-workspace and per-pipeline data retention with scheduled cleanup
- External connectors for tenant object stores (S3/Azure/GCS) and cloud file services (Drive/Dropbox)
- Webhook delivery and in-app notifications with real-time updates
- Graceful shutdown, health checks, and TLS support via the `tls` feature

### Crates

- **nvisy-cli** - Command-line interface and HTTP server binary
- **nvisy-core** - Shared foundation types and utilities
- **nvisy-postgres** - Type-safe async PostgreSQL data layer
- **nvisy-nats** - NATS client (JetStream messaging, job queues, KV)
- **nvisy-s3** - First-party S3-compatible blob storage (files, audits, avatars)
- **nvisy-object-store** - External tenant object-store providers (S3, Azure, GCS)
- **nvisy-file-service** - OAuth-based cloud file-service providers (Google Drive, Dropbox)
- **nvisy-inference** - LLM inference providers (OpenAI, Ollama, Anthropic)
- **nvisy-webhook** - Webhook delivery types and traits
- **nvisy-server** - HTTP handlers, middleware, pipeline, and services

[Unreleased]: https://github.com/nvisycom/server/commits/main
