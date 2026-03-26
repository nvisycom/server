//! Error types for reqwest-based webhook delivery.

use thiserror::Error;

/// Result type alias for reqwest operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Error type for reqwest operations.
#[derive(Debug, Error)]
pub enum Error {
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
                    crate::Error::new(crate::ErrorKind::Timeout)
                        .with_message(e.to_string())
                        .with_source(e)
                } else if e.is_connect() {
                    crate::Error::new(crate::ErrorKind::NetworkError)
                        .with_message("Connection failed")
                        .with_source(e)
                } else {
                    crate::Error::new(crate::ErrorKind::NetworkError)
                        .with_message(e.to_string())
                        .with_source(e)
                }
            }
            Error::Serde(e) => crate::Error::new(crate::ErrorKind::Serialization)
                .with_message(e.to_string())
                .with_source(e),
        }
    }
}
