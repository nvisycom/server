//! Cryptographic error types.

use thiserror::Error;

/// Result type for cryptographic operations.
pub type CryptoResult<T> = std::result::Result<T, CryptoError>;

/// Errors that can occur during cryptographic operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CryptoError {
    /// The ciphertext is too short to contain a valid nonce and tag.
    #[error("ciphertext too short to contain nonce and authentication tag")]
    CiphertextTooShort,
    /// Decryption failed - data may be corrupted or tampered with.
    #[error("decryption failed: data may be corrupted or tampered with")]
    DecryptionFailed,
    /// The provided key has an invalid length.
    #[error("invalid key length: expected 32 bytes")]
    InvalidKeyLength,
    /// Failed to generate random bytes.
    #[error("failed to generate random bytes")]
    RandomGenerationFailed,
    /// JSON serialization/deserialization failed.
    #[error("json error: {0}")]
    Json(String),
}
