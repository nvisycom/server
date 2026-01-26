# nvisy-dal

Data abstraction layer for external integrations. Provides unified async interfaces for storage, databases, and vector stores.

## Installation

```bash
# Core package
uv add nvisy-dal

# With specific providers
uv add "nvisy-dal[postgres,s3,pinecone]"

# All providers
uv add "nvisy-dal[all]"
```

## Available Providers

| Provider | Extra | Description |
|----------|-------|-------------|
| PostgreSQL | `postgres` | Relational database via asyncpg |
| MySQL | `mysql` | Relational database via aiomysql |
| S3 | `s3` | Object storage (AWS S3, MinIO) |
| GCS | `gcs` | Google Cloud Storage |
| Azure Blob | `azure` | Azure Blob Storage |
| Qdrant | `qdrant` | Vector database |
| Pinecone | `pinecone` | Vector database |

## Usage

```python
from nvisy_dal import Provider, DataInput, DataOutput
from nvisy_dal.providers.postgres import PostgresProvider, PostgresCredentials, PostgresParams

# Connect to provider
provider = await PostgresProvider.connect(
    credentials=PostgresCredentials(
        host="localhost",
        port=5432,
        user="postgres",
        password="password",
        database="mydb",
    ),
    params=PostgresParams(table="users"),
)

# Read data
async for record in provider.read(ctx):
    print(record)

# Write data
await provider.write(ctx, records)

# Disconnect
await provider.disconnect()
```

## Architecture

This package is the Python half of the nvisy DAL system:

- **Rust (nvisy-dal crate)**: Streaming, observability, unified interface, server integration
- **Python (nvisy-dal package)**: Provider implementations, client libraries, external integrations

The Rust layer loads this package via PyO3 to delegate actual provider calls to Python.

## Protocols

All providers implement these core protocols:

```python
class Provider(Protocol[Cred, Params]):
    @classmethod
    async def connect(cls, credentials: Cred, params: Params) -> Self: ...
    async def disconnect(self) -> None: ...

class DataInput(Protocol[T, Ctx]):
    async def read(self, ctx: Ctx) -> AsyncIterator[T]: ...

class DataOutput(Protocol[T, Ctx]):
    async def write(self, ctx: Ctx, items: Sequence[T]) -> None: ...
```

## Development

```bash
# Install dev dependencies
uv sync --extra dev

# Run tests
uv run pytest

# Type check
uv run pyright

# Lint
uv run ruff check .
```

## TODO

- [x] Core protocols and error types

### Relational Databases
- [ ] PostgreSQL provider
- [ ] MySQL provider
- [ ] SQLite provider
- [ ] SQL Server provider
- [ ] Oracle provider

### Object Storage
- [ ] S3 provider
- [ ] GCS provider
- [ ] Azure Blob provider
- [ ] MinIO provider
- [ ] Cloudflare R2 provider

### Vector Databases
- [ ] Pinecone provider
- [ ] Qdrant provider
- [ ] Weaviate provider
- [ ] Milvus provider
- [ ] Chroma provider
- [ ] pgvector provider

### Document Databases
- [ ] MongoDB provider
- [ ] DynamoDB provider
- [ ] Firestore provider
- [ ] CouchDB provider

### Key-Value Stores
- [ ] Redis provider
- [ ] Memcached provider
- [ ] etcd provider

### Message Queues
- [ ] Kafka provider
- [ ] RabbitMQ provider
- [ ] NATS provider
- [ ] SQS provider

### Graph Databases
- [ ] Neo4j provider
- [ ] Neptune provider

### Search Engines
- [ ] Elasticsearch provider
- [ ] OpenSearch provider
- [ ] Algolia provider
