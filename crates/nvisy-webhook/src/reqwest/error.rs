//! Error types for reqwest-based webhook delivery.

use thiserror::Error;

/// Result type alias for reqwest operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Error type for reqwest operations.
#[derive(Debug, Error)]
pub enum Error {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Reqwest(#[from] reqwest::Error),
    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl From<Error> for crate::Error {
    fn from(err: Error) -> Self {
        match err {
            Error::Reqwest(e) => {
                if e.is_timeout() {
                    crate::Error::timeout()
                        .with_message(e.to_string())
                        .with_source(e)
                } else if e.is_connect() {
                    crate::Error::network_error()
                        .with_message("Connection failed")
                        .with_source(e)
                } else {
                    crate::Error::network_error()
                        .with_message(e.to_string())
                        .with_source(e)
                }
            }
            Error::Serde(e) => crate::Error::serialization()
                .with_message(e.to_string())
                .with_source(e),
        }
    }
}
