//! Error types for data operations.

/// Boxed error type for dynamic error handling.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Result type for data operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Error type for data operations.
#[derive(Debug, thiserror::Error)]
#[error("{kind}: {message}")]
pub struct Error {
    kind: ErrorKind,
    message: String,
    #[source]
    source: Option<BoxError>,
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

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connection => write!(f, "connection"),
            Self::NotFound => write!(f, "not found"),
            Self::InvalidInput => write!(f, "invalid input"),
            Self::Provider => write!(f, "provider"),
        }
    }
}

impl From<Error> for nvisy_core::Error {
    fn from(err: Error) -> Self {
        let kind = match err.kind {
            ErrorKind::Connection => nvisy_core::ErrorKind::NetworkError,
            ErrorKind::NotFound => nvisy_core::ErrorKind::NotFound,
            ErrorKind::InvalidInput => nvisy_core::ErrorKind::InvalidInput,
            ErrorKind::Provider => nvisy_core::ErrorKind::ExternalError,
        };

        nvisy_core::Error::new(kind)
            .with_message(&err.message)
            .with_source(err)
    }
}
