//! Error types for reqwest-based webhook delivery.

use thiserror::Error;

use crate::ErrorKind;

/// Error type for reqwest operations.
#[derive(Debug, Error)]
pub(crate) enum Error {
    /// HTTP request or middleware error.
    #[error("HTTP error: {0}")]
    Middleware(#[from] reqwest_middleware::Error),
    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl From<Error> for crate::Error {
    fn from(err: Error) -> Self {
        match err {
            Error::Middleware(e) => {
                if e.is_timeout() {
                    crate::Error::new(ErrorKind::Timeout)
                        .with_message(e.to_string())
                        .with_source(e)
                } else if e.is_connect() {
                    crate::Error::new(ErrorKind::DeliveryFailed)
                        .with_message("Connection failed")
                        .with_source(e)
                } else {
                    crate::Error::new(ErrorKind::DeliveryFailed)
                        .with_message(e.to_string())
                        .with_source(e)
                }
            }
            Error::Serde(e) => crate::Error::new(ErrorKind::Serialization)
                .with_message(e.to_string())
                .with_source(e),
        }
    }
}
