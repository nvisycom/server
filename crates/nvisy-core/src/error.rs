//! Shared error and result types.
//!
//! One [`Error`] for every crate that talks to an external system (object
//! stores, cloud file services, webhook endpoints): a classified [`ErrorKind`],
//! a message, and an optional source. Callers branch on the kind, never on the
//! message.

/// A result whose error defaults to the shared [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A boxed, thread-safe source error.
pub type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// Classification of a failure, so callers can branch without matching on
/// messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The input, configuration, or target was invalid, or the remote rejected
    /// the request as malformed.
    Invalid,
    /// The requested resource does not exist.
    NotFound,
    /// The resource already exists.
    AlreadyExists,
    /// A precondition on the request was not met.
    Precondition,
    /// The credentials were rejected, missing, or expired.
    Unauthenticated,
    /// The operation was not permitted.
    PermissionDenied,
    /// The operation is not supported by this backend.
    NotSupported,
    /// The operation timed out.
    Timeout,
    /// The remote could not be reached.
    Connection,
    /// The remote failed while handling the request.
    Runtime,
    /// A local failure no retry can fix, such as a payload that cannot be
    /// serialized.
    Internal,
}

impl ErrorKind {
    /// Whether an operation failing with this kind is worth retrying. Only
    /// transient failures are.
    #[must_use]
    pub fn is_retryable(self) -> bool {
        matches!(self, Self::Timeout | Self::Connection | Self::Runtime)
    }

    /// A safe, user-facing reason that exposes no URLs, bucket names, or other
    /// infrastructure detail, for API responses such as connection
    /// verification.
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Self::Invalid => "The request was rejected as invalid",
            Self::NotFound => "The resource was not found",
            Self::AlreadyExists => "The resource already exists",
            Self::Precondition => "A precondition was not met",
            Self::Unauthenticated => "The credentials were rejected or expired",
            Self::PermissionDenied => "Access was denied",
            Self::NotSupported => "The operation is not supported",
            Self::Timeout => "The request timed out",
            Self::Connection => "Could not connect",
            Self::Runtime => "The remote service returned an error",
            Self::Internal => "An internal error occurred",
        }
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
    /// Creates an error of the given [`ErrorKind`] with a message.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    /// Creates an [`Invalid`](ErrorKind::Invalid) error.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Invalid, message)
    }

    /// Creates a [`Connection`](ErrorKind::Connection) error.
    pub fn connection(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Connection, message)
    }

    /// Creates a [`Runtime`](ErrorKind::Runtime) error.
    pub fn runtime(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Runtime, message)
    }

    /// Creates an [`Internal`](ErrorKind::Internal) error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, message)
    }

    /// Attaches a source error.
    #[must_use]
    pub fn with_source(mut self, source: impl Into<BoxedError>) -> Self {
        self.source = Some(source.into());
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
}

/// Classifies a failed HTTP response status: auth and lookup statuses map to
/// their kinds, other client errors are [`Invalid`](ErrorKind::Invalid), and
/// everything else (timeouts, throttling, server errors) is
/// [`Runtime`](ErrorKind::Runtime).
#[must_use]
pub fn kind_for_status(status: u16) -> ErrorKind {
    match status {
        401 => ErrorKind::Unauthenticated,
        403 => ErrorKind::PermissionDenied,
        404 => ErrorKind::NotFound,
        409 => ErrorKind::AlreadyExists,
        412 => ErrorKind::Precondition,
        400 | 405..=407 | 410 | 411 | 413..=422 => ErrorKind::Invalid,
        _ => ErrorKind::Runtime,
    }
}

#[cfg(feature = "reqwest")]
impl From<reqwest::Error> for Error {
    /// Classifies a reqwest failure: a response status by
    /// [`kind_for_status`], a timeout as [`Timeout`](ErrorKind::Timeout), a
    /// request that could not be built as [`Invalid`](ErrorKind::Invalid), a
    /// response body that could not be read or decoded as
    /// [`Runtime`](ErrorKind::Runtime), and any other transport failure as
    /// [`Connection`](ErrorKind::Connection).
    fn from(err: reqwest::Error) -> Self {
        let kind = if let Some(status) = err.status() {
            kind_for_status(status.as_u16())
        } else if err.is_timeout() {
            ErrorKind::Timeout
        } else if err.is_builder() {
            ErrorKind::Invalid
        } else if err.is_body() || err.is_decode() {
            ErrorKind::Runtime
        } else {
            ErrorKind::Connection
        };
        Self::new(kind, err.to_string()).with_source(err)
    }
}

#[cfg(feature = "reqwest")]
impl From<reqwest_middleware::Error> for Error {
    /// Classifies a middleware-wrapped failure like the reqwest error inside
    /// it; a failure raised by a middleware itself is
    /// [`Runtime`](ErrorKind::Runtime).
    fn from(err: reqwest_middleware::Error) -> Self {
        match err {
            reqwest_middleware::Error::Reqwest(err) => err.into(),
            reqwest_middleware::Error::Middleware(err) => {
                Self::runtime(err.to_string()).with_source(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_sets_kind_message_and_source() {
        let err = Error::connection("failed to connect").with_source(std::io::Error::other("boom"));
        assert_eq!(err.kind(), ErrorKind::Connection);
        assert_eq!(err.message(), "failed to connect");
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn only_transient_kinds_are_retryable() {
        assert!(Error::runtime("x").is_retryable());
        assert!(Error::connection("x").is_retryable());
        assert!(Error::new(ErrorKind::Timeout, "x").is_retryable());
        assert!(!Error::invalid("x").is_retryable());
        assert!(!Error::internal("x").is_retryable());
    }

    #[test]
    fn statuses_map_to_kinds() {
        assert_eq!(kind_for_status(401), ErrorKind::Unauthenticated);
        assert_eq!(kind_for_status(404), ErrorKind::NotFound);
        assert_eq!(kind_for_status(422), ErrorKind::Invalid);
        assert_eq!(kind_for_status(429), ErrorKind::Runtime);
        assert_eq!(kind_for_status(503), ErrorKind::Runtime);
    }
}
