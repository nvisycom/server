//! XChaCha20-Poly1305 encryption and decryption.
//!
//! # Wire Format
//!
//! The ciphertext format is: `nonce (24 bytes) || ciphertext || tag (16 bytes)`

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
pub fn encrypt(key: &EncryptionKey, plaintext: &[u8]) -> CryptoResult<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());

    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|_| CryptoError::RandomGenerationFailed)?;

    let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    result.extend_from_slice(&nonce);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

/// Decrypts ciphertext encrypted with [`encrypt`].
///
/// Expects the ciphertext format: `nonce (24 bytes) || ciphertext || tag (16 bytes)`
pub fn decrypt(key: &EncryptionKey, ciphertext: &[u8]) -> CryptoResult<Vec<u8>> {
    if ciphertext.len() < MIN_CIPHERTEXT_SIZE {
        return Err(CryptoError::CiphertextTooShort);
    }

    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());

    let (nonce_bytes, encrypted) = ciphertext.split_at(NONCE_SIZE);
    let nonce = nonce_bytes.into();

    cipher
        .decrypt(nonce, encrypted)
        .map_err(|_| CryptoError::DecryptionFailed)
}

/// Encrypts a serializable value as JSON.
pub fn encrypt_json<T: serde::Serialize>(key: &EncryptionKey, value: &T) -> CryptoResult<Vec<u8>> {
    let json = serde_json::to_vec(value).map_err(|e| CryptoError::Json(e.to_string()))?;
    encrypt(key, &json)
}

/// Decrypts and deserializes a JSON value.
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
        let plaintext = vec![0xABu8; 1024 * 1024];

        let ciphertext = encrypt(&key, &plaintext).unwrap();
        let decrypted = decrypt(&key, &ciphertext).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_ciphertext_size() {
        let key = EncryptionKey::generate();
        let plaintext = b"test";

        let ciphertext = encrypt(&key, plaintext).unwrap();
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
        ciphertext[NONCE_SIZE + 2] ^= 0xFF;

        let result = decrypt(&key, &ciphertext);
        assert!(matches!(result, Err(CryptoError::DecryptionFailed)));
    }

    #[test]
    fn test_decrypt_truncated_data() {
        let key = EncryptionKey::generate();

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

        assert_ne!(ciphertext1, ciphertext2);

        assert_eq!(decrypt(&key, &ciphertext1).unwrap(), plaintext);
        assert_eq!(decrypt(&key, &ciphertext2).unwrap(), plaintext);
    }
}
