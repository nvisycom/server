//! Error handling for embedding operations.
//!
//! This module provides comprehensive error types for embedding services, including
//! classification of errors into client vs server errors, retry policies, and
//! structured error information to help with debugging and error handling.
//!
//! # Error Classification
//!
//! Errors are classified into several categories:
//!
//! - **Client Errors**: Authentication failures, invalid input, unsupported models
//! - **Server Errors**: Service unavailable, internal errors, model loading failures
//! - **Retryable Errors**: Network issues, timeouts, rate limits, service problems
//! - **Non-retryable Errors**: Authentication, invalid input, unsupported features

use std::time::Duration;

use crate::BoxedError;

/// Result type alias for embedding operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Main error type for embedding operations.
///
/// This error type provides structured information about what went wrong during
/// embedding processing, including the specific error kind and optional source error
/// for better debugging and error handling.
#[derive(Debug, thiserror::Error)]
#[error("{}", .message.as_ref().map(|m| format!("{}: {}", .kind, m)).unwrap_or_else(|| .kind.to_string()))]
pub struct Error {
    /// The specific kind of error that occurred.
    pub kind: ErrorKind,
    /// Optional additional message providing more context.
    pub message: Option<String>,
    /// Optional source error for additional context.
    #[source]
    pub source: Option<BoxedError>,
}

impl Error {
    /// Creates a new error with the given kind.
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            message: None,
            source: None,
        }
    }

    /// Adds a message to this error.
    ///
    /// This method consumes the error and returns a new error with the message attached,
    /// allowing for method chaining when constructing errors.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// Error::new(ErrorKind::InvalidInput)
    ///     .with_message("Text input exceeds maximum length")
    /// ```
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
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
    /// invalid input parameters.
    pub fn is_client_error(&self) -> bool {
        matches!(
            self.kind,
            ErrorKind::Authentication
                | ErrorKind::InvalidInput
                | ErrorKind::UnsupportedFormat
                | ErrorKind::UnsupportedModel
                | ErrorKind::TokenLimitExceeded
        )
    }

    /// Returns true if this is a server error (5xx-style).
    ///
    /// Server errors indicate problems on the service side that are
    /// typically outside the client's control.
    pub fn is_server_error(&self) -> bool {
        matches!(
            self.kind,
            ErrorKind::ServiceUnavailable | ErrorKind::InternalError | ErrorKind::ModelLoadFailed
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

/// Specific kinds of embedding errors.
///
/// This enum categorizes all possible error conditions that can occur
/// during embedding operations, from authentication failures to model loading errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Authentication with the embedding service failed.
    Authentication,

    /// The input provided to the embedding service is invalid.
    InvalidInput,

    /// The input format is not supported by the embedding service.
    UnsupportedFormat,

    /// The requested embedding model is not supported or available.
    UnsupportedModel,

    /// The input exceeds the model's token limit.
    TokenLimitExceeded,

    /// Model loading or initialization failed.
    ModelLoadFailed,

    /// Rate limit has been exceeded.
    RateLimited,

    /// A network error occurred during the request.
    NetworkError,

    /// The operation timed out.
    Timeout,

    /// The embedding service is temporarily unavailable.
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
            Self::UnsupportedModel => write!(f, "Unsupported or unavailable model"),
            Self::TokenLimitExceeded => write!(f, "Token limit exceeded"),
            Self::ModelLoadFailed => write!(f, "Model loading failed"),
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

    /// Creates an unsupported model error.
    pub fn unsupported_model() -> Self {
        Self::new(ErrorKind::UnsupportedModel)
    }

    /// Creates a token limit exceeded error.
    pub fn token_limit_exceeded() -> Self {
        Self::new(ErrorKind::TokenLimitExceeded)
    }

    /// Creates a model load failed error.
    pub fn model_load_failed() -> Self {
        Self::new(ErrorKind::ModelLoadFailed)
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
