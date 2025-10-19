# api.nvisy.com/minio

High-performance, type-safe MinIO object storage client for the Nvisy platform,
built with async/await and comprehensive error handling.

[![Rust](https://img.shields.io/badge/rust-1.89+-blue.svg)](https://www.rust-lang.org/)
[![MinIO](https://img.shields.io/badge/minio-0.3+-green.svg)](https://min.io/)

## Features

- **S3-Compatible Operations** - Full support for MinIO's S3-compatible API
- **Async/Await Support** - Non-blocking operations with Tokio runtime
- **Streaming Uploads/Downloads** - Memory-efficient handling of large objects
- **Comprehensive Error Handling** - Detailed error types with recovery hints
- **Type-Safe Configuration** - Compile-time validation of client settings
- **Production Ready** - Health checks, metrics, and observability built-in
- **Bucket Management** - Create, list, and manage MinIO buckets
- **Object Operations** - Upload, download, delete, and list objects with metadata

## Crates

- [`tokio`](https://crates.io/crates/tokio) - Async runtime for Rust
- [`futures`](https://crates.io/crates/futures) - Async stream utilities
- [`minio`](https://crates.io/crates/minio) - Official MinIO Rust client
