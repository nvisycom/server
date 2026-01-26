# nvisy-dal

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

Data Abstraction Layer for workflow inputs and outputs.

## Overview

This crate provides a unified interface for reading and writing data across various storage backends. It supports blob storage, relational databases, and vector databases.

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

## Streams

The DAL uses wrapped stream types for better ergonomics with pagination support and streaming I/O operations.

## Usage

The DAL provides a consistent interface across all provider types. Create a provider with appropriate credentials and configuration, then use the `DataInput` and `DataOutput` traits for reading and writing data with proper context and stream handling.

## Traits

### DataInput

Provides async read operations that return paginated streams of data.

### DataOutput

Provides async write operations for batches of data items.

## Context

The `Context` struct provides configuration for read/write operations including target specification (collection, table, bucket prefix), pagination cursors, and data limits.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License - see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/server/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
- **API Status**: [nvisy.openstatus.dev](https://nvisy.openstatus.dev)
