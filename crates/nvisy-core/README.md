# nvisy-core

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

Shared foundation types for the Nvisy platform.

## Overview

The home for building blocks shared across the workspace crates, so they
reuse common types instead of duplicating their own. It provides the shared
`Error`/`ErrorKind`/`Result` types with the platform's builder and retry
conventions, a `HealthCheck` contract for aggregating component health, and an
`EndpointPolicy` that validates caller-supplied connection endpoints (SSRF and
cleartext-credential protection). Domain-specific concerns, such as a crate's
own richer error type, stay in the crate that owns them.

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
