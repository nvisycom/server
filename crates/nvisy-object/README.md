# nvisy-object

Object store providers and streaming read/write interfaces for the Nvisy
platform.

## Overview

A cloud-agnostic object storage layer over `object_store`, supporting S3,
Azure Blob Storage, and Google Cloud Storage behind a single
`ObjectStoreClient`. A provider factory verifies credentials and
constructs backends, while source/target stream traits integrate storage
into redaction pipelines. Content is tracked by UUIDv7 source
identifiers with content-type metadata.

## Documentation

See [`docs/`](../../docs/) for architecture, security, and API documentation.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/server/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
- **API Status**: [nvisy.openstatus.dev](https://nvisy.openstatus.dev)
