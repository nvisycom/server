//! XChaCha20-Poly1305 encryption and decryption.
//!
//! This module provides authenticated encryption using XChaCha20-Poly1305,
//! which combines the ChaCha20 stream cipher with the Poly1305 MAC for
//! authenticated encryption with associated data (AEAD).
//!
//! # Wire Format
//!
//! The ciphertext format is: `nonce (24 bytes) || ciphertext || tag (16 bytes)`
//!
//! - **Nonce**: 24-byte random value, safe to generate randomly without collision risk
//! - **Ciphertext**: Same length as plaintext
//! - **Tag**: 16-byte authentication tag appended by the cipher

use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};

use super::error::{CryptoError, CryptoResult};
use super::key::EncryptionKey;

/// Size of the XChaCha20-Poly1305 nonce in bytes.
pub const NONCE_SIZE: usize = 24;

/// Size of the Poly1305 authentication tag in bytes.
pub const TAG_SIZE: usize = 16;

/// Minimum size of valid ciphertext (nonce + tag, no plaintext).
pub const MIN_CIPHERTEXT_SIZE: usize = NONCE_SIZE + TAG_SIZE;

/// Encrypts plaintext using XChaCha20-Poly1305.
///
/// Returns the ciphertext with the nonce prepended. The format is:
/// `nonce (24 bytes) || ciphertext || tag (16 bytes)`
///
/// # Arguments
///
/// * `key` - The 256-bit encryption key
/// * `plaintext` - The data to encrypt
///
/// # Returns
///
/// The encrypted ciphertext with nonce prepended.
///
/// # Example
///
/// ```rust,ignore
/// use nvisy_core::crypto::{encrypt, decrypt, EncryptionKey};
///
/// let key = EncryptionKey::generate();
/// let plaintext = b"hello world";
///
/// let ciphertext = encrypt(&key, plaintext)?;
/// let decrypted = decrypt(&key, &ciphertext)?;
///
/// assert_eq!(plaintext, decrypted.as_slice());
/// ```
pub fn encrypt(key: &EncryptionKey, plaintext: &[u8]) -> CryptoResult<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());

    // Generate a random nonce - XChaCha20's 24-byte nonce is safe to generate randomly
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);

    // Encrypt the plaintext
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|_| CryptoError::RandomGenerationFailed)?;

    // Prepend nonce to ciphertext
    let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    result.extend_from_slice(&nonce);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

/// Decrypts ciphertext encrypted with [`encrypt`].
///
/// Expects the ciphertext format: `nonce (24 bytes) || ciphertext || tag (16 bytes)`
///
/// # Arguments
///
/// * `key` - The 256-bit encryption key (must match the key used for encryption)
/// * `ciphertext` - The encrypted data with prepended nonce
///
/// # Returns
///
/// The decrypted plaintext.
///
/// # Errors
///
/// - [`CryptoError::CiphertextTooShort`] if the ciphertext is shorter than nonce + tag
/// - [`CryptoError::DecryptionFailed`] if decryption fails (wrong key, corrupted data, or tampering)
pub fn decrypt(key: &EncryptionKey, ciphertext: &[u8]) -> CryptoResult<Vec<u8>> {
    if ciphertext.len() < MIN_CIPHERTEXT_SIZE {
        return Err(CryptoError::CiphertextTooShort);
    }

    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());

    // Split nonce and ciphertext
    let (nonce_bytes, encrypted) = ciphertext.split_at(NONCE_SIZE);
    let nonce = nonce_bytes.into();

    // Decrypt
    cipher
        .decrypt(nonce, encrypted)
        .map_err(|_| CryptoError::DecryptionFailed)
}

/// Encrypts a serializable value as JSON.
///
/// This is a convenience function that serializes the value to JSON,
/// then encrypts the JSON bytes.
///
/// # Arguments
///
/// * `key` - The encryption key
/// * `value` - Any value that implements `Serialize`
///
/// # Returns
///
/// The encrypted JSON bytes with nonce prepended.
pub fn encrypt_json<T: serde::Serialize>(key: &EncryptionKey, value: &T) -> CryptoResult<Vec<u8>> {
    let json = serde_json::to_vec(value).map_err(|e| CryptoError::Json(e.to_string()))?;
    encrypt(key, &json)
}

/// Decrypts and deserializes a JSON value.
///
/// This is a convenience function that decrypts the ciphertext,
/// then deserializes the JSON bytes into the target type.
///
/// # Arguments
///
/// * `key` - The encryption key
/// * `ciphertext` - The encrypted JSON data
///
/// # Returns
///
/// The deserialized value.
pub fn decrypt_json<T: serde::de::DeserializeOwned>(
    key: &EncryptionKey,
    ciphertext: &[u8],
) -> CryptoResult<T> {
    let plaintext = decrypt(key, ciphertext)?;
    serde_json::from_slice(&plaintext).map_err(|e| CryptoError::Json(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = EncryptionKey::generate();
        let plaintext = b"hello, world!";

        let ciphertext = encrypt(&key, plaintext).unwrap();
        let decrypted = decrypt(&key, &ciphertext).unwrap();

        assert_eq!(plaintext, decrypted.as_slice());
    }

    #[test]
    fn test_encrypt_decrypt_empty() {
        let key = EncryptionKey::generate();
        let plaintext = b"";

        let ciphertext = encrypt(&key, plaintext).unwrap();
        let decrypted = decrypt(&key, &ciphertext).unwrap();

        assert_eq!(plaintext, decrypted.as_slice());
    }

    #[test]
    fn test_encrypt_decrypt_large() {
        let key = EncryptionKey::generate();
        let plaintext = vec![0xABu8; 1024 * 1024]; // 1 MB

        let ciphertext = encrypt(&key, &plaintext).unwrap();
        let decrypted = decrypt(&key, &ciphertext).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_ciphertext_size() {
        let key = EncryptionKey::generate();
        let plaintext = b"test";

        let ciphertext = encrypt(&key, plaintext).unwrap();

        // nonce (24) + plaintext (4) + tag (16) = 44
        assert_eq!(ciphertext.len(), NONCE_SIZE + plaintext.len() + TAG_SIZE);
    }

    #[test]
    fn test_decrypt_wrong_key() {
        let key1 = EncryptionKey::generate();
        let key2 = EncryptionKey::generate();
        let plaintext = b"secret data";

        let ciphertext = encrypt(&key1, plaintext).unwrap();
        let result = decrypt(&key2, &ciphertext);

        assert!(matches!(result, Err(CryptoError::DecryptionFailed)));
    }

    #[test]
    fn test_decrypt_corrupted_data() {
        let key = EncryptionKey::generate();
        let plaintext = b"secret data";

        let mut ciphertext = encrypt(&key, plaintext).unwrap();
        // Corrupt a byte in the middle
        ciphertext[NONCE_SIZE + 2] ^= 0xFF;

        let result = decrypt(&key, &ciphertext);
        assert!(matches!(result, Err(CryptoError::DecryptionFailed)));
    }

    #[test]
    fn test_decrypt_truncated_data() {
        let key = EncryptionKey::generate();

        // Too short to contain nonce + tag
        let short = vec![0u8; MIN_CIPHERTEXT_SIZE - 1];
        let result = decrypt(&key, &short);

        assert!(matches!(result, Err(CryptoError::CiphertextTooShort)));
    }

    #[test]
    fn test_encrypt_decrypt_json() {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct Credentials {
            api_key: String,
            secret: String,
        }

        let key = EncryptionKey::generate();
        let creds = Credentials {
            api_key: "sk-test-123".to_string(),
            secret: "super-secret".to_string(),
        };

        let ciphertext = encrypt_json(&key, &creds).unwrap();
        let decrypted: Credentials = decrypt_json(&key, &ciphertext).unwrap();

        assert_eq!(creds, decrypted);
    }

    #[test]
    fn test_different_plaintexts_different_ciphertexts() {
        let key = EncryptionKey::generate();
        let plaintext = b"same data";

        let ciphertext1 = encrypt(&key, plaintext).unwrap();
        let ciphertext2 = encrypt(&key, plaintext).unwrap();

        // Different nonces should produce different ciphertexts
        assert_ne!(ciphertext1, ciphertext2);

        // But both should decrypt to the same plaintext
        assert_eq!(decrypt(&key, &ciphertext1).unwrap(), plaintext);
        assert_eq!(decrypt(&key, &ciphertext2).unwrap(), plaintext);
    }
}
