//! Cryptographic utilities for secure data handling.
//!
//! This module provides encryption and decryption utilities using XChaCha20-Poly1305,
//! a modern AEAD cipher suitable for encrypting sensitive data at rest.

mod cipher;
mod error;
mod key;
mod service;
mod stream;

pub(crate) use cipher::{decrypt, decrypt_json, encrypt, encrypt_json};
pub use error::{CryptoError, CryptoResult};
pub(crate) use key::EncryptionKey;
pub use service::{CryptoConfig, CryptoService};
pub(crate) use stream::{decrypt_stream, encrypt_stream};
