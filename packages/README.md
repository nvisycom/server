# Packages

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

This directory contains Python packages that provide provider implementations for the Rust crates.

## nvisy-dal

Data abstraction layer for external integrations. Provides unified async interfaces for storage, databases, and vector stores. The Rust `nvisy-dal` crate loads this package via PyO3 to delegate provider calls.

**Supported providers:** PostgreSQL, MySQL, S3, GCS, Azure Blob, Qdrant, Pinecone

## nvisy-rig

AI/LLM orchestration layer. Provides unified interfaces for LLM providers and agent workflows. Used by the Rust `nvisy-rig` crate for Python-based AI integrations.

**Supported providers:** OpenAI, Anthropic, Cohere

## Development

Each package uses [uv](https://docs.astral.sh/uv/) for dependency management:

```bash
cd packages/nvisy-dal

# Install dependencies
uv sync --extra dev

# Run tests
uv run pytest

# Type check
uv run basedpyright

# Lint
uv run ruff check .
```
