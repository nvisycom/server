# nvisy-server

High-performance HTTP API server for the Nvisy redaction platform, built
with Axum and Tokio.

## Overview

The core HTTP API layer implementing REST endpoints for workspaces,
pipelines, connections, files, and accounts. It provides JWT
authentication with Ed25519, role-based authorization, request
validation, a Tower middleware stack, and auto-generated OpenAPI
documentation via Aide. Depends on all other workspace crates for
persistence, messaging, storage, and webhook delivery.

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
