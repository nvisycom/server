//! Worker error types.

use std::borrow::Cow;

/// Result type alias for worker operations.
pub type Result<T, E = WorkerError> = std::result::Result<T, E>;

/// Worker error type.
#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    /// Failed to subscribe to job stream.
    #[error("subscription failed: {0}")]
    Subscription(#[from] nvisy_nats::Error),

    /// Failed to process a job.
    #[error("job processing failed: {message}")]
    Processing {
        message: Cow<'static, str>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Database operation failed.
    #[error("database error: {0}")]
    Database(#[from] nvisy_postgres::PgError),
}

impl WorkerError {
    /// Creates a processing error with a message.
    pub fn processing(message: impl Into<Cow<'static, str>>) -> Self {
        Self::Processing {
            message: message.into(),
            source: None,
        }
    }

    /// Creates a processing error with a message and source.
    pub fn processing_with_source(
        message: impl Into<Cow<'static, str>>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Processing {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
}
