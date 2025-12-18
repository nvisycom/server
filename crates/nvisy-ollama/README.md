# api.nvisy.com/ollama

High-performance async Rust client for Ollama API services with comprehensive
model management and chat completion support.

[![rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![ollama](https://img.shields.io/badge/Ollama-API-000000?style=flat-square&logo=ollama&logoColor=white)](https://ollama.ai/)
[![reqwest](https://img.shields.io/badge/reqwest-0.12+-000000?style=flat-square&logo=rust&logoColor=white)](https://github.com/seanmonstar/reqwest)

## Features

- **Type-Safe Operations** - Strongly typed request/response structures with serde
- **Async/Await Support** - Built on tokio for high-performance async operations
- **Authentication Support** - API keys, bearer tokens, and basic auth
- **Connection Management** - Efficient HTTP client with connection pooling
- **Error Handling** - Comprehensive error types with retry classification
- **Streaming Responses** - Support for streaming chat and generation endpoints

## Key Dependencies

- `reqwest` - High-performance HTTP client with async support
- `tokio` - Async runtime for connection management and streaming
- `serde` - Type-safe serialization for request/response payloads

## Architecture

The crate provides specialized modules for Ollama API interactions:

- **Client** - Connection management and configuration
- **Config** - Type-safe configuration with builder pattern
- **Credentials** - Authentication methods for different deployment scenarios
- **Error** - Comprehensive error handling with retry logic

All modules maintain type safety through generic parameters and provide access
to the underlying HTTP client for extensibility.