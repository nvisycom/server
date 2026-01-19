//! Error types for nvisy-rig.

use std::fmt;

/// Result type alias for rig operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during rig operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Provider error (API call failed, rate limited, etc.)
    #[error("provider error: {provider}: {message}")]
    Provider { provider: String, message: String },

    /// Session error (not found, expired, etc.)
    #[error("session error: {0}")]
    Session(String),

    /// Agent execution error.
    #[error("agent error: {0}")]
    Agent(String),

    /// Tool execution error.
    #[error("tool error: {tool}: {message}")]
    Tool { tool: String, message: String },

    /// RAG retrieval error.
    #[error("retrieval error: {0}")]
    Retrieval(String),

    /// Embedding error.
    #[error("embedding error: {0}")]
    Embedding(String),

    /// Edit error.
    #[error("edit error: {0}")]
    Edit(String),

    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    /// Creates a provider error.
    pub fn provider(provider: impl fmt::Display, message: impl fmt::Display) -> Self {
        Self::Provider {
            provider: provider.to_string(),
            message: message.to_string(),
        }
    }

    /// Creates a session error.
    pub fn session(message: impl fmt::Display) -> Self {
        Self::Session(message.to_string())
    }

    /// Creates an agent error.
    pub fn agent(message: impl fmt::Display) -> Self {
        Self::Agent(message.to_string())
    }

    /// Creates a tool error.
    pub fn tool(tool: impl fmt::Display, message: impl fmt::Display) -> Self {
        Self::Tool {
            tool: tool.to_string(),
            message: message.to_string(),
        }
    }

    /// Creates a retrieval error.
    pub fn retrieval(message: impl fmt::Display) -> Self {
        Self::Retrieval(message.to_string())
    }

    /// Creates an embedding error.
    pub fn embedding(message: impl fmt::Display) -> Self {
        Self::Embedding(message.to_string())
    }

    /// Creates an edit error.
    pub fn edit(message: impl fmt::Display) -> Self {
        Self::Edit(message.to_string())
    }

    /// Creates a configuration error.
    pub fn config(message: impl fmt::Display) -> Self {
        Self::Config(message.to_string())
    }

    /// Returns true if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Provider { .. } | Self::Io(_))
    }
}
