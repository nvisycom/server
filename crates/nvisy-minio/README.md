# api.nvisy.com/minio

S3-compatible MinIO client for object storage operations with async support and
comprehensive error handling.

[![rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![minio](https://img.shields.io/badge/MinIO-S3%20Compatible-000000?style=flat-square&logo=minio&logoColor=white)](https://min.io/)

## Features

- **S3-Compatible Operations** - Full support for MinIO's S3-compatible API
- **Async/Await Support** - Non-blocking operations with Tokio runtime
- **Bucket Management** - Create, delete, and manage storage buckets
- **Object Operations** - Upload, download, delete, and list objects with
  metadata
- **Streaming Support** - Efficient handling of large files with streaming
- **Error Handling** - Comprehensive error types with context and recovery hints

## Key Dependencies

- `minio` - Official MinIO Rust client for S3-compatible operations
- `tokio` - Async runtime for non-blocking I/O operations
- `bytes` - Efficient byte buffer management for streaming
- `futures` - Stream processing utilities for async operations
