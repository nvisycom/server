//! Comprehensive tracing utilities for MinIO operations.
//!
//! This module provides structured tracing, performance metrics, and observability
//! utilities for all MinIO operations. It replaces the complex hook system with
//! a simpler, more performant tracing approach.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tracing::{Span, debug, error, info, warn};

// Remove unused imports - we'll use the observability OperationMetrics instead
use crate::operations::{DownloadResult, UploadResult};
use crate::types::{DownloadContext, UploadContext};
use crate::{Error, TRACING_TARGET_BUCKETS, TRACING_TARGET_OBJECTS, TRACING_TARGET_OPERATIONS};

/// Operation type for tracing categorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    /// Object upload operation.
    Upload,
    /// Object download operation.
    Download,
    /// Object deletion operation.
    Delete,
    /// Object listing operation.
    List,
    /// Bucket creation operation.
    CreateBucket,
    /// Bucket deletion operation.
    DeleteBucket,
    /// Bucket listing operation.
    ListBuckets,
}

impl OperationType {
    /// Returns the string representation of the operation type.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Upload => "upload",
            Self::Download => "download",
            Self::Delete => "delete",
            Self::List => "list",
            Self::CreateBucket => "create_bucket",
            Self::DeleteBucket => "delete_bucket",
            Self::ListBuckets => "list_buckets",
        }
    }

    /// Returns the appropriate tracing target for this operation.
    pub fn tracing_target(&self) -> &'static str {
        match self {
            Self::Upload | Self::Download | Self::Delete | Self::List => TRACING_TARGET_OBJECTS,
            Self::CreateBucket | Self::DeleteBucket | Self::ListBuckets => TRACING_TARGET_BUCKETS,
        }
    }
}

/// Performance metrics for an operation (lightweight version).
#[derive(Debug, Clone)]
pub struct OperationMetrics {
    /// Operation type.
    pub operation: OperationType,
    /// Total duration of the operation.
    pub duration: Duration,
    /// Size of data processed (bytes).
    pub bytes_processed: u64,
    /// Success status.
    pub success: bool,
    /// Additional metrics.
    pub additional_metrics: HashMap<String, f64>,
}

impl OperationMetrics {
    /// Creates new operation metrics.
    pub fn new(
        operation: OperationType,
        duration: Duration,
        bytes_processed: u64,
        success: bool,
    ) -> Self {
        Self {
            operation,
            duration,
            bytes_processed,
            success,
            additional_metrics: HashMap::new(),
        }
    }

    /// Adds an additional metric.
    pub fn with_metric(mut self, key: String, value: f64) -> Self {
        self.additional_metrics.insert(key, value);
        self
    }

    /// Calculates throughput in bytes per second.
    pub fn throughput_bps(&self) -> f64 {
        if self.duration.as_secs_f64() > 0.0 {
            self.bytes_processed as f64 / self.duration.as_secs_f64()
        } else {
            0.0
        }
    }

    /// Logs the metrics using structured tracing.
    pub fn log_metrics(&self) {
        let throughput = self.throughput_bps();

        if self.success {
            info!(
                target: TRACING_TARGET_OPERATIONS,
                operation = self.operation.as_str(),
                duration_ms = self.duration.as_millis(),
                bytes_processed = self.bytes_processed,
                throughput_mbps = throughput / (1024.0 * 1024.0),
                "Operation completed successfully"
            );
        } else {
            warn!(
                target: TRACING_TARGET_OPERATIONS,
                operation = self.operation.as_str(),
                duration_ms = self.duration.as_millis(),
                bytes_processed = self.bytes_processed,
                "Operation completed with issues"
            );
        }

        // Log additional metrics
        for (key, value) in &self.additional_metrics {
            debug!(
                target: TRACING_TARGET_OPERATIONS,
                operation = self.operation.as_str(),
                metric = key,
                value = value,
                "Additional metric"
            );
        }
    }
}

/// Operation tracer that tracks the lifecycle of operations.
#[derive(Debug)]
pub struct OperationTracer {
    operation: OperationType,
    bucket: String,
    key: Option<String>,
    start_time: Instant,
    span: Span,
    bytes_processed: u64,
    metrics: Option<Arc<crate::operations::observability::OperationMetrics>>,
}

impl OperationTracer {
    /// Creates a new operation tracer for object operations.
    pub fn new_object_operation(operation: OperationType, bucket: &str, key: &str) -> Self {
        let span = tracing::info_span!(
            target: TRACING_TARGET_OBJECTS,
            "minio_operation",
            operation = operation.as_str(),
            bucket = bucket,
            key = key
        );

        info!(
            target: TRACING_TARGET_OBJECTS,
            operation = operation.as_str(),
            bucket = bucket,
            key = key,
            "Starting operation"
        );

        Self {
            operation,
            bucket: bucket.to_string(),
            key: Some(key.to_string()),
            start_time: Instant::now(),
            span,
            bytes_processed: 0,
            metrics: None,
        }
    }

    /// Creates a new operation tracer for bucket operations.
    pub fn new_bucket_operation(operation: OperationType, bucket: &str) -> Self {
        let span = tracing::info_span!(
            target: TRACING_TARGET_BUCKETS,
            "minio_operation",
            operation = operation.as_str(),
            bucket = bucket
        );

        info!(
            target: TRACING_TARGET_BUCKETS,
            operation = operation.as_str(),
            bucket = bucket,
            "Starting bucket operation"
        );

        Self {
            operation,
            bucket: bucket.to_string(),
            key: None,
            start_time: Instant::now(),
            span,
            bytes_processed: 0,
            metrics: None,
        }
    }

    /// Updates the bytes processed counter.
    pub fn add_bytes_processed(&mut self, bytes: u64) {
        self.bytes_processed += bytes;

        debug!(
            target: TRACING_TARGET_OPERATIONS,
            bytes = bytes,
            total_bytes = self.bytes_processed,
            "Bytes processed"
        );
    }

    /// Attaches metrics collector to this tracer.
    pub fn with_metrics(
        mut self,
        metrics: Arc<crate::operations::observability::OperationMetrics>,
    ) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Logs intermediate progress.
    pub fn log_progress(&self, message: &str) {
        debug!(
            target: TRACING_TARGET_OPERATIONS,
            operation = self.operation.as_str(),
            bucket = %self.bucket,
            key = self.key.as_deref().unwrap_or(""),
            elapsed_ms = self.start_time.elapsed().as_millis(),
            bytes_processed = self.bytes_processed,
            message = message,
            "Operation progress"
        );
    }

    /// Completes the operation successfully.
    pub fn complete_success(self) -> OperationMetrics {
        let duration = self.start_time.elapsed();

        info!(
            target: TRACING_TARGET_OPERATIONS,
            operation = self.operation.as_str(),
            bucket = %self.bucket,
            key = self.key.as_deref().unwrap_or(""),
            duration_ms = duration.as_millis(),
            bytes_processed = self.bytes_processed,
            "Operation completed successfully"
        );

        let metrics = OperationMetrics::new(self.operation, duration, self.bytes_processed, true);
        metrics.log_metrics();

        // Record metrics if collector is attached
        if let Some(ref metrics_collector) = self.metrics {
            match self.operation {
                OperationType::Upload => {
                    let upload_result = UploadResult {
                        key: self.key.clone().unwrap_or_default(),
                        size: self.bytes_processed,
                        etag: "".to_string(), // Would need to be passed from actual operation
                        duration,
                    };
                    metrics_collector.record_upload_success(&upload_result);
                }
                OperationType::Download => {
                    let download_result = DownloadResult {
                        key: self.key.clone().unwrap_or_default(),
                        size: self.bytes_processed,
                        content_type: None,
                        duration,
                        metadata: std::collections::HashMap::new(),
                    };
                    metrics_collector.record_download_success(&download_result);
                }
                _ => {} // Other operation types
            }
        }

        metrics
    }

    /// Completes the operation with an error.
    pub fn complete_error(self, error: &Error) -> OperationMetrics {
        let duration = self.start_time.elapsed();

        error!(
            target: TRACING_TARGET_OPERATIONS,
            operation = self.operation.as_str(),
            bucket = %self.bucket,
            key = self.key.as_deref().unwrap_or(""),
            duration_ms = duration.as_millis(),
            bytes_processed = self.bytes_processed,
            error = %error,
            "Operation failed"
        );

        let metrics = OperationMetrics::new(self.operation, duration, self.bytes_processed, false);
        metrics.log_metrics();

        // Record error metrics if collector is attached
        if let Some(ref metrics_collector) = self.metrics {
            let operation_type = match self.operation {
                OperationType::Upload => "upload",
                OperationType::Download => "download",
                OperationType::Delete => "delete",
                OperationType::List => "list",
                OperationType::CreateBucket => "create_bucket",
                OperationType::DeleteBucket => "delete_bucket",
                OperationType::ListBuckets => "list_buckets",
            };
            metrics_collector.record_operation_failure(error, operation_type);
        }

        metrics
    }

    /// Returns the current span for this operation.
    pub fn span(&self) -> &Span {
        &self.span
    }
}

/// Utility functions for tracing upload operations.
pub mod upload_tracing {
    use super::*;

    /// Traces the start of an upload operation.
    pub fn trace_upload_start(context: &UploadContext) -> OperationTracer {
        let mut tracer = OperationTracer::new_object_operation(
            OperationType::Upload,
            &context.bucket,
            &context.key,
        );

        tracer.add_bytes_processed(context.size);

        debug!(
            target: TRACING_TARGET_OPERATIONS,
            bucket = %context.bucket,
            key = %context.key,
            size = context.size,
            content_type = context.content_type.as_deref().unwrap_or("unknown"),
            "Upload context prepared"
        );

        tracer
    }

    /// Traces the start of an upload operation with metrics collection.
    pub fn trace_upload_start_with_metrics(
        context: &UploadContext,
        metrics: Arc<crate::operations::observability::OperationMetrics>,
    ) -> OperationTracer {
        trace_upload_start(context).with_metrics(metrics)
    }

    /// Traces upload success with result details.
    pub fn trace_upload_success(result: &UploadResult) {
        info!(
            target: TRACING_TARGET_OBJECTS,
            key = %result.key,
            size = result.size,
            etag = %result.etag,
            duration_ms = result.duration.as_millis(),
            throughput_mbps = (result.size as f64 / result.duration.as_secs_f64()) / (1024.0 * 1024.0),
            "Upload completed successfully"
        );
    }

    /// Traces upload error with context.
    pub fn trace_upload_error(context: &UploadContext, error: &Error) {
        error!(
            target: TRACING_TARGET_OBJECTS,
            bucket = %context.bucket,
            key = %context.key,
            size = context.size,
            error = %error,
            error_type = error.as_str(),
            "Upload operation failed"
        );
    }
}

/// Utility functions for tracing download operations.
pub mod download_tracing {
    use super::*;

    /// Traces the start of a download operation.
    pub fn trace_download_start(context: &DownloadContext) -> OperationTracer {
        let tracer = OperationTracer::new_object_operation(
            OperationType::Download,
            &context.bucket,
            &context.key,
        );

        debug!(
            target: TRACING_TARGET_OPERATIONS,
            bucket = %context.bucket,
            key = %context.key,
            "Download context prepared"
        );

        tracer
    }

    /// Traces the start of a download operation with metrics collection.
    pub fn trace_download_start_with_metrics(
        context: &DownloadContext,
        metrics: Arc<crate::operations::observability::OperationMetrics>,
    ) -> OperationTracer {
        trace_download_start(context).with_metrics(metrics)
    }

    /// Traces download success with result details.
    pub fn trace_download_success(result: &DownloadResult) {
        info!(
            target: TRACING_TARGET_OBJECTS,
            key = %result.key,
            size = result.size,
            content_type = result.content_type.as_deref().unwrap_or("unknown"),
            duration_ms = result.duration.as_millis(),
            throughput_mbps = (result.size as f64 / result.duration.as_secs_f64()) / (1024.0 * 1024.0),
            metadata_count = result.metadata.len(),
            "Download completed successfully"
        );
    }

    /// Traces download error with context.
    pub fn trace_download_error(context: &DownloadContext, error: &Error) {
        error!(
            target: TRACING_TARGET_OBJECTS,
            bucket = %context.bucket,
            key = %context.key,
            error = %error,
            error_type = error.as_str(),
            "Download operation failed"
        );
    }
}

/// Error extension trait for better error tracing.
pub trait ErrorTracing {
    /// Returns a string representation of the error type for tracing.
    fn as_str(&self) -> &'static str;

    /// Returns whether this error should be retried.
    fn is_retryable(&self) -> bool;

    /// Returns additional context for tracing.
    fn tracing_context(&self) -> HashMap<&'static str, String>;
}

impl ErrorTracing for Error {
    fn as_str(&self) -> &'static str {
        match self {
            Error::Config(_) => "config",
            Error::InvalidRequest(_) => "invalid_request",
            Error::NotFound(_) => "not_found",
            Error::TransientNetwork(_) => "transient_network",
            Error::RateLimited { .. } => "rate_limited",
            Error::QuotaExceeded { .. } => "quota_exceeded",
            Error::ChecksumMismatch { .. } => "checksum_mismatch",
            Error::Timeout { .. } => "timeout",
            Error::ServerError { .. } => "server_error",
            Error::Serialization(_) => "serialization",
            Error::Io(_) => "io",
            Error::Client(_) => "client",
        }
    }

    fn is_retryable(&self) -> bool {
        match self {
            Error::Config(_) => false,
            Error::InvalidRequest(_) => false,
            Error::NotFound(_) => false,
            Error::TransientNetwork(_) => true,
            Error::RateLimited { .. } => true,
            Error::QuotaExceeded { .. } => false,
            Error::ChecksumMismatch { .. } => false,
            Error::Timeout { .. } => true,
            Error::ServerError { .. } => true,
            Error::Serialization(_) => false,
            Error::Io(_) => true,     // Network I/O issues are often transient
            Error::Client(_) => true, // Most client errors are retryable
        }
    }

    fn tracing_context(&self) -> HashMap<&'static str, String> {
        let mut context = HashMap::new();
        context.insert("error_type", self.as_str().to_string());
        context.insert("retryable", self.is_retryable().to_string());

        match self {
            Error::Config(msg) => {
                context.insert("category", "configuration".to_string());
                context.insert("message", msg.clone());
            }
            Error::InvalidRequest(msg) => {
                context.insert("category", "validation".to_string());
                context.insert("message", msg.clone());
            }
            Error::NotFound(msg) => {
                context.insert("category", "resource".to_string());
                context.insert("message", msg.clone());
            }
            Error::TransientNetwork(msg) => {
                context.insert("category", "network".to_string());
                context.insert("message", msg.clone());
            }
            Error::RateLimited { .. } => {
                context.insert("category", "rate_limiting".to_string());
                context.insert("message", "Rate limit exceeded".to_string());
            }
            Error::QuotaExceeded { current, limit } => {
                context.insert("category", "quota".to_string());
                context.insert("message", format!("Quota exceeded: {}/{}", current, limit));
            }
            Error::ChecksumMismatch { expected, actual } => {
                context.insert("category", "integrity".to_string());
                context.insert(
                    "message",
                    format!("Checksum mismatch: expected {}, got {}", expected, actual),
                );
            }
            Error::Timeout { timeout } => {
                context.insert("category", "timeout".to_string());
                context.insert(
                    "message",
                    format!("Operation timed out after {:?}", timeout),
                );
            }
            Error::ServerError {
                message,
                status_code,
            } => {
                context.insert("category", "server".to_string());
                context.insert("message", format!("{} (status: {})", message, status_code));
            }
            Error::Serialization(e) => {
                context.insert("category", "data".to_string());
                context.insert("message", e.to_string());
            }
            Error::Io(e) => {
                context.insert("category", "system".to_string());
                context.insert("message", e.to_string());
            }
            Error::Client(e) => {
                context.insert("category", "network".to_string());
                context.insert("message", e.to_string());
            }
        }

        context
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_type_string_representation() {
        assert_eq!(OperationType::Upload.as_str(), "upload");
        assert_eq!(OperationType::Download.as_str(), "download");
        assert_eq!(OperationType::Delete.as_str(), "delete");
    }

    #[test]
    fn test_operation_metrics_throughput() {
        let metrics =
            OperationMetrics::new(OperationType::Upload, Duration::from_secs(1), 1024, true);

        assert_eq!(metrics.throughput_bps(), 1024.0);
    }

    #[test]
    fn test_error_tracing_context() {
        let error = Error::NotFound("bucket not found".to_string());
        let context = error.tracing_context();

        assert_eq!(context.get("error_type"), Some(&"not_found".to_string()));
        assert_eq!(context.get("retryable"), Some(&"false".to_string()));
        assert_eq!(context.get("category"), Some(&"resource".to_string()));
    }
}
