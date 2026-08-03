//! Error type for object-store operations.

use std::fmt;
use std::time::Duration;

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// Classification of an object-store failure, mapped from the underlying
/// [`object_store::Error`] so callers can branch without matching on messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The object or bucket/container does not exist.
    NotFound,
    /// The object already exists (create-mode conflict).
    AlreadyExists,
    /// A precondition (e.g. if-match) was not met.
    Precondition,
    /// The object was not modified (conditional get).
    NotModified,
    /// Access was denied by the store's authorization.
    PermissionDenied,
    /// The credentials were rejected or missing.
    Unauthenticated,
    /// The operation is not supported by this backend.
    NotSupported,
    /// Failure while establishing or configuring the connection.
    Connection,
    /// Any other runtime failure (network, generic backend error).
    Runtime,
}

impl ErrorKind {
    /// Whether an operation failing with this kind is worth retrying.
    ///
    /// Only transient failures ([`Runtime`](Self::Runtime),
    /// [`Connection`](Self::Connection)) are retryable; a `NotFound`,
    /// `PermissionDenied`, or precondition failure will not change on retry.
    #[must_use]
    pub fn is_retryable(self) -> bool {
        matches!(self, Self::Runtime | Self::Connection)
    }
}

/// An error carrying a classified [`ErrorKind`], a message, and an optional
/// source.
pub struct Error {
    kind: ErrorKind,
    message: String,
    source: Option<BoxedError>,
}

impl Error {
    /// Create an error of the given [`ErrorKind`], formatted as `[{label}] {msg}`.
    pub fn new(kind: ErrorKind, msg: impl fmt::Display, label: &str) -> Self {
        Self {
            kind,
            message: format!("[{label}] {msg}"),
            source: None,
        }
    }

    /// Create a [`Connection`](ErrorKind::Connection) error.
    pub fn connection(msg: impl fmt::Display, label: &str) -> Self {
        Self::new(ErrorKind::Connection, msg, label)
    }

    /// Create a [`Runtime`](ErrorKind::Runtime) error.
    pub fn runtime(msg: impl fmt::Display, label: &str) -> Self {
        Self::new(ErrorKind::Runtime, msg, label)
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

    /// Suggested delay before retrying, or `None` when the error is not
    /// retryable.
    #[must_use]
    pub fn retry_delay(&self) -> Option<Duration> {
        self.kind.is_retryable().then(|| Duration::from_millis(250))
    }
}

impl From<object_store::Error> for Error {
    fn from(err: object_store::Error) -> Self {
        use object_store::Error as O;
        let kind = match &err {
            O::NotFound { .. } => ErrorKind::NotFound,
            O::AlreadyExists { .. } => ErrorKind::AlreadyExists,
            O::Precondition { .. } => ErrorKind::Precondition,
            O::NotModified { .. } => ErrorKind::NotModified,
            O::PermissionDenied { .. } => ErrorKind::PermissionDenied,
            O::Unauthenticated { .. } => ErrorKind::Unauthenticated,
            O::NotSupported { .. } | O::NotImplemented { .. } => ErrorKind::NotSupported,
            _ => ErrorKind::Runtime,
        };
        Self::new(kind, err.to_string(), "object-store").with_source(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("kind", &self.kind)
            .field("message", &self.message)
            .field("source", &self.source)
            .finish()
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|e| e as &(dyn std::error::Error + 'static))
    }
}
