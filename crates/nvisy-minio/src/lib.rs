#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

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
    BucketOperations, DownloadResult, ListObjectsResult, ObjectOperations, UploadResult,
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
}

/// Specialized [`Result`] type for MinIO operations.
///
/// This is a convenience alias that uses [`MinioError`] as the error type,
/// making MinIO operation signatures cleaner and more consistent.
pub type Result<T, E = Error> = std::result::Result<T, E>;
