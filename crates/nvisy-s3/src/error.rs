//! Error types for the blob store.

/// Result alias for blob-store operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// An error from the S3-compatible blob store.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The client could not be built from the supplied configuration.
    #[error("failed to configure the blob store: {0}")]
    Config(String),
    /// A string did not parse as an object key.
    #[error("invalid object key: {0}")]
    InvalidKey(String),
    /// A storage operation (put, get, read, delete, ping) failed.
    #[error("blob store {operation} failed: {source}")]
    Operation {
        /// The operation that failed.
        operation: &'static str,
        /// The classified underlying error.
        #[source]
        source: nvisy_core::Error,
    },
}

impl Error {
    /// Builds an [`Error::Operation`] for `operation`.
    pub(crate) fn operation(operation: &'static str, source: nvisy_core::Error) -> Self {
        Self::Operation { operation, source }
    }
}
