//! Internal error types for nvisy-reqwest.

use std::fmt;

/// Result type alias for nvisy-reqwest operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Internal error type for nvisy-reqwest operations.
#[derive(Debug)]
pub enum Error {
    /// Invalid configuration.
    Config(String),
    /// HTTP request failed.
    Reqwest(reqwest::Error),
    /// Serialization error.
    Serde(serde_json::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(msg) => write!(f, "Invalid configuration: {msg}"),
            Self::Reqwest(err) => write!(f, "HTTP error: {err}"),
            Self::Serde(err) => write!(f, "Serialization error: {err}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Reqwest(err) => Some(err),
            Self::Serde(err) => Some(err),
            Self::Config(_) => None,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Self::Reqwest(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Serde(err)
    }
}

impl From<Error> for nvisy_service::Error {
    fn from(err: Error) -> Self {
        match err {
            Error::Config(msg) => nvisy_service::Error::configuration().with_message(msg),
            Error::Reqwest(e) => {
                if e.is_timeout() {
                    nvisy_service::Error::timeout()
                        .with_message(e.to_string())
                        .with_source(Box::new(e))
                } else if e.is_connect() {
                    nvisy_service::Error::network_error()
                        .with_message("Connection failed")
                        .with_source(Box::new(e))
                } else {
                    nvisy_service::Error::network_error()
                        .with_message(e.to_string())
                        .with_source(Box::new(e))
                }
            }
            Error::Serde(e) => nvisy_service::Error::serialization()
                .with_message(e.to_string())
                .with_source(Box::new(e)),
        }
    }
}
