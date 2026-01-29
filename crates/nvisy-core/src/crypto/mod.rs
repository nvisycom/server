//! Cryptographic utilities for secure data handling.
//!
//! This module provides encryption and decryption utilities using XChaCha20-Poly1305,
//! a modern AEAD cipher suitable for encrypting sensitive data at rest.
//!
//! # Features
//!
//! This module requires the `encryption` feature to be enabled.
//!
//! # Example
//!
//! ```rust,ignore
//! use nvisy_core::crypto::{EncryptionKey, encrypt, decrypt};
//!
//! // Generate a new random key
//! let key = EncryptionKey::generate();
//!
//! // Encrypt some data
//! let plaintext = b"sensitive credentials";
//! let ciphertext = encrypt(&key, plaintext)?;
//!
//! // Decrypt the data
//! let decrypted = decrypt(&key, &ciphertext)?;
//! assert_eq!(plaintext, decrypted.as_slice());
//! ```

mod cipher;
mod error;
mod key;

pub use cipher::{decrypt, decrypt_json, encrypt, encrypt_json};
pub use error::{CryptoError, CryptoResult};
pub use key::EncryptionKey;
