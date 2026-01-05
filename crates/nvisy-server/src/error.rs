//! Service layer error types and utilities.
//!
//! This module provides comprehensive error handling for the service layer with:
//!
//! - Strongly-typed error kinds for different failure categories
//! - Builder pattern for ergonomic error construction
//! - Type-safe error source tracking with boxed trait objects
//! - Integration with `thiserror` for automatic `Display` and `Error` trait implementations

use std::borrow::Cow;
use std::error::Error as StdError;
use std::fmt;

/// Type alias for boxed errors that are Send + Sync.
///
/// This is the standard error boxing type used throughout the service layer
/// for error sources. Using a type alias ensures consistency and reduces
/// verbosity in error type signatures.
///
/// # Thread Safety
///
/// The `Send + Sync` bounds ensure errors can be safely transferred between
/// threads and shared across thread boundaries, which is essential for async
/// Rust where tasks may move between threads.
pub type BoxedError = Box<dyn StdError + Send + Sync>;

/// Result type alias for service layer operations.
///
/// This is a convenience alias that uses [`Error`] as the error type,
/// reducing boilerplate in function signatures throughout the service layer.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Error kind enumeration for categorizing service layer errors.
///
/// This enum represents the different categories of errors that can occur
/// in the service layer. It's separated from [`Error`] to allow
/// for pattern matching on error types without accessing the full error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// Configuration-related errors.
    Config,
    /// External service communication errors.
    External,
    /// Authentication and authorization errors.
    Auth,
    /// File system operation errors.
    FileSystem,
    /// Internal service logic errors.
    Internal,
}

impl ErrorKind {
    /// Returns the error kind as a string for categorization.
    ///
    /// Useful for metrics, logging, or error categorization.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::External => "external_service",
            Self::Auth => "auth",
            Self::FileSystem => "file_system",
            Self::Internal => "internal_service",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Service layer error with structured information.
///
/// This structure provides comprehensive error information including:
///
/// - Error kind for categorization
/// - Human-readable message
/// - Optional source error for error chaining
#[derive(Debug, thiserror::Error)]
#[error("{kind} error: {message}")]
pub struct Error {
    /// The error category/type
    kind: ErrorKind,
    /// Human-readable error message
    message: Cow<'static, str>,
    /// Optional underlying error that caused this error
    #[source]
    source: Option<BoxedError>,
}

impl Error {
    /// Creates a new [`Error`].
    #[inline]
    fn new(kind: ErrorKind, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    /// Attaches a source error to this error, enabling error chain tracking.
    ///
    /// This method consumes the error and returns a new one with the source attached.
    /// It follows the builder pattern for ergonomic error construction.
    #[inline]
    pub fn with_source(mut self, source: impl StdError + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    /// Returns the error kind.
    #[must_use]
    #[inline]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns the error message.
    #[must_use]
    #[inline]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Creates a new configuration error.
    #[inline]
    pub fn config(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ErrorKind::Config, message)
    }

    /// Creates a new external service error.
    #[inline]
    pub fn external(
        service: impl Into<Cow<'static, str>>,
        message: impl Into<Cow<'static, str>>,
    ) -> Self {
        let service_name = service.into();
        let msg = message.into();
        let full_message = format!("{}: {}", service_name, msg);
        Self::new(ErrorKind::External, full_message)
    }

    /// Creates a new authentication error.
    #[inline]
    pub fn auth(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ErrorKind::Auth, message)
    }

    /// Creates a new file system error.
    #[inline]
    pub fn file_system(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ErrorKind::FileSystem, message)
    }

    /// Creates a new internal service error.
    #[inline]
    pub fn internal(
        service: impl Into<Cow<'static, str>>,
        message: impl Into<Cow<'static, str>>,
    ) -> Self {
        let service_name = service.into();
        let msg = message.into();
        let full_message = format!("{}: {}", service_name, msg);
        Self::new(ErrorKind::Internal, full_message)
    }
}

impl From<nvisy_nats::Error> for Error {
    fn from(err: nvisy_nats::Error) -> Self {
        Error::external("nats", err.to_string()).with_source(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let error = Error::config("invalid configuration");
        assert_eq!(error.kind(), ErrorKind::Config);
        assert_eq!(error.message(), "invalid configuration");
    }

    #[test]
    fn test_error_with_source() {
        let source = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error = Error::file_system("cannot read config file").with_source(source);

        assert!(StdError::source(&error).is_some());
        assert_eq!(error.kind(), ErrorKind::FileSystem);
    }

    #[test]
    fn test_external_service_error() {
        let error = Error::external("nats", "Connection refused");

        assert_eq!(error.kind(), ErrorKind::External);
        assert!(error.to_string().contains("nats"));
        assert!(error.to_string().contains("Connection refused"));
    }

    #[test]
    fn test_error_kind_as_str() {
        assert_eq!(ErrorKind::Config.as_str(), "config");
        assert_eq!(ErrorKind::External.as_str(), "external_service");
        assert_eq!(ErrorKind::Auth.as_str(), "auth");
        assert_eq!(ErrorKind::FileSystem.as_str(), "file_system");
        assert_eq!(ErrorKind::Internal.as_str(), "internal_service");
    }
}
