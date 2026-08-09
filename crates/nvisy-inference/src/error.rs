//! Error types for LLM inference operations.

use thiserror::Error;

/// An error building or validating an LLM provider client.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// The client could not be constructed from the config (e.g. an invalid
    /// base URL).
    #[error("failed to build the inference client: {0}")]
    Build(String),

    /// The provider rejected the credentials or was unreachable.
    #[error("inference provider verification failed: {0}")]
    Verify(String),

    /// A completion request against the provider failed at runtime.
    #[error("inference request failed: {0}")]
    Prompt(String),
}
