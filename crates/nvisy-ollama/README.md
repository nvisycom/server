# nvisy-ollama

Ollama client for the Nvisy platform providing embeddings, OCR, and vision
language model capabilities.

[![Rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Ollama](https://img.shields.io/badge/Ollama-API-000000?style=flat-square&logo=ollama&logoColor=white)](https://ollama.ai/)

## Features

- **Embedding Generation** - Text and image vectorization for semantic search
- **OCR Processing** - Document text extraction via vision models
- **Vision Language Models** - Multimodal AI for document understanding
- **Connection Management** - Efficient HTTP client with connection pooling
- **Error Handling** - Comprehensive error types with retry classification

## Key Dependencies

- `ollama-rs` - Rust client library for Ollama API
- `nvisy-core` - Shared AI service traits and types
- `tokio` - Async runtime for concurrent operations

## Architecture

- **Client** - Connection management and configuration
- **Provider** - Implementations of nvisy-core service traits
- **Error** - Comprehensive error handling with retry logic
