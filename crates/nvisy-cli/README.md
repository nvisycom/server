# api.nvisy.com/cli

Command-line interface for the Nvisy document redaction API server with enhanced
lifecycle management and telemetry support.

[![rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![axum](https://img.shields.io/badge/Axum-0.8+-000000?style=flat-square&logo=rust&logoColor=white)](https://github.com/tokio-rs/axum)
[![clap](https://img.shields.io/badge/Clap-4.5+-000000?style=flat-square&logo=rust&logoColor=white)](https://github.com/clap-rs/clap)

## Features

- **Enhanced Server Lifecycle** - Comprehensive startup, shutdown, and health
  monitoring
- **Flexible Configuration** - CLI arguments, environment variables, and
  file-based config
- **Optional Telemetry** - Privacy-first usage analytics and crash reporting
  (opt-in)
- **TLS Support** - HTTPS with certificate management (optional feature)
- **Production Ready** - Graceful shutdown, structured logging, and error
  handling
- **Service Integration** - PostgreSQL, MinIO, OpenRouter, and NATS
  configuration

## Key Dependencies

- `clap` - Command line argument parser with derive macros
- `axum` - Web framework for HTTP server functionality
- `tokio` - Async runtime for concurrent operations
- `tracing` - Structured application-level logging and diagnostics

## Features

- `tls` - TLS/HTTPS support with certificate management
- `telemetry` - Anonymous usage analytics and crash reporting
- `otel` - OpenTelemetry integration for distributed tracing
