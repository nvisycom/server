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
//! - **Observability**: Comprehensive structured tracing, performance metrics, and operation lifecycle tracking

mod bucket_operations;
mod custom_hooks;
mod object_operations;
pub mod observability;

pub use bucket_operations::BucketOperations;
pub use custom_hooks::{
    ErrorTracing, OperationMetrics, OperationTracer, OperationType, download_tracing,
    upload_tracing,
};
pub use object_operations::{DownloadResult, ListObjectsResult, ObjectOperations, UploadResult};
pub use observability::{
    DiagnosticInfo, HealthCheck, HealthStatus, MetricsSnapshot, PerformanceAnalysis,
    PerformanceMonitor, PerformanceThresholds, ResourceUsage,
};
