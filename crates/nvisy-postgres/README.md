# api.nvisy.com/postgres

High-performance, type-safe PostgreSQL database layer for the Nvisy platform,
built with Diesel and async connection pooling.

[![Rust](https://img.shields.io/badge/rust-1.89+-blue.svg)](https://www.rust-lang.org/)
[![Diesel](https://img.shields.io/badge/diesel-2.2+-green.svg)](https://diesel.rs/)
[![PostgreSQL](https://img.shields.io/badge/postgresql-17+-blue.svg)](https://www.postgresql.org/)

## Features

- **Async Connection Pooling** - High-performance connection management with
  Deadpool
- **Type-Safe Queries** - Compile-time SQL validation with Diesel ORM
- **Automatic Migrations** - Embedded migration system with rollback support
- **Comprehensive Error Handling** - Detailed error types with recovery hints
- **Production Ready** - Health checks, metrics, and observability built-in

## Crates

- [`tokio`](https://crates.io/crates/tokio) - Async runtime for Rust
- [`diesel`](https://crates.io/crates/diesel) - Safe, extensible ORM and query
  builder
- [`deadpool`](https://crates.io/crates/deadpool) - Async connection pooling
