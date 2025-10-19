# api.nvisy.com/server

High-performance HTTP API server for document processing and automation, built with Rust and Axum.

[![Rust](https://img.shields.io/badge/rust-1.70+-blue.svg)](https://www.rust-lang.org/)
[![Axum](https://img.shields.io/badge/axum-0.7+-blue.svg)](https://github.com/tokio-rs/axum)

## Features

- **High-Performance Architecture** - Built with Axum and Tokio for async request handling
- **Type-Safe API** - Compile-time validation with strong typing throughout
- **Comprehensive Security** - JWT authentication, CORS, security headers, input validation
- **Production Ready** - Graceful shutdown, connection pooling, health checks
- **Auto-Generated Documentation** - OpenAPI/Swagger specification with interactive UI
- **External Integrations** - AWS S3, Stripe payments, OpenRouter AI

## Crates

- [`tokio`](https://crates.io/crates/tokio) - Async runtime for Rust
- [`axum`](https://crates.io/crates/axum) - Web framework built on Tokio
- [`tower`](https://crates.io/crates/tower) - Middleware and service abstractions
