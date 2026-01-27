# Crates

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)
[![Crates.io](https://img.shields.io/crates/v/nvisy-server?style=flat-square)](https://crates.io/crates/nvisy-server)
[![Docs](https://img.shields.io/docsrs/nvisy-server?style=flat-square&label=docs)](https://docs.rs/nvisy-server)

This directory contains the workspace crates for Nvisy Server.

## Core

### nvisy-cli

Server entry point and CLI configuration. Parses command-line arguments, loads environment configuration, and bootstraps the application by initializing all services and starting the HTTP server.

### nvisy-core

Shared foundation used across all crates. Contains common error types with retry support, utility functions, and base traits. Provides the `Error` and `ErrorKind` types used throughout the application.

### nvisy-server

HTTP API layer built on Axum. Implements REST endpoints for documents, workspaces, accounts, and studio sessions. Includes middleware for authentication (JWT/Ed25519), request validation, and OpenAPI documentation via Aide.

## Data Layer

### nvisy-postgres

PostgreSQL persistence layer using Diesel async. Defines ORM models, query builders, and repository patterns for all database entities. Handles connection pooling via deadpool and compile-time SQL validation.

### nvisy-nats

NATS messaging client for real-time features. Provides JetStream for durable message streams, KV store for distributed state, and object storage for large files. Used for pub/sub events and cross-service communication.

## Workflows

### nvisy-dal

Data Abstraction Layer for workflow inputs and outputs. Provides unified interfaces for reading/writing data across storage backends (S3, GCS, Azure Blob, PostgreSQL, MySQL) and vector databases (Qdrant, Pinecone, Milvus, pgvector). Defines core data types: Blob, Document, Embedding, Graph, Record, Message.

### nvisy-runtime

Workflow execution engine. Defines workflow graphs with input, transformer, and output nodes. Manages provider credentials, node execution, and data flow between pipeline stages. Integrates with nvisy-dal for storage operations.

### nvisy-rig

AI services powered by rig-core. Provides chat completions, RAG pipelines with pgvector embeddings, and document processing. Supports multiple LLM providers (OpenAI, Anthropic, OpenRouter) for studio sessions.

## Integration

### nvisy-webhook

Webhook delivery system. Defines traits and types for sending HTTP callbacks on events. Used to notify external systems about document processing completion, workflow status changes, and other application events.
