//! Error types for the blob store.

/// Result alias for blob-store operations.
pub type S3Result<T> = Result<T, S3Error>;

/// An error from the S3-compatible blob store.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum S3Error {
    /// The client could not be built from the supplied configuration.
    #[error("failed to configure the blob store: {0}")]
    Config(String),

    /// A storage operation (put, get, delete, head) failed.
    ///
    /// Carries the operation name and the underlying message so a caller can log
    /// what failed without matching on the SDK's error types.
    #[error("blob store {operation} failed: {message}")]
    Operation {
        /// The operation that failed (`put`, `get`, `delete`, `head`).
        operation: &'static str,
        /// The underlying error message.
        message: String,
    },

    /// The stored object's bytes could not be read from the response stream.
    #[error("failed to read object body: {0}")]
    Body(String),
}

impl S3Error {
    /// Builds an [`S3Error::Operation`] for `operation` from any displayable error.
    pub(crate) fn operation(operation: &'static str, error: impl std::fmt::Display) -> Self {
        Self::Operation {
            operation,
            message: error.to_string(),
        }
    }
}
