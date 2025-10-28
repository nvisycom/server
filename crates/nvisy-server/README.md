# api.nvisy.com/server

High-performance HTTP API server for the Nvisy document redaction platform,
built with Axum and Tokio.

[![rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![axum](https://img.shields.io/badge/Axum-0.8+-000000?style=flat-square&logo=rust&logoColor=white)](https://github.com/tokio-rs/axum)

## Features

- **Async HTTP Server** - Built with Axum web framework on Tokio runtime
- **JWT Authentication** - Stateless authentication with session management
- **OpenAPI Documentation** - Auto-generated Swagger and Scalar UI
- **Type-Safe Validation** - Comprehensive request/response validation
- **Middleware Stack** - CORS, security headers, and request logging
- **Service Integration** - PostgreSQL, MinIO, OpenRouter, and NATS clients

## Key Dependencies

- `axum` - Modern web framework with excellent async performance
- `tokio` - Async runtime for concurrent request handling
- `tower` - Middleware ecosystem for HTTP services
- `utoipa` - OpenAPI documentation generation

## API Documentation

When running, the server exposes interactive documentation at:

- **Swagger UI**: `/api/swagger`
- **Scalar UI**: `/api/scalar`
- **OpenAPI JSON**: `/api/openapi.json`
