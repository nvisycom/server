//! Master encryption key management for connection data.
//!
//! This module provides functionality for loading and managing the master encryption
//! key used to derive workspace-specific keys for encrypting connection credentials.
//! Workspace keys are derived via HKDF-SHA256 so the master key is never used directly.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(any(test, feature = "config"))]
use clap::Args;
use nvisy_core::crypto::EncryptionKey;
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Tracing target for master key operations.
const TRACING_TARGET: &str = "nvisy_server::master_key";

/// Master encryption key file path configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(any(test, feature = "config"), derive(Args))]
pub struct MasterKeyConfig {
    /// File path to the 32-byte master encryption key.
    #[cfg_attr(
        any(test, feature = "config"),
        arg(
            long,
            env = "ENCRYPTION_KEY_FILEPATH",
            default_value = "./encryption.key"
        )
    )]
    #[serde(default = "MasterKeyConfig::default_key_path")]
    pub key_path: PathBuf,
}

impl MasterKeyConfig {
    fn default_key_path() -> PathBuf {
        "./encryption.key".into()
    }
}

/// Master encryption key used to derive workspace-specific keys.
///
/// This is a thin wrapper around [`EncryptionKey`] that adds file-based loading
/// and tracing. The underlying key is used exclusively to derive per-workspace
/// keys via HKDF-SHA256 — it is never used for encryption directly.
#[derive(Clone)]
pub struct MasterKey {
    inner: Arc<EncryptionKey>,
}

impl MasterKey {
    /// Loads the master key from the path specified in `config`.
    ///
    /// The file must contain exactly 32 raw bytes (256-bit key).
    pub async fn from_config(config: &MasterKeyConfig) -> Result<Self> {
        Self::validate_path(&config.key_path)?;
        Self::load(&config.key_path).await
    }

    /// Loads the master key from a file path.
    pub async fn new(key_path: impl AsRef<Path>) -> Result<Self> {
        let path = key_path.as_ref();
        Self::validate_path(path)?;
        Self::load(path).await
    }

    /// Returns a reference to the underlying [`EncryptionKey`].
    #[inline]
    pub fn encryption_key(&self) -> &EncryptionKey {
        &self.inner
    }

    /// Derives a workspace-specific encryption key via HKDF-SHA256.
    #[inline]
    #[must_use]
    pub fn derive_workspace_key(&self, workspace_id: uuid::Uuid) -> EncryptionKey {
        self.inner.derive_workspace_key(workspace_id)
    }

    /// Validates that the key file exists and is a regular file.
    fn validate_path(path: &Path) -> Result<()> {
        if !path.exists() {
            return Err(Error::config("Encryption key file does not exist"));
        }

        if !path.is_file() {
            return Err(Error::config("Encryption key path is not a file"));
        }

        Ok(())
    }

    /// Reads and parses the 32-byte key from disk.
    async fn load(path: &Path) -> Result<Self> {
        tracing::debug!(
            target: TRACING_TARGET,
            path = %path.display(),
            "Loading master encryption key",
        );

        let bytes = tokio::fs::read(path).await.map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET,
                path = %path.display(),
                error = %e,
                "Failed to read encryption key file",
            );
            Error::file_system("Failed to read encryption key file").with_source(e)
        })?;

        let key = EncryptionKey::from_bytes(&bytes).map_err(|e| {
            tracing::error!(
                target: TRACING_TARGET,
                path = %path.display(),
                error = %e,
                "Invalid encryption key: expected exactly 32 bytes",
            );
            Error::config("Invalid encryption key: expected exactly 32 bytes").with_source(e)
        })?;

        tracing::info!(
            target: TRACING_TARGET,
            "Master encryption key loaded",
        );

        Ok(Self {
            inner: Arc::new(key),
        })
    }
}

impl fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MasterKey")
            .field("key", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[tokio::test]
    async fn load_valid_key() {
        let temp_dir = TempDir::new().unwrap();
        let key_path = temp_dir.path().join("encryption.key");
        fs::write(&key_path, [0xABu8; 32]).unwrap();

        let master_key = MasterKey::new(&key_path).await.unwrap();
        assert_eq!(master_key.encryption_key().as_bytes(), &[0xAB; 32]);
    }

    #[tokio::test]
    async fn reject_invalid_key_length() {
        let temp_dir = TempDir::new().unwrap();
        let key_path = temp_dir.path().join("encryption.key");
        fs::write(&key_path, [0u8; 16]).unwrap();

        assert!(MasterKey::new(&key_path).await.is_err());
    }

    #[tokio::test]
    async fn reject_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let key_path = temp_dir.path().join("nonexistent.key");

        assert!(MasterKey::new(&key_path).await.is_err());
    }

    #[tokio::test]
    async fn derive_workspace_key_delegates() {
        let temp_dir = TempDir::new().unwrap();
        let key_path = temp_dir.path().join("encryption.key");
        let raw = [0x42u8; 32];
        fs::write(&key_path, raw).unwrap();

        let master_key = MasterKey::new(&key_path).await.unwrap();
        let workspace_id = uuid::Uuid::new_v4();

        let derived = master_key.derive_workspace_key(workspace_id);
        let expected = EncryptionKey::from_bytes(&raw)
            .unwrap()
            .derive_workspace_key(workspace_id);

        assert_eq!(derived.as_bytes(), expected.as_bytes());
    }
}
