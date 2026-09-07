//! Error type for cloud file-service operations.

use std::time::Duration;

/// A result whose error is the crate [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Classification of a file-service failure, so callers can branch without
/// matching on messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The file or folder does not exist.
    NotFound,
    /// Access was denied by the provider's authorization.
    PermissionDenied,
    /// The OAuth credentials were rejected, expired, or could not be refreshed.
    Unauthenticated,
    /// The provider rejected the request as malformed.
    BadRequest,
    /// Failure while establishing or configuring the connection.
    Connection,
    /// Any other runtime failure (network, generic provider error).
    Runtime,
}

impl ErrorKind {
    /// Whether an operation failing with this kind is worth retrying. Only
    /// transient failures are retryable.
    #[must_use]
    pub fn is_retryable(self) -> bool {
        matches!(self, Self::Runtime | Self::Connection)
    }

    /// A safe, user-facing reason that does not expose provider URLs or other
    /// infrastructure detail. Suitable for API responses (e.g. verification).
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Self::NotFound => "The file was not found",
            Self::PermissionDenied => "Access was denied by the provider",
            Self::Unauthenticated => "The credentials were rejected or expired",
            Self::BadRequest => "The provider rejected the request",
            Self::Connection => "Could not connect to the provider",
            Self::Runtime => "The provider returned an error",
        }
    }
}

/// An error carrying a classified [`ErrorKind`], a message, and an optional
/// source.
#[derive(Debug, thiserror::Error)]
#[error("[file-service] {message}")]
pub struct Error {
    kind: ErrorKind,
    message: String,
    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl Error {
    /// Create an error of the given [`ErrorKind`].
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    /// Create a [`Connection`](ErrorKind::Connection) error.
    pub fn connection(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Connection, message)
    }

    /// Create a [`Runtime`](ErrorKind::Runtime) error.
    pub fn runtime(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Runtime, message)
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

    /// Whether the caller should retry this operation.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        self.kind.is_retryable()
    }

    /// Suggested delay before retrying, or `None` when not retryable.
    #[must_use]
    pub fn retry_delay(&self) -> Option<Duration> {
        self.kind.is_retryable().then(|| Duration::from_millis(250))
    }
}

/// Maps an HTTP status into the corresponding [`ErrorKind`] for a failed
/// provider request.
pub(crate) fn kind_for_status(status: u16) -> ErrorKind {
    match status {
        401 => ErrorKind::Unauthenticated,
        403 => ErrorKind::PermissionDenied,
        404 => ErrorKind::NotFound,
        400 | 405..=422 => ErrorKind::BadRequest,
        _ => ErrorKind::Runtime,
    }
}

impl From<reqwest::Error> for Error {
    /// Classifies a reqwest failure: a response status maps to the matching
    /// kind, while a transport failure with no status (DNS, TCP/TLS, timeout) is
    /// a [`Connection`](ErrorKind::Connection) error.
    fn from(err: reqwest::Error) -> Self {
        let kind = match err.status() {
            Some(status) => kind_for_status(status.as_u16()),
            None => ErrorKind::Connection,
        };
        Self::new(kind, "cloud file provider request failed").with_source(err)
    }
}
