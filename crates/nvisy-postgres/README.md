# nvisy-postgres

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

Type-safe PostgreSQL persistence layer for the Nvisy platform with async
connection pooling.

## Overview

The database layer built on Diesel with `diesel-async` and Deadpool
pooling. It defines ORM models, query builders, and repository patterns
for all entities, with compile-time SQL validation. Migrations are
embedded in the binary and applied on startup; the schema is generated to
`src/schema.rs` via `make generate-migrations`.

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
