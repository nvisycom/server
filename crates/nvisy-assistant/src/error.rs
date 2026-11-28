//! Error types for assistant language-model operations.

/// A result whose error is the crate [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// An error verifying an LLM provider or running a completion against it.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The provider rejected the credentials or was unreachable.
    #[error("provider verification failed: {0}")]
    Verify(String),

    /// A completion request against the provider failed at runtime.
    #[error("assistant request failed: {0}")]
    Prompt(String),
}
