# Crates

The Rust workspace contains nine crates. The core server logic lives in
`nvisy-server`, which depends on all other crates. `nvisy-core` is the shared
foundation with no internal dependencies. `nvisy-dal` and `nvisy-rig` bridge to
Python packages in `../packages/` via PyO3 for provider implementations.

## nvisy-cli

Server entry point and CLI configuration. Parses command-line arguments, loads
environment configuration, and bootstraps the application by initializing all
services and starting the HTTP server.

## nvisy-core

Shared types, error handling, and encryption utilities used across all crates.
Defines the `Error` and `ErrorKind` types, retry classification, and the
credential encryption primitives (HKDF-SHA256 key derivation,
XChaCha20-Poly1305 authenticated encryption).

## nvisy-dal

Data abstraction layer for workflow inputs and outputs. Defines the core
provider traits (`Provider`, `DataInput`, `DataOutput`), typed data
representations (objects, records, embeddings, documents, messages, graphs), and
the PyO3 bridge that loads Python provider implementations at runtime.

## nvisy-nats

NATS client for real-time messaging, durable job queues, and object storage.
Wraps JetStream for persistent streams, KV store for distributed state, and
object storage for uploaded files. All operations are type-safe through generic
parameters.

## nvisy-postgres

PostgreSQL persistence layer using Diesel with async connection pooling. Defines
ORM models, query builders, and repository patterns for all database entities.
Migrations are embedded in the binary and applied automatically on startup.

## nvisy-rig

AI service integration for completion and embedding model providers. Built on
rig-core with support for multiple LLM backends. Provides chat sessions with
document context, RAG pipelines with pgvector, and streaming responses.

## nvisy-runtime

Workflow compiler and execution engine. Compiles pipeline definitions (JSON
graphs of source, transform, sink, and switch nodes) into optimized runtime
graphs using petgraph. Executes compiled graphs with streaming, item-at-a-time
processing and per-item resumption context.

## nvisy-server

HTTP API layer built on Axum. Implements REST endpoints for workspaces,
pipelines, connections, files, and accounts. Includes JWT authentication with
Ed25519, role-based authorization, request validation, security headers, and
auto-generated OpenAPI documentation via Aide.

## nvisy-webhook

Webhook delivery for external event notification. Defines traits and types for
sending HMAC-SHA256 signed HTTP callbacks on application events such as pipeline
completion and status changes.
