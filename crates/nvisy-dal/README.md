# nvisy-dal

Data Abstraction Layer for workflow inputs and outputs.

## Overview

This crate provides a unified interface for reading and writing data across various storage backends. It supports blob storage (S3, GCS, Azure Blob), relational databases (PostgreSQL, MySQL), and vector databases (Qdrant, Pinecone, Milvus, pgvector).

## Modules

- **`context`** - Context types for data operations (target, cursor, limit)
- **`datatype`** - Data types that flow through the DAL (Blob, Document, Embedding, Record, Graph, Message)
- **`provider`** - Storage and database providers
- **`stream`** - Stream types (`InputStream`, `OutputStream`) wrapping `BoxStream`
- **`traits`** - Core traits (`DataInput`, `DataOutput`)

## Data Types

All types implement the `DataType` marker trait:

- **Blob** - Binary data with path and optional content type
- **Document** - Structured documents with title, content, and metadata
- **Embedding** - Vector embeddings with metadata for similarity search
- **Record** - Tabular data as key-value maps
- **Graph** - Graph structures with nodes and edges
- **Message** - Messages for queue-based systems

## Providers

### Storage Providers (OpenDAL-based)

| Provider | Config | Data Type |
|----------|--------|-----------|
| `S3Provider` | `S3Config` | `Blob` |
| `GcsProvider` | `GcsConfig` | `Blob` |
| `AzblobProvider` | `AzblobConfig` | `Blob` |

### Database Providers (OpenDAL-based)

| Provider | Config | Data Type |
|----------|--------|-----------|
| `PostgresProvider` | `PostgresConfig` | `Record` |
| `MysqlProvider` | `MysqlConfig` | `Record` |

### Vector Providers

| Provider | Config | Data Type |
|----------|--------|-----------|
| `QdrantProvider` | `QdrantConfig` | `Embedding` |
| `PineconeProvider` | `PineconeConfig` | `Embedding` |
| `MilvusProvider` | `MilvusConfig` | `Embedding` |
| `PgVectorProvider` | `PgVectorConfig` | `Embedding` |

## Streams

The DAL uses wrapped stream types for better ergonomics:

```rust
use nvisy_dal::stream::{InputStream, OutputStream, ItemStream};

// InputStream wraps a BoxStream with optional pagination cursor
let input: InputStream<Blob> = provider.read(&ctx).await?;
let cursor = input.cursor(); // Get pagination cursor

// OutputStream wraps a Sink for streaming writes
```

## Usage

### Storage Example

```rust
use nvisy_dal::{Context, DataInput, DataOutput};
use nvisy_dal::provider::{S3Config, S3Provider};
use nvisy_dal::datatype::Blob;
use futures::StreamExt;

// Create provider
let config = S3Config::new("my-bucket", "us-east-1")
    .with_credentials("access_key", "secret_key");
let provider = S3Provider::new(&config)?;

// Read blobs
let ctx = Context::new().with_target("data/");
let mut stream = provider.read(&ctx).await?;

while let Some(result) = stream.next().await {
    let blob = result?;
    println!("Read: {}", blob.path);
}

// Write blobs
let blob = Blob::new("output/file.txt", b"Hello, world!".to_vec());
provider.write(&ctx, vec![blob]).await?;
```

### Database Example

```rust
use nvisy_dal::{Context, DataInput, DataOutput};
use nvisy_dal::provider::{PostgresConfig, PostgresProvider};
use nvisy_dal::datatype::Record;

// Create provider
let config = PostgresConfig::new("postgresql://user:pass@localhost/db")
    .with_table("my_table");
let provider = PostgresProvider::new(&config)?;

// Read records
let ctx = Context::new();
let stream = provider.read(&ctx).await?;

// Write records
let record = Record::new()
    .set("name", "Alice")
    .set("age", 30);
provider.write(&ctx, vec![record]).await?;
```

### Vector Example

```rust
use nvisy_dal::{Context, DataOutput};
use nvisy_dal::provider::{QdrantConfig, QdrantProvider};
use nvisy_dal::datatype::Embedding;

// Create provider
let config = QdrantConfig::new("http://localhost:6334");
let provider = QdrantProvider::new(&config).await?;

// Write embeddings
let ctx = Context::new().with_target("my_collection");
let embedding = Embedding::new("doc1", vec![0.1, 0.2, 0.3]);
provider.write(&ctx, vec![embedding]).await?;

// Search (provider-specific method)
let results = provider.search(
    "my_collection",
    vec![0.1, 0.2, 0.3],
    10,
    true,  // include_vectors
    true,  // include_metadata
    None,  // filter
).await?;
```

## Traits

### DataInput

```rust
#[async_trait]
pub trait DataInput<T: DataType>: Send + Sync {
    async fn read(&self, ctx: &Context) -> Result<InputStream<'static, T>>;
}
```

### DataOutput

```rust
#[async_trait]
pub trait DataOutput<T: DataType>: Send + Sync {
    async fn write(&self, ctx: &Context, items: Vec<T>) -> Result<()>;
}
```

## Context

The `Context` struct provides configuration for read/write operations:

```rust
let ctx = Context::new()
    .with_target("my_collection")  // Collection, table, bucket prefix, etc.
    .with_cursor("abc123")         // Pagination cursor
    .with_limit(100);              // Maximum items to read
```

## License

MIT
