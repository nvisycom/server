# nvisy-object

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

Object store providers and streaming read/write interfaces for the Nvisy
platform. Supports S3, Azure Blob Storage, and Google Cloud Storage.

## Features

- **Provider Abstraction** - Unified trait for credential verification and client construction
- **Streaming I/O** - Source and target stream traits for pipeline integration
- **Multi-Cloud** - S3, Azure Blob Storage, and Google Cloud Storage backends
- **Content Tracking** - UUIDv7-based content source identifiers with content-type metadata

## Key Dependencies

- `object_store` - Cloud-agnostic object storage (S3, Azure, GCS)
- `tokio` - Async runtime for streaming operations
- `serde` - Type-safe serialization for credentials and parameters

## Architecture

The crate provides specialized modules for object storage:

- **Client** - Unified `ObjectStoreClient` wrapping `Arc<dyn ObjectStore>`
- **Providers** - Factory trait with S3, Azure, and GCS implementations
- **Streams** - Source/target stream traits with object store adapters
- **Types** - Self-contained `Error`, `ContentData`, and `ContentSource` types

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License - see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/server/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
- **API Status**: [nvisy.openstatus.dev](https://nvisy.openstatus.dev)
