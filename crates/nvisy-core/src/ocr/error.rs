//! Error handling for OCR operations.
//!
//! This module provides comprehensive error types for OCR services, including
//! classification of errors into client vs server errors, retry policies, and
//! structured error information to help with debugging and error handling.
//!
//! # Error Classification
//!
//! Errors are classified into several categories:
//!
//! - **Client Errors**: Authentication failures, invalid input, unsupported formats
//! - **Server Errors**: Service unavailable, internal errors, overloaded services
//! - **Retryable Errors**: Network issues, timeouts, rate limits, service problems
//! - **Non-retryable Errors**: Authentication, invalid input, unsupported features
//!
//! # Examples
//!
//! ```rust
//! use nvisy_core::ocr::Error;
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

/// Result type alias for OCR operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for OCR operations.
///
/// This error type provides structured information about what went wrong during
/// OCR processing, including the specific error kind and optional source error
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
    /// invalid input parameters.
    pub fn is_client_error(&self) -> bool {
        matches!(
            self.kind,
            ErrorKind::Authentication
                | ErrorKind::InvalidInput
                | ErrorKind::UnsupportedImageFormat
                | ErrorKind::ImageTooLarge
                | ErrorKind::ImageTooSmall
                | ErrorKind::PoorImageQuality
                | ErrorKind::UnsupportedLanguage
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

/// Specific kinds of OCR errors.
///
/// This enum categorizes all possible error conditions that can occur
/// during OCR operations, from authentication failures to processing errors.
#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
    /// Authentication with the OCR service failed.
    #[error("Authentication failed")]
    Authentication,

    /// The input provided to the OCR service is invalid.
    #[error("Invalid input provided")]
    InvalidInput,

    /// The image format is not supported by the OCR service.
    #[error("Unsupported image format")]
    UnsupportedImageFormat,

    /// The image file exceeds the maximum size limit.
    #[error("Image file is too large")]
    ImageTooLarge,

    /// The image resolution is too small for reliable OCR.
    #[error("Image resolution is too small")]
    ImageTooSmall,

    /// The image quality is too poor for accurate text extraction.
    #[error("Poor image quality detected")]
    PoorImageQuality,

    /// The specified language is not supported.
    #[error("Unsupported language")]
    UnsupportedLanguage,

    /// Language detection failed to identify the text language.
    #[error("Language detection failed")]
    LanguageDetectionFailed,

    /// No readable text was detected in the image.
    #[error("No text detected in image")]
    NoTextDetected,

    /// The text extraction process failed.
    #[error("Text extraction failed")]
    TextExtractionFailed,

    /// Document parsing failed during processing.
    #[error("Document parsing failed")]
    DocumentParsingFailed,

    /// Rate limit has been exceeded.
    #[error("Rate limit exceeded")]
    RateLimited,

    /// A network error occurred during the request.
    #[error("Network error occurred")]
    NetworkError,

    /// The operation timed out.
    #[error("Operation timed out")]
    Timeout,

    /// The OCR service is temporarily unavailable.
    #[error("Service unavailable")]
    ServiceUnavailable,

    /// The OCR service is overloaded.
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

    /// Creates a poor image quality error.
    pub fn poor_image_quality() -> Self {
        Self::new(ErrorKind::PoorImageQuality)
    }

    /// Creates a no text detected error.
    pub fn no_text_detected() -> Self {
        Self::new(ErrorKind::NoTextDetected)
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
