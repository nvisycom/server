# api.nvisy.com/cli

High-performance HTTP server CLI for document processing with enhanced lifecycle management, built with Rust and Axum.

[![Rust](https://img.shields.io/badge/rust-1.70+-blue.svg)](https://www.rust-lang.org/)
[![Axum](https://img.shields.io/badge/axum-0.8+-blue.svg)](https://github.com/tokio-rs/axum)
[![Clap](https://img.shields.io/badge/clap-4.0+-blue.svg)](https://github.com/clap-rs/clap)

## Features

- **Enhanced Server Lifecycle** - Comprehensive startup, shutdown, and health monitoring
- **Privacy-First Telemetry** - Optional usage analytics and crash reporting with full user control
- **Advanced Error Handling** - Rich error context with recovery suggestions and error codes
- **Production Ready** - TLS support, graceful shutdown, structured logging, configuration validation
- **Type-Safe Configuration** - CLI arguments, environment variables, and programmatic config
- **Security Focused** - Automatic security warnings and configuration validation

## Usage

```bash
# Basic HTTP server
nvisy-cli --host 0.0.0.0 --port 8080

# HTTPS with TLS
nvisy-cli --tls-cert-path ./cert.pem --tls-key-path ./key.pem

# With telemetry (opt-in)
nvisy-cli --telemetry-enabled --telemetry-usage --telemetry-crashes
```

## Features

- `tls` - TLS/HTTPS support with certificate management
- `telemetry` - Anonymous usage analytics and crash reporting  
- `otel` - OpenTelemetry integration for distributed tracing

## Crates

- [`axum`](https://crates.io/crates/axum) - Web framework built on Tokio
- [`clap`](https://crates.io/crates/clap) - Command line argument parser
- [`tokio`](https://crates.io/crates/tokio) - Async runtime for Rust
- [`tracing`](https://crates.io/crates/tracing) - Structured application-level tracing