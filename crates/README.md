# Crates

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

The Rust workspace contains five crates. The core server logic lives in
`nvisy-server`, which depends on all other crates.

## nvisy-cli

Server entry point and CLI configuration. Parses command-line arguments, loads
environment configuration, and bootstraps the application by initializing all
services and starting the HTTP server.

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
