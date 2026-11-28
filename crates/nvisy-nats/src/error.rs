//! Error types for NATS operations.

use std::time::Duration;

/// Result type for all NATS operations in this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// An error from a NATS operation.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The server could not be reached, or the connection failed.
    #[error("NATS connection failed: {0}")]
    Connection(#[from] async_nats::Error),
    /// A connect or ping did not finish in time.
    #[error("NATS {operation} timed out after {timeout:?}")]
    Timeout {
        /// The operation that timed out.
        operation: &'static str,
        /// How long it was given.
        timeout: Duration,
    },
    /// A payload could not be serialized or deserialized.
    #[error("NATS payload serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    /// A publish, KV, stream, consumer, or acknowledgement operation failed.
    #[error("NATS {operation} failed: {details}")]
    Operation {
        /// The operation that failed.
        operation: &'static str,
        /// What went wrong.
        details: String,
    },
}

impl Error {
    /// Builds an [`Error::Operation`] for `operation`.
    pub(crate) fn operation(operation: &'static str, details: impl Into<String>) -> Self {
        Self::Operation {
            operation,
            details: details.into(),
        }
    }
}
