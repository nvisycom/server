//! MinIO client with configuration management and operations.
//!
//! This module provides a high-level interface for connecting to MinIO object storage,
//! managing client configuration, and performing bucket and object operations. It includes
//! comprehensive error handling, observability through tracing, and production-ready configuration.
//!
//! ## Features
//!
//! - **Client Management**: High-level MinIO client with connection management
//! - **Configuration**: Flexible configuration with validation and defaults
//! - **Authentication**: Secure credential management with multiple auth methods
//! - **Observability**: Comprehensive tracing and metrics for storage operations
//! - **Error Handling**: Rich error types with context and debugging information

mod minio_client;
mod minio_config;
mod minio_credentials;

pub use minio_client::MinioClient;
pub use minio_config::MinioConfig;
pub use minio_credentials::MinioCredentials;
