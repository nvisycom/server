# nvisy-rig

Agent-centric AI framework for document processing, built on [Rig](https://github.com/0xPlaygrounds/rig).

## Overview

This crate provides AI-powered document processing capabilities including:
- Multi-provider AI inference (OpenAI, Anthropic, etc.)
- Document editing agents with tool use
- Streaming chat responses
- Ephemeral sessions with NATS KV storage
- RAG (Retrieval-Augmented Generation) via pgvector

## Features

- **Providers**: Multi-provider support with unified configuration
- **Sessions**: Ephemeral chat sessions with document context
- **RAG**: Vector-based semantic search for document understanding
- **Tools**: Extensible tool registry for agent capabilities
- **Edits**: Proposed edit workflow with approval policies

## Usage

```rust,ignore
use nvisy_rig::{ProviderRegistry, RigService};
use nvisy_rig::session::SessionStore;

// Configure providers
let providers = ProviderRegistry::new();

// Create the service
let tools = Default::default();
let sessions = SessionStore::new(nats_client).await?;
let service = RigService::new(providers, tools, sessions);
```

## Modules

- `provider` - AI provider configuration and registry
- `session` - Chat session management with NATS KV
- `agent` - Agent execution and prompt building
- `rag` - RAG pipeline with text splitting and vector search
- `tool` - Tool definitions and registry
- `edit` - Proposed edit operations
- `service` - High-level service API

## License

MIT
