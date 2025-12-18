# api.nvisy.com/qdrant

Type-safe Qdrant vector database client for the Nvisy platform with async connection
management and comprehensive search operations.

[![rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![qdrant](https://img.shields.io/badge/Qdrant-1.16+-000000?style=flat-square&logo=rust&logoColor=white)](https://qdrant.tech/)
[![tokio](https://img.shields.io/badge/Tokio-1.48+-000000?style=flat-square&logo=rust&logoColor=white)](https://tokio.rs/)

## Features

- **Async Connection Management** - High-performance connection handling with
  automatic reconnection
- **Type-Safe Operations** - Compile-time vector and payload validation
- **Vector Search** - Similarity search with filtering and scoring capabilities
- **Collection Management** - Create and configure Qdrant collections
- **Point Operations** - CRUD operations for vectors with metadata
- **Error Handling** - Comprehensive error types with context
- **Production Ready** - Health checks and connection monitoring

## Key Dependencies

- `qdrant-client` - Official Qdrant client for Rust with gRPC support
- `tokio` - Async runtime for high-performance concurrent operations
- `serde` - Serialization framework for type-safe data handling
