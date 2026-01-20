//! Error types for data operations.

use std::fmt;

/// Result type for data operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Error type for data operations.
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    message: String,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

/// The kind of data error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Connection error.
    Connection,
    /// Resource not found.
    NotFound,
    /// Invalid input.
    InvalidInput,
    /// Provider error.
    Provider,
}

impl Error {
    /// Creates a new error.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    /// Adds a source error.
    pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    /// Returns the error kind.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Creates a connection error.
    pub fn connection(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Connection, message)
    }

    /// Creates a not found error.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, message)
    }

    /// Creates an invalid input error.
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidInput, message)
    }

    /// Creates a provider error.
    pub fn provider(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Provider, message)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|e| e.as_ref() as &(dyn std::error::Error + 'static))
    }
}
