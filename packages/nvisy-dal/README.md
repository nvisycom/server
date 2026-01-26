# nvisy-dal

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

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

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License - see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/server/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
- **API Status**: [nvisy.openstatus.dev](https://nvisy.openstatus.dev)
