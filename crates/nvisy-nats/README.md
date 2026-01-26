# nvisy-nats

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

Task-focused NATS client for the Nvisy platform with comprehensive JetStream
support and unified streaming infrastructure.

## Features

- **Type-Safe Operations** - Generic KV store with compile-time type safety
- **Unified Streaming** - Jobs and real-time updates use the same stream
  infrastructure
- **Object Storage** - File and binary data storage using NATS JetStream
- **Job Processing** - Distributed background job queue with retry logic
- **Connection Management** - Automatic reconnection with exponential backoff
- **Error Handling** - Comprehensive error types with retry classification

## Key Dependencies

- `async-nats` - High-performance async NATS client with JetStream support
- `tokio` - Async runtime for connection management and streaming
- `serde` - Type-safe serialization for message payloads

## Architecture

The crate provides specialized modules for common NATS use cases:

- **Client** - Connection management and configuration
- **KV** - Type-safe Key-Value operations for sessions and caching
- **Object** - Object storage for files and binary data via JetStream
- **Stream** - Unified real-time updates and distributed job processing

All modules maintain type safety through generic parameters and provide access
to the underlying NATS client for extensibility.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License - see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/server/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
- **API Status**: [nvisy.openstatus.dev](https://nvisy.openstatus.dev)
