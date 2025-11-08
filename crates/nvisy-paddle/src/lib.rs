#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! # nvisy-paddle
//!
//! A production-ready HTTP client for PaddleOCR and PaddleX services.
//!
//! This crate provides a high-level client for interacting with PaddleOCR-VL and other
//! PaddleX pipelines via HTTP API, with comprehensive error handling, observability,
//! and type-safe request/response structures.
//!
//! ## Features
//!
//! - **Client**: HTTP client for PaddleOCR-VL document parsing
//! - **Error Handling**: Comprehensive error types with recovery strategies
//! - **Type Safety**: Strongly-typed request and response structures
//! - **Observability**: Structured logging and tracing integration
//!
//! ## Quick Start
//!
//! ```ignore
//! use nvisy_paddle::{PdClient, PdConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), nvisy_paddle::Error> {
//!     let config = PdConfig::new("http://localhost:8080");
//!     let client = PdClient::new(config);
//!
//!     // Parse a document with PaddleOCR-VL
//!     let result = client.parse_document("path/to/document.pdf").await?;
//!     println!("Parsed content: {}", result.content);
//!
//!     Ok(())
//! }
//! ```

use std::time::Duration;

// Tracing targets for observability
/// Logging target for PaddleX client operations.
pub const PADDLEX_TARGET: &str = "nvisy_paddle::client";

/// Logging target for PaddleOCR-VL operations.
pub const PADDLEOCR_VL_TARGET: &str = "nvisy_paddle::ocr_vl";

/// Logging target for HTTP requests and responses.
pub const HTTP_TARGET: &str = "nvisy_paddle::http";

// Core modules
pub mod client;

pub use client::{PdClient, PdConfig};

/// Result type for all PaddleX operations in this crate.
///
/// This is a convenience type alias that defaults to using [`Error`] as the error type.
/// Most functions in this crate return this type for consistent error handling.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Comprehensive error types for PaddleX operations.
///
/// This enum covers all possible failure modes when interacting with PaddleX services,
/// providing detailed context and appropriate error handling strategies.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// HTTP client errors (connection, timeout, etc.)
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// API returned an error response
    #[error("API error (status {status}): {message}")]
    Api {
        /// HTTP status code
        status: u16,
        /// Error message from the API
        message: String,
        /// Optional error code from the API
        code: Option<String>,
    },

    /// Invalid or malformed API response
    #[error("Invalid response: {message}")]
    InvalidResponse {
        /// Description of what's invalid
        message: String,
        /// Optional raw response body for debugging
        body: Option<String>,
    },

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Configuration errors
    #[error("Configuration error: {message}")]
    Config {
        /// Description of the configuration problem
        message: String,
    },

    /// Request timeout
    #[error("Request timed out after {timeout:?}")]
    Timeout {
        /// Duration before timeout occurred
        timeout: Duration,
    },

    /// Rate limiting errors
    #[error("Rate limit exceeded: {message}")]
    RateLimit {
        /// Details about the rate limit violation
        message: String,
        /// Time until rate limit resets (if known)
        retry_after: Option<Duration>,
    },

    /// Service unavailable
    #[error("Service unavailable: {message}")]
    ServiceUnavailable {
        /// Description of the unavailability
        message: String,
        /// Optional retry delay suggestion
        retry_after: Option<Duration>,
    },

    /// File I/O errors
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Unsupported file format or operation
    #[error("Unsupported: {message}")]
    Unsupported {
        /// Description of what's unsupported
        message: String,
    },

    /// Invalid input data
    #[error("Invalid input: {message}")]
    InvalidInput {
        /// Description of what's invalid
        message: String,
    },

    /// Generic operation error with context
    #[error("Operation failed: {operation} - {details}")]
    Operation {
        /// Name of the operation that failed
        operation: String,
        /// Additional details about the failure
        details: String,
    },
}

impl Error {
    /// Create an API error
    pub fn api(status: u16, message: impl Into<String>, code: Option<String>) -> Self {
        Self::Api {
            status,
            message: message.into(),
            code,
        }
    }

    /// Create an invalid response error
    pub fn invalid_response(message: impl Into<String>, body: Option<String>) -> Self {
        Self::InvalidResponse {
            message: message.into(),
            body,
        }
    }

    /// Create a configuration error
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    /// Create a timeout error
    pub fn timeout(timeout: Duration) -> Self {
        Self::Timeout { timeout }
    }

    /// Create a rate limit error
    pub fn rate_limit(message: impl Into<String>, retry_after: Option<Duration>) -> Self {
        Self::RateLimit {
            message: message.into(),
            retry_after,
        }
    }

    /// Create a service unavailable error
    pub fn service_unavailable(message: impl Into<String>, retry_after: Option<Duration>) -> Self {
        Self::ServiceUnavailable {
            message: message.into(),
            retry_after,
        }
    }

    /// Create an unsupported error
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::Unsupported {
            message: message.into(),
        }
    }

    /// Create an invalid input error
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }

    /// Create an operation error
    pub fn operation(operation: impl Into<String>, details: impl Into<String>) -> Self {
        Self::Operation {
            operation: operation.into(),
            details: details.into(),
        }
    }

    /// Check if this error indicates a temporary failure that might succeed on retry
    pub fn is_retryable(&self) -> bool {
        match self {
            // Always retryable
            Error::Timeout { .. } | Error::RateLimit { .. } | Error::ServiceUnavailable { .. } => {
                true
            }

            // HTTP errors - check if they're network-related
            Error::Http(err) => err.is_timeout() || err.is_connect() || err.is_request(),

            // API errors - 5xx and 429 are retryable
            Error::Api { status, .. } => matches!(*status, 429 | 500..=599),

            // Never retryable
            Error::Serialization(_)
            | Error::Config { .. }
            | Error::InvalidResponse { .. }
            | Error::Io(_)
            | Error::Unsupported { .. }
            | Error::InvalidInput { .. }
            | Error::Operation { .. } => false,
        }
    }

    /// Get the HTTP status code if this is an HTTP/API error
    pub fn status_code(&self) -> Option<u16> {
        match self {
            Error::Api { status, .. } => Some(*status),
            Error::Http(err) => err.status().map(|s| s.as_u16()),
            _ => None,
        }
    }

    /// Get the retry delay if this error provides one
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Error::RateLimit { retry_after, .. }
            | Error::ServiceUnavailable { retry_after, .. } => *retry_after,
            _ => None,
        }
    }

    /// Get the error category for metrics/logging
    pub fn category(&self) -> &'static str {
        match self {
            Error::Http(_) => "http",
            Error::Api { .. } => "api",
            Error::InvalidResponse { .. } => "invalid_response",
            Error::Serialization(_) => "serialization",
            Error::Config { .. } => "config",
            Error::Timeout { .. } => "timeout",
            Error::RateLimit { .. } => "rate_limit",
            Error::ServiceUnavailable { .. } => "service_unavailable",
            Error::Io(_) => "io",
            Error::Unsupported { .. } => "unsupported",
            Error::InvalidInput { .. } => "invalid_input",
            Error::Operation { .. } => "operation",
        }
    }

    /// Check if this is a client-side error (programming/configuration issue)
    pub fn is_client_error(&self) -> bool {
        match self {
            Error::Config { .. }
            | Error::InvalidInput { .. }
            | Error::Unsupported { .. }
            | Error::Serialization(_)
            | Error::Io(_) => true,
            Error::Api { status, .. } => (400..500).contains(status) && *status != 429,
            _ => false,
        }
    }

    /// Check if this is a server-side error
    pub fn is_server_error(&self) -> bool {
        match self {
            Error::ServiceUnavailable { .. } => true,
            Error::Api { status, .. } => (500..600).contains(status),
            _ => false,
        }
    }

    /// Get a user-friendly error message suitable for display
    pub fn user_message(&self) -> String {
        match self {
            Error::Http(_) => "Network error occurred. Please check your connection.".to_string(),
            Error::Timeout { timeout } => {
                format!("Request timed out after {:?}. Please try again.", timeout)
            }
            Error::RateLimit { message, .. } => {
                format!("Rate limit exceeded: {}. Please try again later.", message)
            }
            Error::ServiceUnavailable { message, .. } => {
                format!("Service is temporarily unavailable: {}.", message)
            }
            Error::InvalidInput { message } => format!("Invalid input: {}", message),
            Error::Unsupported { message } => format!("Unsupported: {}", message),
            Error::Config { message } => format!("Configuration error: {}", message),
            _ => "An unexpected error occurred. Please try again.".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categories() {
        let timeout_err = Error::timeout(Duration::from_secs(30));
        assert_eq!(timeout_err.category(), "timeout");
        assert!(timeout_err.is_retryable());

        let api_err = Error::api(500, "Internal server error", None);
        assert_eq!(api_err.category(), "api");
        assert!(api_err.is_retryable());
        assert!(api_err.is_server_error());

        let config_err = Error::config("Missing API key");
        assert_eq!(config_err.category(), "config");
        assert!(!config_err.is_retryable());
        assert!(config_err.is_client_error());
    }

    #[test]
    fn test_retryable_errors() {
        assert!(Error::timeout(Duration::from_secs(10)).is_retryable());
        assert!(Error::rate_limit("Too many requests", None).is_retryable());
        assert!(Error::service_unavailable("Maintenance", None).is_retryable());
        assert!(Error::api(503, "Service unavailable", None).is_retryable());
        assert!(Error::api(429, "Rate limited", None).is_retryable());

        assert!(!Error::api(400, "Bad request", None).is_retryable());
        assert!(!Error::config("Invalid config").is_retryable());
        assert!(!Error::invalid_input("Bad data").is_retryable());
    }

    #[test]
    fn test_status_code() {
        let api_err = Error::api(404, "Not found", None);
        assert_eq!(api_err.status_code(), Some(404));

        let timeout_err = Error::timeout(Duration::from_secs(10));
        assert_eq!(timeout_err.status_code(), None);
    }

    #[test]
    fn test_retry_after() {
        let retry_duration = Duration::from_secs(60);
        let rate_limit_err = Error::rate_limit("Limit exceeded", Some(retry_duration));
        assert_eq!(rate_limit_err.retry_after(), Some(retry_duration));

        let config_err = Error::config("Bad config");
        assert_eq!(config_err.retry_after(), None);
    }
}
