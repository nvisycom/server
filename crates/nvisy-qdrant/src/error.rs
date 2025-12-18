//! Error types and utilities for Qdrant operations.
//!
//! This module provides comprehensive error handling for all Qdrant operations,
//! including connection errors, search errors, collection management errors, and timeout errors.

use std::time::Duration;

/// Result type for all Qdrant operations in this crate.
///
/// This is a convenience type alias that defaults to using [`QdrantError`] as the error type.
/// Most functions in this crate return this type for consistent error handling.
pub type QdrantResult<T, E = QdrantError> = std::result::Result<T, E>;

/// Unified error type for Qdrant operations
#[derive(Debug, thiserror::Error)]
pub enum QdrantError {
    /// Qdrant client/connection errors
    #[error("Qdrant connection error: {0}")]
    Connection(#[from] qdrant_client::QdrantError),

    /// Serialization errors when sending or receiving data
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Collection not found
    #[error("Collection '{name}' not found")]
    CollectionNotFound { name: String },

    /// Collection already exists
    #[error("Collection '{name}' already exists")]
    CollectionAlreadyExists { name: String },

    /// Point not found
    #[error("Point '{id}' not found in collection '{collection}'")]
    PointNotFound { collection: String, id: String },

    /// Invalid vector dimensions
    #[error("Invalid vector dimensions: expected {expected}, got {actual}")]
    InvalidVectorDimensions { expected: usize, actual: usize },

    /// Invalid vector format
    #[error("Invalid vector format: {reason}")]
    InvalidVector { reason: String },

    /// Search operation failed
    #[error("Search operation failed in collection '{collection}': {reason}")]
    SearchError { collection: String, reason: String },

    /// Batch operation failed
    #[error("Batch operation failed: {failed_count}/{total_count} operations failed")]
    BatchOperationFailed {
        failed_count: usize,
        total_count: usize,
    },

    /// Operation timeout
    #[error("Operation timed out after {timeout:?}")]
    Timeout { timeout: Duration },

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {message}")]
    RateLimited { message: String },

    /// Authentication failed
    #[error("Authentication failed: {reason}")]
    Authentication { reason: String },

    /// Server error from Qdrant
    #[error("Qdrant server error: {status_code} - {message}")]
    ServerError { status_code: u16, message: String },

    /// Invalid configuration
    #[error("Invalid configuration: {reason}")]
    InvalidConfig { reason: String },

    /// Collection operation failed
    #[error("Collection operation failed on '{collection}': {operation} - {error}")]
    CollectionError {
        collection: String,
        operation: String,
        error: String,
    },

    /// Point operation failed
    #[error("Point operation failed: {operation} - {details}")]
    PointError { operation: String, details: String },

    /// Index operation failed
    #[error("Index operation failed on collection '{collection}': {reason}")]
    IndexError { collection: String, reason: String },

    /// Payload operation failed
    #[error("Payload operation failed: {0}")]
    PayloadError(String),

    /// Filter operation failed
    #[error("Filter operation failed: {reason}")]
    FilterError { reason: String },

    /// Generic operation error with context
    #[error("Qdrant operation failed: {operation} - {details}")]
    Operation { operation: String, details: String },

    /// Invalid input provided
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Repository operation error
    #[error("Repository operation '{operation}' failed on collection '{collection}'")]
    RepositoryError {
        operation: String,
        collection: String,
        source: Box<QdrantError>,
    },

    /// Conversion error when transforming data types
    #[error("Conversion error: {0}")]
    Conversion(String),

    /// Unexpected error occurred
    #[error("Unexpected error: {message}")]
    Unexpected { message: String },
}

impl QdrantError {
    /// Create a collection not found error
    pub fn collection_not_found(name: impl Into<String>) -> Self {
        Self::CollectionNotFound { name: name.into() }
    }

    /// Create a collection already exists error
    pub fn collection_already_exists(name: impl Into<String>) -> Self {
        Self::CollectionAlreadyExists { name: name.into() }
    }

    /// Create a point not found error
    pub fn point_not_found(collection: impl Into<String>, id: impl Into<String>) -> Self {
        Self::PointNotFound {
            collection: collection.into(),
            id: id.into(),
        }
    }

    /// Create an invalid vector dimensions error
    pub fn invalid_vector_dimensions(expected: usize, actual: usize) -> Self {
        Self::InvalidVectorDimensions { expected, actual }
    }

    /// Create an invalid vector error
    pub fn invalid_vector(reason: impl Into<String>) -> Self {
        Self::InvalidVector {
            reason: reason.into(),
        }
    }

    /// Create a search error
    pub fn search_error(collection: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::SearchError {
            collection: collection.into(),
            reason: reason.into(),
        }
    }

    /// Create a batch operation failed error
    pub fn batch_operation_failed(failed_count: usize, total_count: usize) -> Self {
        Self::BatchOperationFailed {
            failed_count,
            total_count,
        }
    }

    /// Create a rate limited error
    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self::RateLimited {
            message: message.into(),
        }
    }

    /// Create an authentication error
    pub fn authentication(reason: impl Into<String>) -> Self {
        Self::Authentication {
            reason: reason.into(),
        }
    }

    /// Create a server error
    pub fn server_error(status_code: u16, message: impl Into<String>) -> Self {
        Self::ServerError {
            status_code,
            message: message.into(),
        }
    }

    /// Create an invalid configuration error
    pub fn invalid_config(reason: impl Into<String>) -> Self {
        Self::InvalidConfig {
            reason: reason.into(),
        }
    }

    /// Create a collection error
    pub fn collection_error(
        collection: impl Into<String>,
        operation: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self::CollectionError {
            collection: collection.into(),
            operation: operation.into(),
            error: error.into(),
        }
    }

    /// Create a point error
    pub fn point_error(operation: impl Into<String>, details: impl Into<String>) -> Self {
        Self::PointError {
            operation: operation.into(),
            details: details.into(),
        }
    }

    /// Create an index error
    pub fn index_error(collection: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::IndexError {
            collection: collection.into(),
            reason: reason.into(),
        }
    }

    /// Create a payload error
    pub fn payload_error(reason: impl Into<String>) -> Self {
        Self::PayloadError(reason.into())
    }

    /// Create a filter error
    pub fn filter_error(reason: impl Into<String>) -> Self {
        Self::FilterError {
            reason: reason.into(),
        }
    }

    /// Create an operation error with context
    pub fn operation(op: impl Into<String>, details: impl Into<String>) -> Self {
        Self::Operation {
            operation: op.into(),
            details: details.into(),
        }
    }

    /// Create a timeout error with the given duration
    pub fn timeout(duration: Duration) -> Self {
        Self::Timeout { timeout: duration }
    }

    /// Create an unexpected error
    pub fn unexpected(message: impl Into<String>) -> Self {
        Self::Unexpected {
            message: message.into(),
        }
    }

    /// Returns whether this error indicates a transient failure that might succeed on retry.
    ///
    /// Transient errors include timeouts, rate limits, and certain server errors that may
    /// be resolved by retrying the operation.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            QdrantError::Timeout { .. }
                | QdrantError::RateLimited { .. }
                | QdrantError::Connection(_)
                | QdrantError::ServerError {
                    status_code: 500..=599,
                    ..
                }
        )
    }

    /// Returns whether this error indicates a permanent failure that won't succeed on retry.
    ///
    /// Permanent errors include authentication failures, not found errors, invalid data,
    /// and client errors that require changes to resolve.
    pub fn is_permanent(&self) -> bool {
        !self.is_transient()
    }

    /// Returns whether this error indicates a client-side error (4xx status codes or equivalent).
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            QdrantError::CollectionNotFound { .. }
                | QdrantError::PointNotFound { .. }
                | QdrantError::InvalidVector { .. }
                | QdrantError::InvalidVectorDimensions { .. }
                | QdrantError::InvalidConfig { .. }
                | QdrantError::Authentication { .. }
                | QdrantError::ServerError {
                    status_code: 400..=499,
                    ..
                }
        )
    }

    /// Returns whether this error indicates a server-side error (5xx status codes or equivalent).
    pub fn is_server_error(&self) -> bool {
        matches!(
            self,
            QdrantError::ServerError {
                status_code: 500..=599,
                ..
            }
        )
    }

    /// Get a user-friendly error message suitable for display
    pub fn user_message(&self) -> String {
        match self {
            QdrantError::Connection(_) => {
                "Connection to Qdrant server failed. Please check your connection.".to_string()
            }
            QdrantError::Timeout { timeout } => {
                format!("Operation timed out after {:?}. Please try again.", timeout)
            }
            QdrantError::CollectionNotFound { name } => {
                format!("Collection '{}' not found.", name)
            }
            QdrantError::PointNotFound { id, .. } => format!("Point '{}' not found.", id),
            QdrantError::InvalidVectorDimensions { expected, actual } => format!(
                "Vector has wrong dimensions. Expected {} dimensions, got {}.",
                expected, actual
            ),
            QdrantError::RateLimited { .. } => {
                "Rate limit exceeded. Please wait before trying again.".to_string()
            }
            QdrantError::Authentication { .. } => {
                "Authentication failed. Please check your API key.".to_string()
            }
            QdrantError::Serialization(_) => {
                "Data format error. Please check your input.".to_string()
            }
            QdrantError::InvalidConfig { reason } => format!("Configuration error: {}", reason),
            QdrantError::InvalidInput(reason) => format!("Invalid input: {}", reason),
            QdrantError::RepositoryError {
                operation,
                collection,
                ..
            } => {
                format!(
                    "Repository operation '{}' failed on collection '{}'",
                    operation, collection
                )
            }
            _ => "An unexpected error occurred. Please try again.".to_string(),
        }
    }

    /// Get the HTTP status code if this error represents an HTTP error
    pub fn status_code(&self) -> Option<u16> {
        match self {
            QdrantError::ServerError { status_code, .. } => Some(*status_code),
            QdrantError::CollectionNotFound { .. } | QdrantError::PointNotFound { .. } => Some(404),
            QdrantError::Authentication { .. } => Some(401),
            QdrantError::RateLimited { .. } => Some(429),
            QdrantError::InvalidVector { .. }
            | QdrantError::InvalidVectorDimensions { .. }
            | QdrantError::InvalidConfig { .. }
            | QdrantError::InvalidInput(_) => Some(400),
            _ => None,
        }
    }
}
