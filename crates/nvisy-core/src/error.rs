//! Common error type definitions.

use strum::{AsRefStr, IntoStaticStr};
use thiserror::Error;

/// Type alias for boxed dynamic errors that can be sent across threads.
///
/// This type is commonly used as a source error in structured error types,
/// providing a way to wrap any error that implements the standard `Error` trait
/// while maintaining Send and Sync bounds for multi-threaded contexts.
pub type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// Type alias for Results with our custom Error type.
pub type Result<T> = std::result::Result<T, Error>;

/// Categories of errors that can occur in nvisy-core operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, AsRefStr, IntoStaticStr)]
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
    Unknown,
}

/// A structured error type for nvisy-core operations.
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
    pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
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

    /// Creates a new authentication error.
    pub fn authentication() -> Self {
        Self::new(ErrorKind::Authentication)
    }

    /// Creates a new authorization error.
    pub fn authorization() -> Self {
        Self::new(ErrorKind::Authorization)
    }

    /// Creates a new rate limited error.
    pub fn rate_limited() -> Self {
        Self::new(ErrorKind::RateLimited)
    }

    /// Creates a new service unavailable error.
    pub fn service_unavailable() -> Self {
        Self::new(ErrorKind::ServiceUnavailable)
    }

    /// Creates a new internal error.
    pub fn internal_error() -> Self {
        Self::new(ErrorKind::InternalError)
    }

    /// Creates a new external error.
    pub fn external_error() -> Self {
        Self::new(ErrorKind::ExternalError)
    }

    /// Creates a new configuration error.
    pub fn configuration() -> Self {
        Self::new(ErrorKind::Configuration)
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

    /// Creates a new unknown error.
    pub fn unknown() -> Self {
        Self::new(ErrorKind::Unknown)
    }

    /// Returns the error kind.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns the error kind as a string.
    pub fn kind_str(&self) -> &'static str {
        self.kind.into()
    }
}
