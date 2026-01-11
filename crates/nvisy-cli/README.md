# nvisy-cli

Command-line interface and HTTP server for the Nvisy platform.

[![Rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Axum](https://img.shields.io/badge/Axum-0.8+-000000?style=flat-square&logo=rust&logoColor=white)](https://github.com/tokio-rs/axum)

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
