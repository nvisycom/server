//! Error handling for VLM (Vision Language Model) operations.
//!
//! This module provides comprehensive error types for VLM services, including
//! classification of errors into client vs server errors, retry policies, and
//! structured error information to help with debugging and error handling.
//!
//! # Error Classification
//!
//! Errors are classified into several categories:
//!
//! - **Client Errors**: Authentication failures, invalid input, unsupported models
//! - **Server Errors**: Service unavailable, internal errors, model inference failures
//! - **Retryable Errors**: Network issues, timeouts, rate limits, service problems
//! - **Non-retryable Errors**: Authentication, invalid prompts, unsupported features
//!
//! # Examples
//!
//! ```rust
//! use nvisy_core::vlm::Error;
//!
//! // Create specific error types
//! let auth_error = Error::authentication();
//! let timeout_error = Error::timeout();
//!
//! // Check error classification
//! assert!(auth_error.is_client_error());
//! assert!(timeout_error.is_retryable());
//!
//! // Get retry delay for retryable errors
//! if let Some(delay) = timeout_error.retry_delay() {
//!     // Wait before retrying
//! }
//! ```

use std::time::Duration;

use crate::BoxedError;

/// Result type alias for VLM operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for VLM operations.
///
/// This error type provides structured information about what went wrong during
/// VLM processing, including the specific error kind and optional source error
/// for better debugging and error handling.
#[derive(Debug, thiserror::Error)]
#[error("{kind}")]
pub struct Error {
    /// The specific kind of error that occurred.
    pub kind: ErrorKind,
    /// Optional source error for additional context.
    #[source]
    pub source: Option<BoxedError>,
}

impl Error {
    /// Creates a new error with the given kind.
    pub fn new(kind: ErrorKind) -> Self {
        Self { kind, source: None }
    }

    /// Adds a source error to this error.
    ///
    /// This method consumes the error and returns a new error with the source attached,
    /// allowing for method chaining when constructing errors.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// Error::new(ErrorKind::NetworkError)
    ///     .with_source(io_error)
    /// ```
    pub fn with_source(mut self, source: BoxedError) -> Self {
        self.source = Some(source);
        self
    }

    /// Returns true if this is a client error (4xx-style).
    ///
    /// Client errors indicate problems with the request that the client
    /// should fix before retrying, such as authentication issues or
    /// invalid prompt parameters.
    pub fn is_client_error(&self) -> bool {
        matches!(
            self.kind,
            ErrorKind::Authentication | ErrorKind::InvalidInput | ErrorKind::UnsupportedFormat
        )
    }

    /// Returns true if this is a server error (5xx-style).
    ///
    /// Server errors indicate problems on the service side that are
    /// typically outside the client's control.
    pub fn is_server_error(&self) -> bool {
        matches!(
            self.kind,
            ErrorKind::ServiceUnavailable
                | ErrorKind::InternalError
                | ErrorKind::ModelInferenceFailed
        )
    }

    /// Returns true if the operation should be retried.
    ///
    /// Retryable errors are typically transient issues like network
    /// problems, rate limits, or temporary service unavailability.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self.kind,
            ErrorKind::RateLimited
                | ErrorKind::NetworkError
                | ErrorKind::Timeout
                | ErrorKind::ServiceUnavailable
        )
    }

    /// Returns the suggested retry delay for retryable errors.
    ///
    /// Returns `None` for non-retryable errors. The delay duration
    /// is based on the error type and follows common retry patterns.
    pub fn retry_delay(&self) -> Option<Duration> {
        match self.kind {
            ErrorKind::RateLimited => Some(Duration::from_secs(60)),
            ErrorKind::ServiceUnavailable => Some(Duration::from_secs(10)),
            ErrorKind::NetworkError => Some(Duration::from_secs(5)),
            ErrorKind::Timeout => Some(Duration::from_secs(2)),
            _ => None,
        }
    }
}

/// Specific kinds of VLM errors.
///
/// This enum categorizes all possible error conditions that can occur
/// during VLM operations, from authentication failures to model inference errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Authentication with the VLM service failed.
    Authentication,

    /// The input provided to the VLM service is invalid.
    InvalidInput,

    /// The image format is not supported by the VLM service.
    UnsupportedFormat,

    /// Model inference failed during processing.
    ModelInferenceFailed,

    /// Rate limit has been exceeded.
    RateLimited,

    /// A network error occurred during the request.
    NetworkError,

    /// The operation timed out.
    Timeout,

    /// The VLM service is temporarily unavailable.
    ServiceUnavailable,

    /// An internal service error occurred.
    InternalError,
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Authentication => write!(f, "Authentication failed"),
            Self::InvalidInput => write!(f, "Invalid input provided"),
            Self::UnsupportedFormat => write!(f, "Unsupported format"),
            Self::ModelInferenceFailed => write!(f, "Model inference failed"),
            Self::RateLimited => write!(f, "Rate limit exceeded"),
            Self::NetworkError => write!(f, "Network error occurred"),
            Self::Timeout => write!(f, "Operation timed out"),
            Self::ServiceUnavailable => write!(f, "Service unavailable"),
            Self::InternalError => write!(f, "Internal service error"),
        }
    }
}

// Convenience constructors for common error scenarios
impl Error {
    /// Creates an authentication error.
    pub fn authentication() -> Self {
        Self::new(ErrorKind::Authentication)
    }

    /// Creates an invalid input error.
    pub fn invalid_input() -> Self {
        Self::new(ErrorKind::InvalidInput)
    }

    /// Creates an unsupported format error.
    pub fn unsupported_format() -> Self {
        Self::new(ErrorKind::UnsupportedFormat)
    }

    /// Creates a model inference failed error.
    pub fn model_inference_failed() -> Self {
        Self::new(ErrorKind::ModelInferenceFailed)
    }

    /// Creates a rate limited error.
    pub fn rate_limited() -> Self {
        Self::new(ErrorKind::RateLimited)
    }

    /// Creates a network error.
    pub fn network_error() -> Self {
        Self::new(ErrorKind::NetworkError)
    }

    /// Creates a timeout error.
    pub fn timeout() -> Self {
        Self::new(ErrorKind::Timeout)
    }

    /// Creates a service unavailable error.
    pub fn service_unavailable() -> Self {
        Self::new(ErrorKind::ServiceUnavailable)
    }

    /// Creates an internal error.
    pub fn internal_error() -> Self {
        Self::new(ErrorKind::InternalError)
    }
}
