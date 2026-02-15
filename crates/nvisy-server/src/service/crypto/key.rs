//! Encryption key management.

use std::fmt;

use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use uuid::Uuid;

use super::error::{CryptoError, CryptoResult};

/// The size of an XChaCha20-Poly1305 key in bytes.
pub const KEY_SIZE: usize = 32;

/// Domain separation string for workspace key derivation.
const WORKSPACE_KEY_INFO: &[u8] = b"nvisy-workspace-encryption-key-v1";

/// A 256-bit encryption key for XChaCha20-Poly1305.
///
/// This type wraps the raw key bytes and provides safe construction methods.
/// The key is stored in memory and should be handled carefully to avoid leaks.
#[derive(Clone)]
pub struct EncryptionKey {
    bytes: [u8; KEY_SIZE],
}

impl EncryptionKey {
    /// Creates a new encryption key from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns [`CryptoError::InvalidKeyLength`] if the slice is not exactly 32 bytes.
    pub fn from_bytes(bytes: &[u8]) -> CryptoResult<Self> {
        let bytes: [u8; KEY_SIZE] = bytes
            .try_into()
            .map_err(|_| CryptoError::InvalidKeyLength)?;
        Ok(Self { bytes })
    }

    /// Generates a new random encryption key using a cryptographically secure RNG.
    #[must_use]
    pub fn generate() -> Self {
        let mut bytes = [0u8; KEY_SIZE];
        rand::rng().fill_bytes(&mut bytes);
        Self { bytes }
    }

    /// Returns the raw key bytes.
    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; KEY_SIZE] {
        &self.bytes
    }

    /// Consumes the key and returns the raw bytes.
    #[inline]
    #[must_use]
    pub fn into_bytes(self) -> [u8; KEY_SIZE] {
        self.bytes
    }

    /// Derives a workspace-specific encryption key using HKDF-SHA256.
    ///
    /// This creates a unique encryption key for each workspace by combining
    /// the master key with the workspace ID.
    #[must_use]
    pub fn derive_workspace_key(&self, workspace_id: Uuid) -> Self {
        let hkdf = Hkdf::<Sha256>::new(Some(workspace_id.as_bytes()), &self.bytes);

        let mut derived_key = [0u8; KEY_SIZE];
        hkdf.expand(WORKSPACE_KEY_INFO, &mut derived_key)
            .expect("HKDF expand should not fail for 32-byte output");

        Self { bytes: derived_key }
    }
}

impl fmt::Debug for EncryptionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EncryptionKey")
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}

impl TryFrom<&[u8]> for EncryptionKey {
    type Error = CryptoError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::from_bytes(bytes)
    }
}

impl TryFrom<Vec<u8>> for EncryptionKey {
    type Error = CryptoError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        Self::from_bytes(&bytes)
    }
}

impl AsRef<[u8]> for EncryptionKey {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_key() {
        let key1 = EncryptionKey::generate();
        let key2 = EncryptionKey::generate();
        assert_ne!(key1.as_bytes(), key2.as_bytes());
    }

    #[test]
    fn test_from_bytes_valid() {
        let bytes = [0u8; KEY_SIZE];
        let key = EncryptionKey::from_bytes(&bytes).unwrap();
        assert_eq!(key.as_bytes(), &bytes);
    }

    #[test]
    fn test_from_bytes_invalid_length() {
        let short = [0u8; 16];
        assert!(matches!(
            EncryptionKey::from_bytes(&short),
            Err(CryptoError::InvalidKeyLength)
        ));

        let long = [0u8; 64];
        assert!(matches!(
            EncryptionKey::from_bytes(&long),
            Err(CryptoError::InvalidKeyLength)
        ));
    }

    #[test]
    fn test_debug_redacts_key() {
        let key = EncryptionKey::generate();
        let debug = format!("{:?}", key);
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains(&format!("{:?}", key.as_bytes())));
    }

    #[test]
    fn test_derive_workspace_key_deterministic() {
        let master_key = EncryptionKey::generate();
        let workspace_id = Uuid::new_v4();

        let derived1 = master_key.derive_workspace_key(workspace_id);
        let derived2 = master_key.derive_workspace_key(workspace_id);

        assert_eq!(derived1.as_bytes(), derived2.as_bytes());
    }

    #[test]
    fn test_derive_workspace_key_different_workspaces() {
        let master_key = EncryptionKey::generate();
        let workspace1 = Uuid::new_v4();
        let workspace2 = Uuid::new_v4();

        let derived1 = master_key.derive_workspace_key(workspace1);
        let derived2 = master_key.derive_workspace_key(workspace2);

        assert_ne!(derived1.as_bytes(), derived2.as_bytes());
    }

    #[test]
    fn test_derive_workspace_key_different_masters() {
        let master1 = EncryptionKey::generate();
        let master2 = EncryptionKey::generate();
        let workspace_id = Uuid::new_v4();

        let derived1 = master1.derive_workspace_key(workspace_id);
        let derived2 = master2.derive_workspace_key(workspace_id);

        assert_ne!(derived1.as_bytes(), derived2.as_bytes());
    }

    #[test]
    fn test_derived_key_differs_from_master() {
        let master_key = EncryptionKey::generate();
        let workspace_id = Uuid::new_v4();

        let derived = master_key.derive_workspace_key(workspace_id);
        assert_ne!(derived.as_bytes(), master_key.as_bytes());
    }
}
