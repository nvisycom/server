#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
#![allow(clippy::result_large_err, clippy::large_enum_variant)]

// Tracing target constants for consistent logging
pub const TRACING_TARGET_CLIENT: &str = "nvisy_minio::client";
pub const TRACING_TARGET_OPERATIONS: &str = "nvisy_minio::operations";
pub const TRACING_TARGET_BUCKETS: &str = "nvisy_minio::buckets";
pub const TRACING_TARGET_OBJECTS: &str = "nvisy_minio::objects";

pub mod client;
pub mod operations;
pub mod types;

pub use nvisy_core::fs::{DataSensitivity, SupportedFormat};

// Re-export for convenience
pub use crate::client::{MinioClient, MinioConfig, MinioCredentials};
pub use crate::operations::{
    BucketOperations, DiagnosticInfo, DownloadResult, HealthCheck, HealthStatus, ListObjectsResult,
    MetricsSnapshot, ObjectOperations, PerformanceAnalysis, PerformanceMonitor,
    PerformanceThresholds, ResourceUsage, UploadResult,
};
pub use crate::types::{
    BucketInfo, BucketPolicy, DownloadContext, Object, ObjectInfo, ObjectMetadata, ObjectTags,
    Stage, UploadContext,
};

/// Error type for MinIO object storage operations.
#[derive(Debug, thiserror::Error)]
#[must_use = "errors should be handled appropriately"]
pub enum Error {
    /// Configuration error.
    ///
    /// This includes invalid configuration parameters, missing required settings,
    /// malformed URLs, or other issues related to the MinIO client configuration.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Invalid request or malformed data.
    ///
    /// This occurs when the request parameters are invalid, malformed,
    /// or when the data being uploaded is corrupted.
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Resource not found.
    ///
    /// This occurs when trying to access a bucket or object that doesn't exist.
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// Transient network error that may succeed on retry.
    ///
    /// This includes temporary network issues, DNS resolution failures,
    /// and connection timeouts that are likely to resolve on retry.
    #[error("Transient network error: {0} (retryable)")]
    TransientNetwork(String),

    /// Rate limiting error with optional retry-after duration.
    ///
    /// This occurs when the request rate exceeds the server's limits.
    /// The retry_after field indicates when the client should retry.
    #[error("Rate limited: retry after {retry_after:?}")]
    RateLimited {
        /// When the client should retry the request.
        retry_after: Option<std::time::Duration>,
    },

    /// Storage quota exceeded error.
    ///
    /// This occurs when the operation would exceed storage quotas.
    #[error("Storage quota exceeded: {current}/{limit} bytes")]
    QuotaExceeded {
        /// Current usage in bytes.
        current: u64,
        /// Quota limit in bytes.
        limit: u64,
    },

    /// Object checksum validation failed.
    ///
    /// This occurs when the computed checksum doesn't match the expected value.
    #[error("Object checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// Expected checksum value.
        expected: String,
        /// Actual computed checksum value.
        actual: String,
    },

    /// Operation timeout error.
    ///
    /// This occurs when an operation takes longer than the configured timeout.
    #[error("Operation timeout after {timeout:?}")]
    Timeout {
        /// The timeout duration that was exceeded.
        timeout: std::time::Duration,
    },

    /// Server-side error that may be retryable.
    ///
    /// This includes 5xx HTTP status codes and server-side issues.
    #[error("Server error: {message} (status: {status_code})")]
    ServerError {
        /// Error message from the server.
        message: String,
        /// HTTP status code.
        status_code: u16,
    },

    /// Serialization or deserialization error.
    ///
    /// This occurs when there are issues converting data to/from the expected format.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// I/O operation failed.
    ///
    /// This includes file system errors, stream reading/writing failures,
    /// and other I/O related issues.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Underlying MinIO client error.
    ///
    /// This wraps errors from the underlying minio crate that don't fit
    /// into the other specific categories.
    #[error("MinIO client error: {0}")]
    Client(#[from] minio::s3::error::Error),
}

impl Error {
    /// Returns whether this error indicates a configuration issue.
    pub fn is_config_error(&self) -> bool {
        matches!(self, Error::Config(_))
    }

    /// Returns whether this error indicates invalid request data.
    pub fn is_invalid_request(&self) -> bool {
        matches!(self, Error::InvalidRequest(_))
    }

    /// Returns whether this error indicates a missing resource.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Error::NotFound(_))
    }

    /// Returns whether this error should trigger an automatic retry.
    ///
    /// Only transient errors that are likely to succeed on retry should return true.
    pub fn is_retryable(&self) -> bool {
        match self {
            Error::TransientNetwork(_) => true,
            Error::RateLimited { .. } => true,
            Error::Timeout { .. } => true,
            Error::ServerError { status_code, .. } => {
                // Retry on 5xx errors (server issues) but not 4xx (client issues)
                *status_code >= 500 && *status_code < 600
            }
            Error::Io(_) => true,      // Network I/O issues are often transient
            Error::Client(_) => false, // Let the client error determine retry logic
            // Non-retryable errors
            Error::Config(_) => false,
            Error::InvalidRequest(_) => false,
            Error::NotFound(_) => false,
            Error::QuotaExceeded { .. } => false,
            Error::ChecksumMismatch { .. } => false,
            Error::Serialization(_) => false,
        }
    }

    /// Returns the recommended delay before retrying this operation.
    ///
    /// Returns `None` if the error is not retryable or no specific delay is recommended.
    pub fn retry_delay(&self) -> Option<std::time::Duration> {
        match self {
            Error::RateLimited { retry_after } => *retry_after,
            Error::TransientNetwork(_) => Some(std::time::Duration::from_secs(1)),
            Error::Timeout { .. } => Some(std::time::Duration::from_millis(500)),
            Error::ServerError { .. } => Some(std::time::Duration::from_secs(2)),
            Error::Io(_) => Some(std::time::Duration::from_millis(200)),
            _ => None,
        }
    }

    /// Returns the severity level of this error for logging purposes.
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Error::Config(_) => ErrorSeverity::Critical,
            Error::QuotaExceeded { .. } => ErrorSeverity::Critical,
            Error::ChecksumMismatch { .. } => ErrorSeverity::High,
            Error::NotFound(_) => ErrorSeverity::Medium,
            Error::InvalidRequest(_) => ErrorSeverity::Medium,
            Error::Serialization(_) => ErrorSeverity::Medium,
            Error::TransientNetwork(_) => ErrorSeverity::Low,
            Error::RateLimited { .. } => ErrorSeverity::Low,
            Error::Timeout { .. } => ErrorSeverity::Low,
            Error::ServerError { .. } => ErrorSeverity::Medium,
            Error::Io(_) => ErrorSeverity::Low,
            Error::Client(_) => ErrorSeverity::Medium,
        }
    }

    /// Returns additional context for debugging this error.
    pub fn context(&self) -> std::collections::HashMap<&'static str, String> {
        let mut context = std::collections::HashMap::new();

        context.insert("retryable", self.is_retryable().to_string());
        context.insert("severity", format!("{:?}", self.severity()));

        if let Some(delay) = self.retry_delay() {
            context.insert("retry_delay_ms", delay.as_millis().to_string());
        }

        match self {
            Error::QuotaExceeded { current, limit } => {
                context.insert(
                    "usage_percentage",
                    format!("{:.1}", (*current as f64 / *limit as f64) * 100.0),
                );
            }
            Error::ServerError { status_code, .. } => {
                context.insert("http_status", status_code.to_string());
            }
            Error::Timeout { timeout } => {
                context.insert("timeout_seconds", timeout.as_secs().to_string());
            }
            _ => {}
        }

        context
    }
}

/// Error severity levels for logging and monitoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// Critical errors that require immediate attention.
    Critical,
    /// High-priority errors that should be investigated quickly.
    High,
    /// Medium-priority errors that should be monitored.
    Medium,
    /// Low-priority errors that are expected during normal operation.
    Low,
}

/// Specialized [`Result`] type for MinIO operations.
///
/// This is a convenience alias that uses [`MinioError`] as the error type,
/// making MinIO operation signatures cleaner and more consistent.
pub type Result<T, E = Error> = std::result::Result<T, E>;
