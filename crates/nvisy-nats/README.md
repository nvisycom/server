# api.nvisy.com/nats

Task-focused NATS client for the Nvisy platform with comprehensive JetStream
support and unified streaming infrastructure.

[![rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![nats](https://img.shields.io/badge/NATS-JetStream-000000?style=flat-square&logo=nats&logoColor=white)](https://nats.io/)
[![async-nats](https://img.shields.io/badge/async--nats-0.38+-000000?style=flat-square&logo=rust&logoColor=white)](https://github.com/nats-io/nats.rs)

## Features

- **Type-Safe Operations** - Generic KV store with compile-time type safety
- **Unified Streaming** - Jobs and real-time updates use the same stream
  infrastructure
- **Object Storage** - File and binary data storage using NATS JetStream
- **Job Processing** - Distributed background job queue with retry logic
- **Connection Management** - Automatic reconnection with exponential backoff
- **Error Handling** - Comprehensive error types with retry classification

## Key Dependencies

- `async-nats` - High-performance async NATS client with JetStream support
- `tokio` - Async runtime for connection management and streaming
- `serde` - Type-safe serialization for message payloads

## Architecture

The crate provides specialized modules for common NATS use cases:

- **Client** - Connection management and configuration
- **KV** - Type-safe Key-Value operations for sessions and caching
- **Object** - Object storage for files and binary data via JetStream
- **Stream** - Unified real-time updates and distributed job processing

All modules maintain type safety through generic parameters and provide access
to the underlying NATS client for extensibility.
