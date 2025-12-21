//! Error types and utilities for Qdrant operations.

use std::time::Duration;

use thiserror::Error;

/// Type alias for boxed dynamic errors that can be sent across threads.
pub type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// Type alias for Results with our custom Error type.
pub type Result<T> = std::result::Result<T, Error>;

/// Categories of errors that can occur in nvisy-qdrant operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Input validation failed.
    InvalidInput,
    /// Network-related error occurred.
    NetworkError,
    /// Resource not found.
    NotFound,
    /// Timeout occurred.
    Timeout,
    /// Serialization/deserialization error.
    Serialization,
    /// Configuration error.
    Configuration,
    /// Connection error.
    Connection,
    /// Collection operation error.
    Collection,
    /// Point operation error.
    Point,
    /// Search operation error.
    Search,
    /// Batch operation error.
    Batch,
    /// Rate limit exceeded.
    RateLimited,
    /// Authentication failed.
    Authentication,
    /// Server error.
    ServerError,
    /// Unknown error occurred.
    Unknown,
}

/// A structured error type for nvisy-qdrant operations.
#[derive(Debug, Error)]
#[error("{kind:?}{}", message.as_ref().map(|m| format!(": {}", m)).unwrap_or_default())]
pub struct Error {
    /// The kind of error that occurred.
    pub kind: ErrorKind,
    /// Optional error message.
    pub message: Option<String>,
    /// Optional source error.
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
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Adds a source error to this error.
    pub fn with_source(mut self, source: BoxedError) -> Self {
        self.source = Some(source);
        self
    }

    /// Creates a new invalid input error.
    pub fn invalid_input() -> Self {
        Self::new(ErrorKind::InvalidInput)
    }

    /// Creates a new network error.
    pub fn network_error() -> Self {
        Self::new(ErrorKind::NetworkError)
    }

    /// Creates a new not found error.
    pub fn not_found() -> Self {
        Self::new(ErrorKind::NotFound)
    }

    /// Creates a new timeout error.
    pub fn timeout() -> Self {
        Self::new(ErrorKind::Timeout)
    }

    /// Creates a new serialization error.
    pub fn serialization() -> Self {
        Self::new(ErrorKind::Serialization)
    }

    /// Creates a new configuration error.
    pub fn configuration() -> Self {
        Self::new(ErrorKind::Configuration)
    }

    /// Creates a new connection error.
    pub fn connection() -> Self {
        Self::new(ErrorKind::Connection)
    }

    /// Creates a new collection error.
    pub fn collection() -> Self {
        Self::new(ErrorKind::Collection)
    }

    /// Creates a new point error.
    pub fn point() -> Self {
        Self::new(ErrorKind::Point)
    }

    /// Creates a new search error.
    pub fn search() -> Self {
        Self::new(ErrorKind::Search)
    }

    /// Creates a new batch error.
    pub fn batch() -> Self {
        Self::new(ErrorKind::Batch)
    }

    /// Creates a new rate limited error.
    pub fn rate_limited() -> Self {
        Self::new(ErrorKind::RateLimited)
    }

    /// Creates a new authentication error.
    pub fn authentication() -> Self {
        Self::new(ErrorKind::Authentication)
    }

    /// Creates a new server error.
    pub fn server_error() -> Self {
        Self::new(ErrorKind::ServerError)
    }

    /// Creates a new unknown error.
    pub fn unknown() -> Self {
        Self::new(ErrorKind::Unknown)
    }

    /// Returns true if this is a client error (4xx equivalent).
    pub fn is_client_error(&self) -> bool {
        matches!(
            self.kind,
            ErrorKind::InvalidInput
                | ErrorKind::Authentication
                | ErrorKind::NotFound
                | ErrorKind::RateLimited
        )
    }

    /// Returns true if this is a server error (5xx equivalent).
    pub fn is_server_error(&self) -> bool {
        matches!(
            self.kind,
            ErrorKind::ServerError
                | ErrorKind::Connection
                | ErrorKind::Configuration
                | ErrorKind::Timeout
                | ErrorKind::Unknown
        )
    }

    /// Returns true if this error is potentially retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self.kind,
            ErrorKind::NetworkError
                | ErrorKind::RateLimited
                | ErrorKind::Timeout
                | ErrorKind::Connection
        )
    }

    /// Returns the recommended retry delay for this error.
    pub fn retry_delay(&self) -> Option<Duration> {
        match self.kind {
            ErrorKind::RateLimited => Some(Duration::from_secs(60)),
            ErrorKind::NetworkError => Some(Duration::from_secs(5)),
            ErrorKind::Timeout => Some(Duration::from_secs(10)),
            ErrorKind::Connection => Some(Duration::from_secs(5)),
            _ => None,
        }
    }

    /// Returns true if this is an authentication error.
    pub fn is_auth_error(&self) -> bool {
        matches!(self.kind, ErrorKind::Authentication)
    }

    /// Returns true if this is a rate limiting error.
    pub fn is_rate_limit_error(&self) -> bool {
        matches!(self.kind, ErrorKind::RateLimited)
    }

    /// Returns true if this is a timeout error.
    pub fn is_timeout_error(&self) -> bool {
        matches!(self.kind, ErrorKind::Timeout)
    }

    /// Returns true if this is a network error.
    pub fn is_network_error(&self) -> bool {
        matches!(self.kind, ErrorKind::NetworkError)
    }

    /// Returns true if this is a not found error.
    pub fn is_not_found(&self) -> bool {
        matches!(self.kind, ErrorKind::NotFound)
    }

    /// Returns true if this is a collection error.
    pub fn is_collection(&self) -> bool {
        matches!(self.kind, ErrorKind::Collection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let error = Error::new(ErrorKind::InvalidInput);
        assert_eq!(error.kind, ErrorKind::InvalidInput);
        assert!(error.message.is_none());
        assert!(error.source.is_none());
    }

    #[test]
    fn test_error_with_message() {
        let error = Error::invalid_input().with_message("Test message");
        assert_eq!(error.kind, ErrorKind::InvalidInput);
        assert_eq!(error.message.as_ref().unwrap(), "Test message");
    }

    #[test]
    fn test_client_error_classification() {
        assert!(Error::authentication().is_client_error());
        assert!(Error::invalid_input().is_client_error());
        assert!(Error::not_found().is_client_error());
        assert!(Error::rate_limited().is_client_error());
    }

    #[test]
    fn test_server_error_classification() {
        assert!(Error::server_error().is_server_error());
        assert!(Error::connection().is_server_error());
        assert!(Error::configuration().is_server_error());
        assert!(Error::timeout().is_server_error());
        assert!(Error::unknown().is_server_error());
    }

    #[test]
    fn test_retryable_classification() {
        assert!(Error::rate_limited().is_retryable());
        assert!(Error::network_error().is_retryable());
        assert!(Error::timeout().is_retryable());
        assert!(Error::connection().is_retryable());
        assert!(!Error::authentication().is_retryable());
        assert!(!Error::invalid_input().is_retryable());
    }

    #[test]
    fn test_retry_delays() {
        assert_eq!(
            Error::rate_limited().retry_delay(),
            Some(Duration::from_secs(60))
        );
        assert_eq!(
            Error::network_error().retry_delay(),
            Some(Duration::from_secs(5))
        );
        assert_eq!(
            Error::timeout().retry_delay(),
            Some(Duration::from_secs(10))
        );
        assert_eq!(
            Error::connection().retry_delay(),
            Some(Duration::from_secs(5))
        );
        assert_eq!(Error::invalid_input().retry_delay(), None);
    }

    #[test]
    fn test_specialized_error_checks() {
        assert!(Error::authentication().is_auth_error());
        assert!(Error::rate_limited().is_rate_limit_error());
        assert!(Error::timeout().is_timeout_error());
        assert!(Error::network_error().is_network_error());
        assert!(Error::not_found().is_not_found());
        assert!(!Error::invalid_input().is_auth_error());
        assert!(!Error::authentication().is_rate_limit_error());
    }

    #[test]
    fn test_error_display() {
        let error = Error::invalid_input().with_message("Test input validation failed");
        let display_string = format!("{}", error);
        assert!(display_string.contains("InvalidInput"));
        assert!(display_string.contains("Test input validation failed"));
    }
}
