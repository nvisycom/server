# nvisy-postgres

Type-safe PostgreSQL database layer for the Nvisy platform with async connection
pooling and embedded migrations.

[![Rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Diesel](https://img.shields.io/badge/Diesel-2.3+-000000?style=flat-square&logo=rust&logoColor=white)](https://diesel.rs/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-17+-000000?style=flat-square&logo=postgresql&logoColor=white)](https://www.postgresql.org/)

## Features

- **Async Connection Pooling** - High-performance connection management with
  Deadpool
- **Type-Safe Queries** - Compile-time SQL validation with Diesel ORM
- **Embedded Migrations** - Automatic schema management with rollback support
- **Error Handling** - Comprehensive database error types with context
- **Production Ready** - Health checks and connection monitoring

## Key Dependencies

- `diesel` - Safe, extensible ORM and query builder for Rust
- `diesel-async` - Async support for Diesel with PostgreSQL
- `deadpool` - Async connection pooling for high-concurrency workloads

## Schema Management

Database schema is automatically generated from migrations using:

```bash
make generate-migrations
```

The generated schema is located at `src/schema.rs` and provides type-safe table
definitions for Diesel queries.
