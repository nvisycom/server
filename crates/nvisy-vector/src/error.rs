//! Vector store error types.

use thiserror::Error;

/// Result type for vector store operations.
pub type VectorResult<T> = Result<T, VectorError>;

/// Vector store errors.
#[derive(Debug, Error)]
pub enum VectorError {
    /// Connection error.
    #[error("connection error: {0}")]
    Connection(String),

    /// Collection not found.
    #[error("collection not found: {0}")]
    CollectionNotFound(String),

    /// Invalid configuration.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// Authentication error.
    #[error("authentication error: {0}")]
    Authentication(String),

    /// Operation timeout.
    #[error("operation timed out: {0}")]
    Timeout(String),

    /// Vector dimension mismatch.
    #[error("dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Backend-specific error.
    #[error("backend error: {0}")]
    Backend(String),

    /// Serialization/deserialization error.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Feature not enabled.
    #[error("feature not enabled: {0}")]
    FeatureNotEnabled(String),
}

impl VectorError {
    /// Creates a connection error.
    pub fn connection(msg: impl Into<String>) -> Self {
        Self::Connection(msg.into())
    }

    /// Creates a collection not found error.
    pub fn collection_not_found(name: impl Into<String>) -> Self {
        Self::CollectionNotFound(name.into())
    }

    /// Creates an invalid config error.
    pub fn invalid_config(msg: impl Into<String>) -> Self {
        Self::InvalidConfig(msg.into())
    }

    /// Creates an authentication error.
    pub fn authentication(msg: impl Into<String>) -> Self {
        Self::Authentication(msg.into())
    }

    /// Creates a timeout error.
    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout(msg.into())
    }

    /// Creates a dimension mismatch error.
    pub fn dimension_mismatch(expected: usize, actual: usize) -> Self {
        Self::DimensionMismatch { expected, actual }
    }

    /// Creates a backend error.
    pub fn backend(msg: impl Into<String>) -> Self {
        Self::Backend(msg.into())
    }

    /// Creates a serialization error.
    pub fn serialization(msg: impl Into<String>) -> Self {
        Self::Serialization(msg.into())
    }

    /// Creates a feature not enabled error.
    pub fn feature_not_enabled(feature: impl Into<String>) -> Self {
        Self::FeatureNotEnabled(feature.into())
    }
}

impl From<serde_json::Error> for VectorError {
    fn from(err: serde_json::Error) -> Self {
        Self::serialization(err.to_string())
    }
}
