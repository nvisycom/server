# nvisy-dal

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

Data Abstraction Layer for workflow inputs and outputs.

This crate provides a unified interface for reading and writing data
across various storage backends.

## Architecture

The DAL is split into two parts:
- **Rust**: Streaming, observability, unified interface, server integration
- **Python**: Provider implementations, client libraries, external integrations

## Modules

- **`contexts`** - Pagination context types for resumable streaming
- **`datatypes`** - Data types that flow through providers
- **`params`** - Provider parameters for read/write operations
- **`streams`** - Async stream types for input/output
- **`provider`** - Storage and database provider implementations

## Data Types

All types implement the `DataType` marker trait:

- **Object** - Binary data with path and content type
- **Document** - JSON content with metadata
- **Embedding** - Vector with metadata
- **Record** - Key-value column map
- **Message** - Payload with headers
- **Graph** - Nodes and edges

## Core Traits

- **`DataInput`** - Async read operations returning streams of resumable items
- **`DataOutput`** - Async write operations for batches
- **`Provider`** - Connection lifecycle management (from `nvisy-core`)

## Resumable Streaming

Each read item is paired with a context for resumption, enabling efficient recovery if streaming is interrupted.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License - see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/server/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
- **API Status**: [nvisy.openstatus.dev](https://nvisy.openstatus.dev)
