//! Error types for nvisy-ollama
//!
//! This module provides comprehensive error handling for the Ollama client library.

use thiserror::Error;

/// Main error type for the nvisy-ollama library
#[derive(Error, Debug)]
pub enum Error {
    /// HTTP client errors
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization/deserialization errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// URL parsing errors
    #[error("URL error: {0}")]
    Url(#[from] url::ParseError),

    /// API errors returned by the Ollama service
    #[error("API error (status {status}): {message}")]
    Api { status: u16, message: String },

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Authentication errors
    #[error("Authentication error: {0}")]
    Auth(String),

    /// Connection errors
    #[error("Connection error: {0}")]
    Connection(String),

    /// Timeout errors
    #[error("Timeout error: {0}")]
    Timeout(String),

    /// Model not found errors
    #[error("Model not found: {model}")]
    ModelNotFound { model: String },

    /// Invalid request errors
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Rate limiting errors
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    /// Generic errors
    #[error("Error: {0}")]
    Other(String),
}

impl Error {
    /// Create an API error
    pub fn api_error(status: u16, message: impl Into<String>) -> Self {
        Self::Api {
            status,
            message: message.into(),
        }
    }

    /// Create a configuration error
    pub fn invalid_config(message: impl Into<String>) -> Self {
        Self::Config(message.into())
    }

    /// Create an authentication error
    pub fn auth_error(message: impl Into<String>) -> Self {
        Self::Auth(message.into())
    }

    /// Create a connection error
    pub fn connection_error(message: impl Into<String>) -> Self {
        Self::Connection(message.into())
    }

    /// Create a timeout error
    pub fn timeout_error(message: impl Into<String>) -> Self {
        Self::Timeout(message.into())
    }

    /// Create a model not found error
    pub fn model_not_found(model: impl Into<String>) -> Self {
        Self::ModelNotFound {
            model: model.into(),
        }
    }

    /// Create an invalid request error
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::InvalidRequest(message.into())
    }

    /// Create a rate limit error
    pub fn rate_limit_error(message: impl Into<String>) -> Self {
        Self::RateLimit(message.into())
    }

    /// Create a generic error
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other(message.into())
    }

    /// Check if this is a client error (4xx status codes)
    pub fn is_client_error(&self) -> bool {
        matches!(self, Self::Api { status, .. } if *status >= 400 && *status < 500)
    }

    /// Check if this is a server error (5xx status codes)
    pub fn is_server_error(&self) -> bool {
        matches!(self, Self::Api { status, .. } if *status >= 500)
    }

    /// Check if this is a timeout error
    pub fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout(_)) || matches!(self, Self::Http(e) if e.is_timeout())
    }

    /// Check if this is a connection error
    pub fn is_connection_error(&self) -> bool {
        matches!(self, Self::Connection(_)) || matches!(self, Self::Http(e) if e.is_connect())
    }

    /// Check if this is a rate limit error
    pub fn is_rate_limit(&self) -> bool {
        matches!(self, Self::RateLimit(_)) || matches!(self, Self::Api { status: 429, .. })
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Http(e) => e.is_timeout() || e.is_connect(),
            Self::Timeout(_) | Self::Connection(_) => true,
            Self::Api { status, .. } => {
                // Retry on 5xx errors and 429 (rate limit)
                *status >= 500 || *status == 429
            }
            _ => false,
        }
    }
}

/// Result type alias for nvisy-ollama operations
pub type Result<T> = std::result::Result<T, Error>;

// Import builder error type for From implementation
use crate::client::OllamaBuilderError;

impl From<OllamaBuilderError> for Error {
    fn from(err: OllamaBuilderError) -> Self {
        Error::Config(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_classifications() {
        let client_error = Error::api_error(404, "Not found");
        let server_error = Error::api_error(500, "Internal server error");
        let rate_limit = Error::api_error(429, "Rate limit exceeded");
        let timeout = Error::timeout_error("Connection timed out");

        assert!(client_error.is_client_error());
        assert!(!client_error.is_server_error());
        assert!(!client_error.is_retryable());

        assert!(!server_error.is_client_error());
        assert!(server_error.is_server_error());
        assert!(server_error.is_retryable());

        assert!(rate_limit.is_client_error());
        assert!(rate_limit.is_rate_limit());
        assert!(rate_limit.is_retryable());

        assert!(timeout.is_timeout());
        assert!(timeout.is_retryable());
    }

    #[test]
    fn test_error_constructors() {
        let config_err = Error::invalid_config("Invalid URL");
        assert!(matches!(config_err, Error::Config(_)));

        let model_err = Error::model_not_found("llama2");
        assert!(matches!(model_err, Error::ModelNotFound { .. }));

        let auth_err = Error::auth_error("Invalid API key");
        assert!(matches!(auth_err, Error::Auth(_)));
    }
}
