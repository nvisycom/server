//! Cryptographic utilities for secure data handling.
//!
//! This module provides encryption and decryption utilities using XChaCha20-Poly1305,
//! a modern AEAD cipher suitable for encrypting sensitive data at rest.

mod encryption;
mod error;
mod generation;
mod hashing_reader;
mod key;
mod limited_reader;
mod service;

pub(crate) use encryption::{
    decrypt, decrypt_json, decrypt_reader, encrypt, encrypt_json, encrypt_reader,
};
pub use error::{CryptoError, CryptoResult};
pub(crate) use generation::generate_secret;
pub(crate) use hashing_reader::{HashingReader, Measurements};
pub(crate) use key::EncryptionKey;
pub(crate) use limited_reader::LimitedReader;
pub use service::{CryptoConfig, CryptoService};
