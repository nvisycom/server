# Packages

Python packages that provide runtime implementations for the Rust crates. The
Rust core defines provider traits and data types; these packages supply the
concrete implementations that connect to external systems. The PyO3 bridge loads
them at pipeline execution time.

## nvisy-dal

Implements the data provider protocols defined by the Rust `nvisy-dal` crate.
Each provider connects to an external system and exposes async read and/or write
operations through a uniform interface.

Supported provider categories:

- Relational databases
- Object stores
- Vector databases
- Document databases
- Message queues
- Graph databases

## nvisy-rig

Implements AI model providers for the Rust `nvisy-rig` crate. Provides
completion and embedding interfaces across LLM providers.

## Development

Each package uses [uv](https://docs.astral.sh/uv/) for dependency management:

```bash
cd packages/<package>

uv sync --extra dev   # Install dependencies
uv run pytest         # Run tests
uv run basedpyright   # Type check
uv run ruff check .   # Lint
```
