//! MinIO operations for buckets and objects.
//!
//! This module provides high-level interfaces for performing operations on
//! MinIO buckets and objects, including creation, deletion, listing, uploading,
//! and downloading with comprehensive error handling and observability.
//!
//! ## Features
//!
//! - **Bucket Operations**: Create, delete, list, and manage MinIO buckets
//! - **Object Operations**: Upload, download, delete, and list objects with metadata
//! - **Streaming Support**: Memory-efficient handling of large objects
//! - **Metadata Management**: Rich metadata support for objects and buckets
//! - **Error Handling**: Comprehensive error handling with recovery hints
//! - **Observability**: Detailed tracing and metrics for all operations

mod bucket_operations;
mod custom_hooks;
mod object_operations;

pub use bucket_operations::BucketOperations;
pub use object_operations::{DownloadResult, ListObjectsResult, ObjectOperations, UploadResult};
