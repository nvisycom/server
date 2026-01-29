//! Convenient re-exports for common use.

pub use crate::common::{Provider, ServiceHealth, ServiceStatus, Timing};
#[cfg(feature = "encryption")]
pub use crate::crypto::{
    CryptoError, CryptoResult, EncryptionKey, decrypt, decrypt_json, encrypt, encrypt_json,
};
pub use crate::error::{BoxedError, Error, ErrorKind, Result};
