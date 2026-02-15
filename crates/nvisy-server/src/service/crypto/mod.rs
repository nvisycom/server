//! Cryptographic utilities for secure data handling.
//!
//! This module provides encryption and decryption utilities using XChaCha20-Poly1305,
//! a modern AEAD cipher suitable for encrypting sensitive data at rest.

mod cipher;
mod error;
mod key;

pub use cipher::{decrypt, decrypt_json, encrypt, encrypt_json};
pub use error::{CryptoError, CryptoResult};
pub use key::EncryptionKey;
