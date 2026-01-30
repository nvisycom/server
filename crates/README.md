# Crates

The Rust workspace contains six crates. The core server logic lives in
`nvisy-server`, which depends on all other crates. `nvisy-core` is the shared
foundation with no internal dependencies.

## nvisy-cli

Server entry point and CLI configuration. Parses command-line arguments, loads
environment configuration, and bootstraps the application by initializing all
services and starting the HTTP server.

## nvisy-core

Shared types, error handling, and encryption utilities used across all crates.
Defines the `Error` and `ErrorKind` types, retry classification, and the
credential encryption primitives (HKDF-SHA256 key derivation,
XChaCha20-Poly1305 authenticated encryption).

## nvisy-nats

NATS client for real-time messaging, durable job queues, and object storage.
Wraps JetStream for persistent streams, KV store for distributed state, and
object storage for uploaded files. All operations are type-safe through generic
parameters.

## nvisy-postgres

PostgreSQL persistence layer using Diesel with async connection pooling. Defines
ORM models, query builders, and repository patterns for all database entities.
Migrations are embedded in the binary and applied automatically on startup.

## nvisy-server

HTTP API layer built on Axum. Implements REST endpoints for workspaces,
pipelines, connections, files, and accounts. Includes JWT authentication with
Ed25519, role-based authorization, request validation, security headers, and
auto-generated OpenAPI documentation via Aide.

## nvisy-webhook

Webhook delivery for external event notification. Defines traits and types for
sending HMAC-SHA256 signed HTTP callbacks on application events such as pipeline
completion and status changes.
