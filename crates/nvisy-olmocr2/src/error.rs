//! Error types for nvisy-olmocr2
//!
//! This module provides comprehensive error handling for the OCR client library.

use std::time::Duration;

/// Result type for all OCR operations in this crate.
///
/// This is a convenience type alias that defaults to using [`Error`] as the error type.
/// Most functions in this crate return this type for consistent error handling.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Unified error type for OCR operations
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// HTTP client/connection errors
    #[error("HTTP client error: {0}")]
    Http(#[from] reqwest::Error),

    /// Serialization errors when sending or receiving data
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Base64 encoding/decoding errors
    #[error("Base64 encoding error: {0}")]
    Base64(#[from] base64::DecodeError),

    /// URL parsing errors
    #[error("URL parsing error: {0}")]
    UrlParse(#[from] url::ParseError),

    /// Operation timeout
    #[error("Operation timed out after {timeout:?}")]
    Timeout { timeout: Duration },

    /// OCR API error response
    #[error("OCR API error: {status} - {message}")]
    ApiError { status: u16, message: String },

    /// Document processing failed
    #[error("Document processing failed for '{document}': {reason}")]
    ProcessingFailed { document: String, reason: String },

    /// Model not found or unavailable
    #[error("Model '{model}' not available: {reason}")]
    ModelUnavailable { model: String, reason: String },

    /// Unsupported document format
    #[error("Unsupported document format: {format}. Supported formats: {supported:?}")]
    UnsupportedFormat {
        format: String,
        supported: Vec<String>,
    },

    /// Document size exceeds limits
    #[error("Document size {size} bytes exceeds limit of {limit} bytes")]
    DocumentTooLarge { size: usize, limit: usize },

    /// Invalid image data
    #[error("Invalid image data: {reason}")]
    InvalidImage { reason: String },

    /// OCR confidence below threshold
    #[error("OCR confidence {confidence:.2} below minimum threshold {threshold:.2}")]
    LowConfidence { confidence: f64, threshold: f64 },

    /// Batch processing error
    #[error("Batch processing error: {processed}/{total} documents processed successfully")]
    BatchProcessingError { processed: usize, total: usize },

    /// Authentication/authorization error
    #[error("Authentication failed: {reason}")]
    AuthError { reason: String },

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {retry_after:?}")]
    RateLimited { retry_after: Option<Duration> },

    /// Invalid configuration
    #[error("Invalid configuration: {reason}")]
    InvalidConfig { reason: String },

    /// Generic operation error with context
    #[error("OCR operation failed: {operation} - {details}")]
    Operation { operation: String, details: String },
}

impl Error {
    /// Create an API error
    pub fn api_error(status: u16, message: impl Into<String>) -> Self {
        Self::ApiError {
            status,
            message: message.into(),
        }
    }

    /// Create a processing failed error
    pub fn processing_failed(document: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ProcessingFailed {
            document: document.into(),
            reason: reason.into(),
        }
    }

    /// Create a model unavailable error
    pub fn model_unavailable(model: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ModelUnavailable {
            model: model.into(),
            reason: reason.into(),
        }
    }

    /// Create an unsupported format error
    pub fn unsupported_format(format: impl Into<String>, supported: Vec<String>) -> Self {
        Self::UnsupportedFormat {
            format: format.into(),
            supported,
        }
    }

    /// Create a document too large error
    pub fn document_too_large(size: usize, limit: usize) -> Self {
        Self::DocumentTooLarge { size, limit }
    }

    /// Create an invalid image error
    pub fn invalid_image(reason: impl Into<String>) -> Self {
        Self::InvalidImage {
            reason: reason.into(),
        }
    }

    /// Create a low confidence error
    pub fn low_confidence(confidence: f64, threshold: f64) -> Self {
        Self::LowConfidence {
            confidence,
            threshold,
        }
    }

    /// Create a batch processing error
    pub fn batch_processing_error(processed: usize, total: usize) -> Self {
        Self::BatchProcessingError { processed, total }
    }

    /// Create an authentication error
    pub fn auth_error(reason: impl Into<String>) -> Self {
        Self::AuthError {
            reason: reason.into(),
        }
    }

    /// Create a rate limited error
    pub fn rate_limited(retry_after: Option<Duration>) -> Self {
        Self::RateLimited { retry_after }
    }

    /// Create an invalid configuration error
    pub fn invalid_config(reason: impl Into<String>) -> Self {
        Self::InvalidConfig {
            reason: reason.into(),
        }
    }

    /// Create a timeout error with the given duration
    pub fn timeout(duration: Duration) -> Self {
        Self::Timeout { timeout: duration }
    }

    /// Create an operation error with context
    pub fn operation(op: impl Into<String>, details: impl Into<String>) -> Self {
        Self::Operation {
            operation: op.into(),
            details: details.into(),
        }
    }

    /// Get a user-friendly error message suitable for display
    pub fn user_message(&self) -> String {
        match self {
            Error::Http(_) => {
                "Network connection failed. Please check your internet connection.".to_string()
            }
            Error::Timeout { timeout } => {
                format!(
                    "OCR processing timed out after {:?}. Please try again with a smaller document.",
                    timeout
                )
            }
            Error::UnsupportedFormat { format, supported } => {
                format!(
                    "Unsupported file format '{}'. Please use one of: {}",
                    format,
                    supported.join(", ")
                )
            }
            Error::DocumentTooLarge { size, limit } => {
                format!(
                    "Document is too large ({:.1} MB). Maximum size is {:.1} MB.",
                    *size as f64 / 1024.0 / 1024.0,
                    *limit as f64 / 1024.0 / 1024.0
                )
            }
            Error::InvalidImage { .. } => {
                "Invalid image file. Please check the image format and try again.".to_string()
            }
            Error::LowConfidence {
                confidence,
                threshold,
            } => {
                format!(
                    "OCR confidence ({:.1}%) is below the minimum threshold ({:.1}%). Results may be inaccurate.",
                    confidence * 100.0,
                    threshold * 100.0
                )
            }
            Error::AuthError { .. } => {
                "Authentication failed. Please check your credentials.".to_string()
            }
            Error::RateLimited { retry_after } => match retry_after {
                Some(duration) => format!(
                    "Rate limit exceeded. Please wait {:?} before trying again.",
                    duration
                ),
                None => "Rate limit exceeded. Please try again later.".to_string(),
            },
            Error::ApiError { status, message } => {
                format!("OCR service error ({}): {}", status, message)
            }
            Error::InvalidConfig { reason } => format!("Configuration error: {}", reason),
            _ => {
                "An unexpected error occurred during OCR processing. Please try again.".to_string()
            }
        }
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            Error::Http(err) => err.is_timeout() || err.is_connect(),
            Error::Timeout { .. } => true,
            Error::ApiError { status, .. } => *status >= 500 || *status == 429,
            Error::RateLimited { .. } => true,
            _ => false,
        }
    }

    /// Get suggested retry delay for retryable errors
    pub fn retry_delay(&self) -> Option<Duration> {
        match self {
            Error::RateLimited { retry_after } => *retry_after,
            Error::Timeout { .. } => Some(Duration::from_secs(1)),
            Error::Http(_) => Some(Duration::from_millis(500)),
            Error::ApiError { status, .. } if *status >= 500 => Some(Duration::from_secs(2)),
            _ => None,
        }
    }
}

// Import builder error type for From implementation
use crate::client::OlmBuilderError;

impl From<OlmBuilderError> for Error {
    fn from(err: OlmBuilderError) -> Self {
        Error::InvalidConfig {
            reason: err.to_string(),
        }
    }
}
