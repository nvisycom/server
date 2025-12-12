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

use thiserror::Error;

/// Result type alias for VLM operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for VLM operations.
///
/// This error type provides structured information about what went wrong during
/// VLM processing, including the specific error kind and optional source error
/// for better debugging and error handling.
#[derive(Debug, Error)]
#[error("{kind}")]
pub struct Error {
    /// The specific kind of error that occurred.
    pub kind: ErrorKind,
    /// Optional source error for additional context.
    #[source]
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl Error {
    /// Creates a new error with the given kind.
    pub fn new(kind: ErrorKind) -> Self {
        Self { kind, source: None }
    }

    /// Creates a new error with the given kind and source error.
    pub fn with_source(kind: ErrorKind, source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self {
            kind,
            source: Some(source),
        }
    }

    /// Returns true if this is a client error (4xx-style).
    ///
    /// Client errors indicate problems with the request that the client
    /// should fix before retrying, such as authentication issues or
    /// invalid prompt parameters.
    pub fn is_client_error(&self) -> bool {
        matches!(
            self.kind,
            ErrorKind::Authentication
                | ErrorKind::InvalidInput
                | ErrorKind::UnsupportedImageFormat
                | ErrorKind::ImageTooLarge
                | ErrorKind::ImageTooSmall
                | ErrorKind::UnsupportedModel
                | ErrorKind::InvalidPrompt
        )
    }

    /// Returns true if this is a server error (5xx-style).
    ///
    /// Server errors indicate problems on the service side that are
    /// typically outside the client's control.
    pub fn is_server_error(&self) -> bool {
        matches!(
            self.kind,
            ErrorKind::ServiceUnavailable | ErrorKind::ServiceOverloaded | ErrorKind::InternalError
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
                | ErrorKind::ServiceOverloaded
        )
    }

    /// Returns the suggested retry delay for retryable errors.
    ///
    /// Returns `None` for non-retryable errors. The delay duration
    /// is based on the error type and follows common retry patterns.
    pub fn retry_delay(&self) -> Option<Duration> {
        match self.kind {
            ErrorKind::RateLimited => Some(Duration::from_secs(60)),
            ErrorKind::ServiceOverloaded => Some(Duration::from_secs(30)),
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
#[derive(Debug, Error)]
pub enum ErrorKind {
    /// Authentication with the VLM service failed.
    #[error("Authentication failed")]
    Authentication,

    /// The input provided to the VLM service is invalid.
    #[error("Invalid input provided")]
    InvalidInput,

    /// The image format is not supported by the VLM service.
    #[error("Unsupported image format")]
    UnsupportedImageFormat,

    /// The image file exceeds the maximum size limit.
    #[error("Image file is too large")]
    ImageTooLarge,

    /// The image resolution is too small for reliable processing.
    #[error("Image resolution is too small")]
    ImageTooSmall,

    /// The prompt provided is invalid or malformed.
    #[error("Invalid prompt")]
    InvalidPrompt,

    /// The specified model is not supported or available.
    #[error("Unsupported model")]
    UnsupportedModel,

    /// Model inference failed during processing.
    #[error("Model inference failed")]
    ModelInferenceFailed,

    /// Rate limit has been exceeded.
    #[error("Rate limit exceeded")]
    RateLimited,

    /// A network error occurred during the request.
    #[error("Network error occurred")]
    NetworkError,

    /// The operation timed out.
    #[error("Operation timed out")]
    Timeout,

    /// The VLM service is temporarily unavailable.
    #[error("Service unavailable")]
    ServiceUnavailable,

    /// The VLM service is overloaded.
    #[error("Service overloaded")]
    ServiceOverloaded,

    /// The requested feature is not supported.
    #[error("Unsupported feature")]
    UnsupportedFeature,

    /// An internal service error occurred.
    #[error("Internal service error")]
    InternalError,

    /// Failed to parse response or input data.
    #[error("Parse error")]
    ParseError,
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
        Self::new(ErrorKind::UnsupportedImageFormat)
    }

    /// Creates an image too large error.
    pub fn image_too_large() -> Self {
        Self::new(ErrorKind::ImageTooLarge)
    }

    /// Creates an invalid prompt error.
    pub fn invalid_prompt() -> Self {
        Self::new(ErrorKind::InvalidPrompt)
    }

    /// Creates an unsupported model error.
    pub fn unsupported_model() -> Self {
        Self::new(ErrorKind::UnsupportedModel)
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

    /// Creates an authentication error with source.
    pub fn authentication_with_source(source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::with_source(ErrorKind::Authentication, source)
    }

    /// Creates a network error with source.
    pub fn network_error_with_source(source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::with_source(ErrorKind::NetworkError, source)
    }

    /// Creates a model inference error with source.
    pub fn model_inference_failed_with_source(
        source: Box<dyn std::error::Error + Send + Sync>,
    ) -> Self {
        Self::with_source(ErrorKind::ModelInferenceFailed, source)
    }

    /// Creates an internal error with source.
    pub fn internal_error_with_source(source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::with_source(ErrorKind::InternalError, source)
    }
}
