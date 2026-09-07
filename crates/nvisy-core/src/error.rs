//! Shared error and result types.
//!
//! A general-purpose [`Error`] carrying a classified [`ErrorKind`], a message,
//! and an optional source, with the builder helpers (`with_message`,
//! `with_source`) and retry classification (`is_retryable`, `retry_delay`) the
//! rest of the platform's error types follow. Crates with provider-specific
//! failure modes define their own richer error; this is the foundation shape for
//! shared code that has no such specialization.

use std::time::Duration;

/// A result whose error defaults to the shared [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A boxed, thread-safe source error.
pub type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// Default delay suggested before retrying a transient failure.
const DEFAULT_RETRY_DELAY: Duration = Duration::from_millis(250);

/// Classification of a failure, so callers can branch without matching on
/// messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The input or configuration was invalid.
    Invalid,
    /// The requested resource does not exist.
    NotFound,
    /// The credentials were rejected, missing, or expired.
    Unauthenticated,
    /// The operation was not permitted.
    PermissionDenied,
    /// The operation timed out.
    Timeout,
    /// Failure while establishing or configuring a connection.
    Connection,
    /// Any other runtime failure.
    Runtime,
}

impl ErrorKind {
    /// Whether an operation failing with this kind is worth retrying. Only
    /// transient failures are retryable.
    #[must_use]
    pub fn is_retryable(self) -> bool {
        matches!(self, Self::Timeout | Self::Connection | Self::Runtime)
    }
}

/// An error carrying a classified [`ErrorKind`], a message, and an optional
/// source.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    kind: ErrorKind,
    message: String,
    #[source]
    source: Option<BoxedError>,
}

impl Error {
    /// Create an error of the given [`ErrorKind`] with an empty message.
    #[must_use]
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            message: String::new(),
            source: None,
        }
    }

    /// Create an [`Invalid`](ErrorKind::Invalid) error with a message.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Invalid).with_message(message)
    }

    /// Create a [`Connection`](ErrorKind::Connection) error with a message.
    pub fn connection(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Connection).with_message(message)
    }

    /// Create a [`Runtime`](ErrorKind::Runtime) error with a message.
    pub fn runtime(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Runtime).with_message(message)
    }

    /// Set the human-readable message.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Attach a source error.
    #[must_use]
    pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    /// The classified kind of this error.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Whether the caller should retry the operation.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        self.kind.is_retryable()
    }

    /// Suggested delay before retrying, or `None` when not retryable.
    #[must_use]
    pub fn retry_delay(&self) -> Option<Duration> {
        self.kind.is_retryable().then_some(DEFAULT_RETRY_DELAY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_sets_kind_message_and_source() {
        let source = std::io::Error::other("boom");
        let err = Error::connection("failed to connect").with_source(source);
        assert_eq!(err.kind(), ErrorKind::Connection);
        assert_eq!(err.message(), "failed to connect");
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn retryability_follows_kind() {
        assert!(Error::runtime("x").is_retryable());
        assert!(Error::connection("x").is_retryable());
        assert!(!Error::invalid("x").is_retryable());
        assert_eq!(Error::runtime("x").retry_delay(), Some(DEFAULT_RETRY_DELAY));
        assert_eq!(Error::invalid("x").retry_delay(), None);
    }
}
