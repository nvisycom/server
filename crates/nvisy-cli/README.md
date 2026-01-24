# nvisy-cli

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

Command-line interface and HTTP server for the Nvisy platform.

## Features

- **Server Lifecycle** - Startup, graceful shutdown, and health monitoring
- **Flexible Configuration** - CLI arguments and environment variables
- **TLS Support** - HTTPS with rustls (optional)
- **AI Backends** - Pluggable providers for embeddings, OCR, and VLM

## Key Dependencies

- `clap` - Command line argument parser with derive macros
- `axum` - Web framework for HTTP server
- `tokio` - Async runtime for concurrent operations
- `tracing` - Structured logging and diagnostics

## Optional Features

- **tls** - HTTPS support with rustls
- **dotenv** - Load configuration from `.env` files
