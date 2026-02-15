//! Structured error handling for webhook operations.

use hipstr::HipStr;
use strum::{AsRefStr, Display, EnumString, IntoStaticStr};
use thiserror::Error;

/// Type alias for boxed dynamic errors that can be sent across threads.
pub type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// Type alias for Results with our custom Error type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Categories of errors that can occur in webhook operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[derive(AsRefStr, Display, EnumString, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum ErrorKind {
    /// Input validation failed.
    InvalidInput,
    /// Network-related error occurred.
    NetworkError,
    /// Authentication failed.
    Authentication,
    /// Authorization failed.
    Authorization,
    /// Rate limit exceeded.
    RateLimited,
    /// Service temporarily unavailable.
    ServiceUnavailable,
    /// Internal service error.
    InternalError,
    /// External service error.
    ExternalError,
    /// Configuration error.
    Configuration,
    /// Resource not found.
    NotFound,
    /// Timeout occurred.
    Timeout,
    /// Serialization/deserialization error.
    Serialization,
    /// Unknown error occurred.
    #[default]
    Unknown,
}

impl ErrorKind {
    /// Check if this error kind is typically retryable.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::NetworkError | Self::Timeout | Self::ServiceUnavailable | Self::RateLimited
        )
    }
}

/// Structured error type with classification and context tracking.
#[must_use]
#[derive(Debug, Error)]
#[error("[{kind}]{}", message.as_ref().map(|m| format!(": {m}")).unwrap_or_default())]
pub struct Error {
    /// The kind of error that occurred.
    pub kind: ErrorKind,
    /// Primary error message.
    pub message: Option<HipStr<'static>>,
    /// Underlying source error, if any.
    #[source]
    pub source: Option<BoxedError>,
    /// Additional context information.
    pub context: Option<HipStr<'static>>,
}

impl Error {
    /// Creates a new error with the given kind.
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            message: None,
            source: None,
            context: None,
        }
    }

    /// Creates a new error from a source error.
    pub fn from_source(kind: ErrorKind, source: impl Into<BoxedError>) -> Self {
        Self {
            kind,
            message: None,
            source: Some(source.into()),
            context: None,
        }
    }

    /// Adds a message to this error.
    pub fn with_message(mut self, message: impl Into<HipStr<'static>>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Sets the source of the error.
    pub fn with_source(mut self, source: impl Into<BoxedError>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Adds context to the error.
    pub fn with_context(mut self, context: impl Into<HipStr<'static>>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Check if this error is retryable based on its kind.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        self.kind.is_retryable()
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::from_source(ErrorKind::InternalError, error).with_message("I/O operation failed")
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(error: std::string::FromUtf8Error) -> Self {
        Self::from_source(ErrorKind::Serialization, error).with_message("Invalid UTF-8 encoding")
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(error: std::str::Utf8Error) -> Self {
        Self::from_source(ErrorKind::Serialization, error).with_message("Invalid UTF-8 encoding")
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_error_new() {
        let error = Error::new(ErrorKind::Unknown);
        assert_eq!(error.kind, ErrorKind::Unknown);
        assert!(error.message.is_none());
        assert!(error.source.is_none());
        assert!(error.context.is_none());
    }

    #[test]
    fn test_error_builder_pattern() {
        let error = Error::new(ErrorKind::Configuration)
            .with_message("bad config")
            .with_context("additional context");

        assert_eq!(error.kind, ErrorKind::Configuration);
        assert_eq!(error.message.as_deref(), Some("bad config"));
        assert_eq!(error.context.as_deref(), Some("additional context"));
    }

    #[test]
    fn test_error_display() {
        let error = Error::new(ErrorKind::InternalError).with_message("test error");

        let display_str = error.to_string();
        assert!(display_str.contains("internal_error"));
        assert!(display_str.contains("test error"));
    }

    #[test]
    fn test_error_from_io() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error = Error::from(io_error);

        assert_eq!(error.kind, ErrorKind::InternalError);
        assert_eq!(error.message.as_deref(), Some("I/O operation failed"));
        assert!(error.source.is_some());
    }

    #[test]
    fn test_is_retryable() {
        let retryable = Error::new(ErrorKind::NetworkError);
        assert!(retryable.is_retryable());

        let retryable = Error::new(ErrorKind::ServiceUnavailable);
        assert!(retryable.is_retryable());

        let not_retryable = Error::new(ErrorKind::InvalidInput);
        assert!(!not_retryable.is_retryable());
    }

    #[test]
    fn test_from_source() {
        let source = std::io::Error::other("underlying error");
        let error = Error::from_source(ErrorKind::ExternalError, source);

        assert!(error.source.is_some());
        assert_eq!(error.kind, ErrorKind::ExternalError);
    }

    #[test]
    fn test_default() {
        assert_eq!(ErrorKind::default(), ErrorKind::Unknown);
    }

    #[test]
    fn test_from_str() {
        assert_eq!(
            ErrorKind::from_str("not_found").unwrap(),
            ErrorKind::NotFound
        );
        assert_eq!(ErrorKind::from_str("timeout").unwrap(), ErrorKind::Timeout);
        assert_eq!(ErrorKind::from_str("unknown").unwrap(), ErrorKind::Unknown);
        assert!(ErrorKind::from_str("invalid").is_err());
    }

    #[test]
    fn test_retryable() {
        assert!(ErrorKind::NetworkError.is_retryable());
        assert!(ErrorKind::Timeout.is_retryable());
        assert!(ErrorKind::ServiceUnavailable.is_retryable());
        assert!(ErrorKind::RateLimited.is_retryable());

        assert!(!ErrorKind::InvalidInput.is_retryable());
        assert!(!ErrorKind::Authentication.is_retryable());
        assert!(!ErrorKind::NotFound.is_retryable());
        assert!(!ErrorKind::Unknown.is_retryable());
    }
}
