# api.nvisy.com/nats

NATS messaging client for pub/sub operations, event streaming, and distributed communication with async support.

[![rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![nats](https://img.shields.io/badge/NATS-Messaging-000000?style=flat-square&logo=nats&logoColor=white)](https://nats.io/)

## Features

- **Pub/Sub Messaging** - Publish and subscribe to NATS subjects for event-driven architecture
- **JetStream Support** - Persistent messaging with stream processing capabilities
- **Async/Await Support** - Non-blocking operations with Tokio runtime
- **Connection Management** - Automatic reconnection and connection pooling
- **Error Handling** - Comprehensive error types with connection and message context
- **Distributed Communication** - Microservice communication patterns and request-reply

## Key Dependencies

- `async-nats` - Official async NATS client for Rust
- `tokio` - Async runtime for non-blocking message handling
- `serde` - JSON serialization/deserialization for message payloads
- `futures` - Stream processing utilities for message subscriptions

