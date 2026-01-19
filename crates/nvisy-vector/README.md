# nvisy-vector

Vector store abstraction layer for Nvisy Server.

## Supported Backends

- **Qdrant** - High-performance vector similarity search engine
- **Milvus** - Open-source vector database for AI applications
- **Pinecone** - Managed vector database service
- **pgvector** - PostgreSQL extension for vector similarity search

## Features

Enable specific backends via Cargo features:

```toml
[dependencies]
nvisy-vector = { version = "0.1", features = ["qdrant"] }
```

Available features:
- `qdrant` - Qdrant support
- `milvus` - Milvus support
- `pinecone` - Pinecone support
- `pgvector` - PostgreSQL pgvector support
- `all-backends` - All backends

## Usage

```rust
use nvisy_vector::{VectorStore, VectorStoreConfig};

// Create a store from configuration
let config = VectorStoreConfig::Qdrant(QdrantConfig::new("http://localhost:6334"));
let store = VectorStore::new(config).await?;

// Upsert vectors
store.upsert("collection", vectors).await?;

// Search for similar vectors
let results = store.search("collection", query_vector, 10).await?;
```
