//! Error types for nvisy-rig.

use std::fmt;

use rig::completion::{CompletionError, PromptError};
use rig::embeddings::EmbeddingError;

/// Result type alias for rig operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during rig operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Provider error (API call failed, rate limited, etc.)
    #[error("provider error: {provider}: {message}")]
    Provider { provider: String, message: String },

    /// RAG retrieval error.
    #[error("retrieval error: {0}")]
    Retrieval(String),

    /// Embedding error.
    #[error("embedding error: {0}")]
    Embedding(#[from] EmbeddingError),

    /// Completion error.
    #[error("completion error: {0}")]
    Completion(#[from] CompletionError),

    /// Prompt error.
    #[error("prompt error: {0}")]
    Prompt(#[from] PromptError),

    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),

    /// Parse error (JSON parsing, etc.)
    #[error("parse error: {0}")]
    Parse(String),
}

impl Error {
    /// Creates a provider error.
    pub fn provider(provider: impl fmt::Display, message: impl fmt::Display) -> Self {
        Self::Provider {
            provider: provider.to_string(),
            message: message.to_string(),
        }
    }

    /// Creates a retrieval error.
    pub fn retrieval(message: impl fmt::Display) -> Self {
        Self::Retrieval(message.to_string())
    }

    /// Creates a configuration error.
    pub fn config(message: impl fmt::Display) -> Self {
        Self::Config(message.to_string())
    }

    /// Creates a parse error.
    pub fn parse(message: impl fmt::Display) -> Self {
        Self::Parse(message.to_string())
    }

    /// Returns true if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Provider { .. })
    }
}

impl From<Error> for nvisy_core::Error {
    fn from(err: Error) -> Self {
        let (kind, message) = match &err {
            Error::Provider { provider, message } => (
                nvisy_core::ErrorKind::ExternalError,
                format!("{}: {}", provider, message),
            ),
            Error::Retrieval(msg) => (nvisy_core::ErrorKind::ExternalError, msg.clone()),
            Error::Embedding(_) => (nvisy_core::ErrorKind::ExternalError, err.to_string()),
            Error::Completion(_) => (nvisy_core::ErrorKind::ExternalError, err.to_string()),
            Error::Prompt(_) => (nvisy_core::ErrorKind::InvalidInput, err.to_string()),
            Error::Config(msg) => (nvisy_core::ErrorKind::Configuration, msg.clone()),
            Error::Parse(msg) => (nvisy_core::ErrorKind::Serialization, msg.clone()),
        };

        nvisy_core::Error::new(kind)
            .with_message(message)
            .with_source(err)
    }
}
