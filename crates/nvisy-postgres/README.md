# nvisy-postgres

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

Type-safe PostgreSQL database layer for the Nvisy platform with async connection
pooling and embedded migrations.

## Features

- **Async Connection Pooling:** High-performance connection management with
  Deadpool
- **Type-Safe Queries:** Compile-time SQL validation with Diesel ORM
- **Embedded Migrations:** Automatic schema management with rollback support
- **Error Handling:** Comprehensive database error types with context
- **Production Ready:** Health checks and connection monitoring

## Key Dependencies

- `diesel`: Safe, extensible ORM and query builder for Rust
- `diesel-async`: Async support for Diesel with PostgreSQL
- `deadpool`: Async connection pooling for high-concurrency workloads

## Schema Management

Database schema is automatically generated from migrations using:

```bash
make generate-migrations
```

The generated schema is located at `src/schema.rs` and provides type-safe table
definitions for Diesel queries.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation:** [docs.nvisy.com](https://docs.nvisy.com)
- **Issues:** [GitHub Issues](https://github.com/nvisycom/server/issues)
- **Email:** [support@nvisy.com](mailto:support@nvisy.com)
- **API Status:** [nvisy.openstatus.dev](https://nvisy.openstatus.dev)
