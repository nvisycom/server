//! Error types for nvisy-ollama.

use ollama_rs::error::OllamaError;
use thiserror::Error;

/// Error type for the nvisy-ollama library.
#[derive(Error, Debug)]
pub enum Error {
    /// Ollama API errors from ollama-rs.
    #[error("Ollama error: {0}")]
    Ollama(#[from] OllamaError),

    /// Configuration errors.
    #[error("Configuration error: {0}")]
    Config(String),
}

impl Error {
    /// Create a configuration error.
    pub fn invalid_config(message: impl Into<String>) -> Self {
        Self::Config(message.into())
    }
}

/// Result type alias for nvisy-ollama operations.
pub type Result<T> = std::result::Result<T, Error>;
