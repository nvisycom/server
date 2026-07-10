//! Encryption service backed by a file-loaded master key.
//!
//! [`CryptoService`] loads the master key once at startup and derives a
//! per-workspace key (HKDF-SHA256) for each operation, so the master key is
//! never used directly for workspace data. Master-scoped variants are also
//! available for data not tied to a workspace.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::AsyncRead;
use uuid::Uuid;

use super::{
    CryptoResult, EncryptionKey, decrypt, decrypt_json, decrypt_reader, encrypt, encrypt_json,
    encrypt_reader, generate_secret,
};
use crate::{Error, Result};

/// Tracing target for crypto service operations.
const TRACING_TARGET: &str = "nvisy_server::crypto";

/// Master encryption key file path configuration.
#[derive(Debug, Clone)]
pub struct CryptoConfig {
    /// File path to the 32-byte master encryption key.
    pub key_path: PathBuf,
}

impl Default for CryptoConfig {
    fn default() -> Self {
        Self {
            key_path: "./encryption.key".into(),
        }
    }
}

/// Workspace-aware encryption service.
///
/// Holds the master key and derives per-workspace keys on demand. Cheap to
/// clone (the key is shared through an `Arc`).
#[derive(Clone)]
pub struct CryptoService {
    master_key: Arc<EncryptionKey>,
}

impl CryptoService {
    /// Loads the master key from the path specified in `config`.
    ///
    /// The file must contain exactly 32 raw bytes (256-bit key).
    pub async fn from_config(config: &CryptoConfig) -> Result<Self> {
        Self::load(&config.key_path).await
    }

    /// Loads the master key from a file path.
    ///
    /// The file must contain exactly 32 raw bytes (256-bit key).
    pub async fn from_key_file(key_path: impl AsRef<Path>) -> Result<Self> {
        Self::load(key_path.as_ref()).await
    }

    /// Encrypts a serializable value under the given workspace's key.
    pub fn encrypt_json<T: Serialize>(
        &self,
        workspace_id: Uuid,
        value: &T,
    ) -> CryptoResult<Vec<u8>> {
        encrypt_json(&self.workspace_key(workspace_id), value)
    }

    /// Decrypts a value previously encrypted under the given workspace's key.
    pub fn decrypt_json<T: DeserializeOwned>(
        &self,
        workspace_id: Uuid,
        ciphertext: &[u8],
    ) -> CryptoResult<T> {
        decrypt_json(&self.workspace_key(workspace_id), ciphertext)
    }

    /// Encrypts an in-memory buffer under the given workspace's key.
    ///
    /// Produces the same chunked, authenticated framing that
    /// [`encrypt_reader`](Self::encrypt_reader) streams, so a buffer sealed here
    /// can be opened by either [`decrypt`](Self::decrypt) or `decrypt_reader`.
    /// Use `encrypt_reader` when the data is file-sized and shouldn't be
    /// buffered whole.
    pub fn encrypt(&self, workspace_id: Uuid, plaintext: &[u8]) -> CryptoResult<Vec<u8>> {
        encrypt(&self.workspace_key(workspace_id), plaintext)
    }

    /// Decrypts a buffer previously encrypted under the given workspace's key.
    pub fn decrypt(&self, workspace_id: Uuid, ciphertext: &[u8]) -> CryptoResult<Vec<u8>> {
        decrypt(&self.workspace_key(workspace_id), ciphertext)
    }

    /// Wraps a plaintext reader so it streams out workspace-encrypted bytes,
    /// for piping large uploads to storage without buffering.
    pub fn encrypt_reader<R>(&self, workspace_id: Uuid, reader: R) -> impl AsyncRead + use<R>
    where
        R: AsyncRead + Unpin + Send,
    {
        encrypt_reader(self.workspace_key(workspace_id), reader)
    }

    /// Wraps a ciphertext reader so it yields decrypted plaintext, for streaming
    /// large downloads from storage without buffering.
    pub fn decrypt_reader<R>(&self, workspace_id: Uuid, reader: R) -> impl AsyncRead + use<R>
    where
        R: AsyncRead + Unpin + Send,
    {
        decrypt_reader(self.workspace_key(workspace_id), reader)
    }

    /// Generates a fresh random secret token, hex-encoded.
    ///
    /// For shared secrets that must be recoverable in full (e.g. HMAC signing
    /// keys) — hand the plaintext to the caller once, then store it encrypted.
    pub fn generate_secret(&self) -> String {
        generate_secret()
    }

    /// Derives the per-workspace key via HKDF-SHA256.
    #[inline]
    fn workspace_key(&self, workspace_id: Uuid) -> EncryptionKey {
        self.master_key.derive_workspace_key(workspace_id)
    }

    /// Reads and parses the 32-byte master key from disk.
    async fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(Error::config("Encryption key file does not exist"));
        }
        if !path.is_file() {
            return Err(Error::config("Encryption key path is not a file"));
        }

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

        tracing::info!(target: TRACING_TARGET, "Master encryption key loaded");

        Ok(Self {
            master_key: Arc::new(key),
        })
    }
}

impl fmt::Debug for CryptoService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CryptoService")
            .field("master_key", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde::Deserialize;
    use tempfile::TempDir;

    use super::*;

    async fn service_with_key(raw: [u8; 32]) -> CryptoService {
        let temp_dir = TempDir::new().unwrap();
        let key_path = temp_dir.path().join("encryption.key");
        fs::write(&key_path, raw).unwrap();
        // Keep the temp dir alive for the duration of the load.
        let service = CryptoService::from_key_file(&key_path).await.unwrap();
        drop(temp_dir);
        service
    }

    #[tokio::test]
    async fn reject_invalid_key_length() {
        let temp_dir = TempDir::new().unwrap();
        let key_path = temp_dir.path().join("encryption.key");
        fs::write(&key_path, [0u8; 16]).unwrap();
        assert!(CryptoService::from_key_file(&key_path).await.is_err());
    }

    #[tokio::test]
    async fn reject_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let key_path = temp_dir.path().join("nonexistent.key");
        assert!(CryptoService::from_key_file(&key_path).await.is_err());
    }

    #[tokio::test]
    async fn workspace_roundtrip() {
        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct Secret {
            token: String,
        }

        let crypto = service_with_key([0x42; 32]).await;
        let workspace_id = Uuid::new_v4();
        let secret = Secret {
            token: "s3cr3t".to_owned(),
        };

        let ciphertext = crypto.encrypt_json(workspace_id, &secret).unwrap();
        let decrypted: Secret = crypto.decrypt_json(workspace_id, &ciphertext).unwrap();
        assert_eq!(decrypted, secret);
    }

    #[tokio::test]
    async fn other_workspace_cannot_decrypt() {
        let crypto = service_with_key([0x42; 32]).await;
        let ciphertext = crypto.encrypt(Uuid::new_v4(), b"data").unwrap();
        let result = crypto.decrypt(Uuid::new_v4(), &ciphertext);
        assert!(result.is_err());
    }
}
