# nvisy-server

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

High-performance HTTP API server for the Nvisy document redaction platform,
built with Axum and Tokio.

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
