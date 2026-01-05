//! Internal error types for nvisy-reqwest.

use thiserror::Error;

/// Result type alias for nvisy-reqwest operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Internal error type for nvisy-reqwest operations.
#[derive(Debug, Error)]
pub enum Error {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Reqwest(#[from] reqwest::Error),
    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl From<Error> for nvisy_webhook::Error {
    fn from(err: Error) -> Self {
        match err {
            Error::Reqwest(e) => {
                if e.is_timeout() {
                    nvisy_webhook::Error::timeout()
                        .with_message(e.to_string())
                        .with_source(e)
                } else if e.is_connect() {
                    nvisy_webhook::Error::network_error()
                        .with_message("Connection failed")
                        .with_source(e)
                } else {
                    nvisy_webhook::Error::network_error()
                        .with_message(e.to_string())
                        .with_source(e)
                }
            }
            Error::Serde(e) => nvisy_webhook::Error::serialization()
                .with_message(e.to_string())
                .with_source(e),
        }
    }
}
