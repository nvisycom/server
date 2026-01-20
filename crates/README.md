# Crates

This directory contains the workspace crates for Nvisy Server.

## Core Crates

| Crate | Description |
|-------|-------------|
| `nvisy-cli` | Server entry point and CLI configuration |
| `nvisy-core` | Shared types, errors, and utilities |
| `nvisy-server` | HTTP API handlers and middleware |

## Data Layer

| Crate | Description |
|-------|-------------|
| `nvisy-postgres` | PostgreSQL ORM layer (Diesel async) |
| `nvisy-nats` | NATS client (JetStream, KV, object storage) |
| `nvisy-dal` | Data Abstraction Layer for workflow I/O |

## AI & Workflows

| Crate | Description |
|-------|-------------|
| `nvisy-rig` | AI services (chat, RAG, embeddings) |
| `nvisy-runtime` | Workflow execution engine |

## Integration

| Crate | Description |
|-------|-------------|
| `nvisy-webhook` | Webhook delivery traits and types |
