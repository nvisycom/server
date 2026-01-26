# Provider Architecture

Data providers enable reading from and writing to external systems (storage, databases, vector stores). This document defines the architecture for implementing providers in Python while maintaining type safety with the Rust core.

## Design Principles

1. **Rust owns the API boundary** - All HTTP schemas defined in Rust, Python conforms to them
2. **Python owns integrations** - Provider implementations leverage Python's ecosystem
3. **Type safety across the boundary** - Schemas generated from Rust, validated in Python
4. **Async-first** - No synchronous APIs, no blocking calls
5. **Minimal coupling** - Providers are independent, share only core protocols

## Architecture

```
┌────────────────────────────────────────────────────┐
│                    Rust Core                       │
│                                                    │
│  OpenAPI Schema ◄── schemars ◄── Rust Types       │
│        │                            │              │
│        ▼                            ▼              │
│  JSON Schema files            nvisy-dal traits    │
│        │                            │              │
└────────┼────────────────────────────┼──────────────┘
         │                            │
         ▼                            ▼
┌────────────────────────────────────────────────────┐
│                  Python Providers                  │
│                                                    │
│  datamodel-codegen ──► Pydantic Models            │
│                            │                       │
│                            ▼                       │
│                    Provider Protocols              │
│                            │                       │
│                            ▼                       │
│              Provider Implementations              │
│                                                    │
└────────────────────────────────────────────────────┘
```

## Schema Flow

### 1. Define in Rust

Schemas are defined once in Rust using `schemars`:

```rust
#[derive(JsonSchema, Serialize, Deserialize)]
pub struct ObjectContext {
    pub prefix: Option<String>,
    pub continuation_token: Option<String>,
    pub limit: Option<u32>,
}
```

### 2. Export to JSON Schema

Build script exports schemas to `schemas/`:

```
schemas/
├── contexts/
│   ├── object.json
│   ├── relational.json
│   └── vector.json
├── credentials/
│   ├── s3.json
│   ├── gcs.json
│   └── ...
└── datatypes/
    ├── blob.json
    ├── document.json
    └── ...
```

### 3. Generate Python Models

Python models generated from JSON Schema at build time:

```bash
uv run datamodel-codegen \
  --input schemas/ \
  --output packages/nvisy-dal-core/nvisy_dal_core/generated/
```

### 4. Validate at Runtime

Generated models used in provider implementations with Pydantic validation.

## Provider Interface

Providers implement async protocols for reading and writing data.

### Input Protocol

```python
@runtime_checkable
class DataInput(Protocol[T_co, Ctx_contra]):
    """Protocol for reading data from external sources."""
    
    async def read(self, ctx: Ctx_contra) -> AsyncIterator[T_co]:
        """Yield items from the source based on context."""
        ...
```

### Output Protocol

```python
@runtime_checkable  
class DataOutput(Protocol[T_contra, Ctx_contra]):
    """Protocol for writing data to external sinks."""
    
    async def write(self, ctx: Ctx_contra, items: Sequence[T_contra]) -> None:
        """Write a batch of items to the sink."""
        ...
```

### Provider Protocol

```python
@runtime_checkable
class Provider(Protocol[Cred, Params]):
    """Protocol for provider lifecycle management."""
    
    @classmethod
    async def connect(cls, credentials: Cred, params: Params) -> Self:
        """Establish connection to the external service."""
        ...
    
    async def disconnect(self) -> None:
        """Release resources and close connections."""
        ...
```

## Package Structure

Single package with optional dependencies per provider:

```
packages/nvisy-dal/
├── pyproject.toml
├── py.typed                    # PEP 561 marker
└── src/
    └── nvisy_dal/
        ├── __init__.py
        ├── protocols.py        # DataInput, DataOutput, Provider
        ├── errors.py           # DalError, error kinds
        ├── _generated/         # From JSON Schema (committed)
        │   ├── __init__.py
        │   ├── contexts.py
        │   └── datatypes.py
        └── providers/
            ├── __init__.py
            ├── s3.py
            ├── gcs.py
            ├── azure.py
            ├── postgres.py
            ├── mysql.py
            ├── qdrant.py
            └── pinecone.py
```

### Layout Rationale

- **Single package** - Internal code, not publishing separately to PyPI
- **`src/` layout** - Prevents accidental imports from project root during development
- **Flat providers** - One module per provider, no nested input/output structure
- **`_generated/` committed** - Reproducible builds, `_` prefix indicates internal
- **Optional deps** - `pip install nvisy-dal[s3,postgres]` for selective installation

### Dependencies

```toml
# pyproject.toml
[project]
name = "nvisy-dal"
dependencies = [
    "pydantic>=2.0",
]

[project.optional-dependencies]
s3 = ["boto3>=1.35", "types-boto3"]
gcs = ["google-cloud-storage>=2.18"]
azure = ["azure-storage-blob>=12.23"]
postgres = ["asyncpg>=0.30"]
mysql = ["aiomysql>=0.2"]
qdrant = ["qdrant-client>=1.12"]
pinecone = ["pinecone-client>=5.0"]
all = ["nvisy-dal[s3,gcs,azure,postgres,mysql,qdrant,pinecone]"]
dev = ["nvisy-dal[all]", "pytest>=8.0", "pytest-asyncio>=0.24", "moto>=5.0"]
```

## Python Standards

### Tooling

| Tool | Purpose |
|------|---------|
| `uv` | Package management, virtualenv, lockfile |
| `ruff` | Linting + formatting (replaces black, isort, flake8) |
| `pyright` | Type checking in strict mode |
| `pytest` | Testing with `pytest-asyncio` |

### Configuration

All config in `pyproject.toml`:

```toml
[project]
requires-python = ">=3.12"

[tool.ruff]
target-version = "py312"
line-length = 100

[tool.ruff.lint]
select = ["ALL"]
ignore = ["D", "ANN101", "ANN102", "COM812", "ISC001"]

[tool.ruff.lint.isort]
known-first-party = ["nvisy_dal"]

[tool.pyright]
pythonVersion = "3.12"
typeCheckingMode = "strict"

[tool.pytest.ini_options]
asyncio_mode = "auto"
asyncio_default_fixture_loop_scope = "function"
```

### Code Style

- Type hints on all public APIs
- Protocols over ABCs (structural typing)
- `Final` for constants, `ClassVar` for class attributes
- `Sequence` over `list` in parameters (covariance)
- `Mapping` over `dict` in parameters
- `async def` always, no sync wrappers
- Context managers for resource cleanup
- `structlog` for structured logging

### Error Handling

```python
from enum import StrEnum
from typing import final

class ErrorKind(StrEnum):
    """Classification of provider errors."""
    
    CONNECTION = "connection"
    NOT_FOUND = "not_found"
    INVALID_INPUT = "invalid_input"
    TIMEOUT = "timeout"
    PROVIDER = "provider"

@final
class DalError(Exception):
    """Base error for all provider operations."""
    
    __slots__ = ("message", "kind", "source")
    
    def __init__(
        self,
        message: str,
        kind: ErrorKind = ErrorKind.PROVIDER,
        source: BaseException | None = None,
    ) -> None:
        super().__init__(message)
        self.message = message
        self.kind = kind
        self.source = source
```

## PyO3 Bridge

The bridge module in `nvisy-dal` handles:

1. **Runtime management** - Python interpreter lifecycle
2. **Async bridging** - Rust futures ↔ Python coroutines
3. **Type conversion** - Via `pythonize` using shared JSON Schema
4. **Error propagation** - Python exceptions → Rust errors
5. **GIL coordination** - Release during I/O for concurrency

### Guarantees

- Provider methods are called with validated inputs (Pydantic)
- Outputs conform to expected schema (Pydantic serialization)
- Errors include Python traceback for debugging
- GIL released during all I/O operations

## Testing Strategy

### Unit Tests (Python)

- Mock external services (`moto` for AWS, `responses` for HTTP)
- Test protocol conformance
- Test error handling paths

### Integration Tests (Rust)

- Test PyO3 bridge with real Python runtime
- Verify type conversion round-trips
- Test async behavior across boundary

### Contract Tests

- Validate generated Python models against Rust schemas
- Run on CI after schema changes

## Adding a Provider

1. Define credentials/params schema in Rust (`crates/nvisy-dal/src/schemas/`)
2. Export JSON Schema (`make schemas`)
3. Regenerate Python models (`make codegen`)
4. Add optional dependency to `pyproject.toml`
5. Create provider module in `src/nvisy_dal/providers/`
6. Implement `DataInput` and/or `DataOutput` protocols
7. Add unit tests with mocked external service
8. Register in PyO3 bridge
