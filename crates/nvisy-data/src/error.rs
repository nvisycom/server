//! Error types for data operations.

use std::fmt;

/// Result type for data operations.
pub type DataResult<T> = Result<T, DataError>;

/// Error type for data operations.
#[derive(Debug)]
pub struct DataError {
    kind: DataErrorKind,
    message: String,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

/// The kind of data error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataErrorKind {
    /// Connection error (e.g., network failure).
    Connection,
    /// Not found error (e.g., file or collection doesn't exist).
    NotFound,
    /// Permission denied.
    Permission,
    /// Invalid input or configuration.
    Invalid,
    /// Serialization/deserialization error.
    Serialization,
    /// Backend-specific error.
    Backend,
    /// Unknown or unclassified error.
    Unknown,
}

impl DataError {
    /// Creates a new error with the given kind and message.
    pub fn new(kind: DataErrorKind, message: impl Into<String>) -> Self {
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
    pub fn kind(&self) -> DataErrorKind {
        self.kind
    }

    /// Creates a connection error.
    pub fn connection(message: impl Into<String>) -> Self {
        Self::new(DataErrorKind::Connection, message)
    }

    /// Creates a not found error.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(DataErrorKind::NotFound, message)
    }

    /// Creates a permission error.
    pub fn permission(message: impl Into<String>) -> Self {
        Self::new(DataErrorKind::Permission, message)
    }

    /// Creates an invalid input error.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(DataErrorKind::Invalid, message)
    }

    /// Creates a serialization error.
    pub fn serialization(message: impl Into<String>) -> Self {
        Self::new(DataErrorKind::Serialization, message)
    }

    /// Creates a backend error.
    pub fn backend(message: impl Into<String>) -> Self {
        Self::new(DataErrorKind::Backend, message)
    }
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for DataError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|e| e.as_ref() as &(dyn std::error::Error + 'static))
    }
}
