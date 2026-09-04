//! Error types for the blob store.

use std::error::Error as StdError;

/// A boxed underlying error, retained so callers can downcast to the SDK's typed
/// service error rather than only reading its rendered message.
type BoxError = Box<dyn StdError + Send + Sync>;

/// Result alias for blob-store operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// An error from the S3-compatible blob store.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The client could not be built from the supplied configuration.
    #[error("failed to configure the blob store: {0}")]
    Config(String),

    /// A storage operation (put, get, delete, head) failed.
    ///
    /// Carries the operation name and the underlying message so a caller can log
    /// what failed without matching on the SDK's error types, and keeps the
    /// underlying error as the [`source`](StdError::source) so a caller that
    /// wants the typed SDK error can downcast to it.
    #[error("blob store {operation} failed: {message}")]
    Operation {
        /// The operation that failed (`put`, `get`, `delete`, `head`).
        operation: &'static str,
        /// The underlying error message.
        message: String,
        /// The underlying error, when the failure wraps one.
        #[source]
        source: Option<BoxError>,
    },

    /// The stored object's bytes could not be read from the response stream.
    #[error("failed to read object body: {0}")]
    Body(String),
}

impl Error {
    /// Builds an [`Error::Operation`] for `operation`, keeping `error` as the
    /// message and its source so a caller can downcast to the typed SDK error.
    pub(crate) fn operation(
        operation: &'static str,
        error: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self::Operation {
            operation,
            message: error.to_string(),
            source: Some(Box::new(error)),
        }
    }

    /// Builds an [`Error::Operation`] for `operation` from a message alone,
    /// for failures that do not wrap an underlying error.
    pub(crate) fn operation_msg(operation: &'static str, message: impl Into<String>) -> Self {
        Self::Operation {
            operation,
            message: message.into(),
            source: None,
        }
    }
}
