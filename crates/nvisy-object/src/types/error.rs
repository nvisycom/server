//! Minimal error type for object-store operations.

use std::fmt;

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// A lightweight error carrying a message, an optional source, and a
/// retryable flag.
pub struct Error {
    message: String,
    source: Option<BoxedError>,
    retryable: bool,
}

impl Error {
    /// Create a runtime error formatted as `[{label}] {msg}`.
    pub fn runtime(msg: impl fmt::Display, label: &str, retryable: bool) -> Self {
        Self {
            message: format!("[{label}] {msg}"),
            source: None,
            retryable,
        }
    }

    /// Create a connection error formatted as `[{label}] {msg}`.
    pub fn connection(msg: impl fmt::Display, label: &str, retryable: bool) -> Self {
        Self {
            message: format!("[{label}] {msg}"),
            source: None,
            retryable,
        }
    }

    /// Attach a source error.
    pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    /// Whether the caller should retry this operation.
    pub fn is_retryable(&self) -> bool {
        self.retryable
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
            .field("message", &self.message)
            .field("retryable", &self.retryable)
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
