//! Storage backend implementation.

use opendal::{Operator, services};

use crate::TRACING_TARGET;
use crate::config::{BackendType, StorageConfig};
use crate::error::{StorageError, StorageResult};

/// Unified storage backend that wraps OpenDAL operators.
#[derive(Clone)]
pub struct StorageBackend {
    operator: Operator,
    config: StorageConfig,
}

impl StorageBackend {
    /// Creates a new storage backend from configuration.
    pub async fn new(config: StorageConfig) -> StorageResult<Self> {
        let operator = Self::create_operator(&config)?;

        tracing::info!(
            target: TRACING_TARGET,
            backend = ?config.backend_type,
            root = %config.root,
            "Storage backend initialized"
        );

        Ok(Self { operator, config })
    }

    /// Returns the configuration for this backend.
    pub fn config(&self) -> &StorageConfig {
        &self.config
    }

    /// Returns the backend type.
    pub fn backend_type(&self) -> &BackendType {
        &self.config.backend_type
    }

    /// Reads a file from storage.
    pub async fn read(&self, path: &str) -> StorageResult<Vec<u8>> {
        tracing::debug!(
            target: TRACING_TARGET,
            path = %path,
            "Reading file"
        );

        let data = self.operator.read(path).await?.to_vec();

        tracing::debug!(
            target: TRACING_TARGET,
            path = %path,
            size = data.len(),
            "File read complete"
        );

        Ok(data)
    }

    /// Writes data to a file in storage.
    pub async fn write(&self, path: &str, data: &[u8]) -> StorageResult<()> {
        tracing::debug!(
            target: TRACING_TARGET,
            path = %path,
            size = data.len(),
            "Writing file"
        );

        self.operator.write(path, data.to_vec()).await?;

        tracing::debug!(
            target: TRACING_TARGET,
            path = %path,
            "File write complete"
        );

        Ok(())
    }

    /// Deletes a file from storage.
    pub async fn delete(&self, path: &str) -> StorageResult<()> {
        tracing::debug!(
            target: TRACING_TARGET,
            path = %path,
            "Deleting file"
        );

        self.operator.delete(path).await?;

        tracing::debug!(
            target: TRACING_TARGET,
            path = %path,
            "File deleted"
        );

        Ok(())
    }

    /// Checks if a file exists.
    pub async fn exists(&self, path: &str) -> StorageResult<bool> {
        Ok(self.operator.exists(path).await?)
    }

    /// Gets metadata for a file.
    pub async fn stat(&self, path: &str) -> StorageResult<FileMetadata> {
        let meta = self.operator.stat(path).await?;

        // Convert chrono DateTime to jiff Timestamp
        let last_modified = meta
            .last_modified()
            .and_then(|dt| jiff::Timestamp::from_second(dt.timestamp()).ok());

        Ok(FileMetadata {
            size: meta.content_length(),
            last_modified,
            content_type: meta.content_type().map(|s| s.to_string()),
        })
    }

    /// Lists files in a directory.
    pub async fn list(&self, path: &str) -> StorageResult<Vec<String>> {
        use futures::TryStreamExt;

        let entries: Vec<_> = self.operator.lister(path).await?.try_collect().await?;

        Ok(entries.into_iter().map(|e| e.path().to_string()).collect())
    }

    /// Copies a file from one path to another.
    pub async fn copy(&self, from: &str, to: &str) -> StorageResult<()> {
        tracing::debug!(
            target: TRACING_TARGET,
            from = %from,
            to = %to,
            "Copying file"
        );

        self.operator.copy(from, to).await?;

        Ok(())
    }

    /// Moves a file from one path to another.
    pub async fn rename(&self, from: &str, to: &str) -> StorageResult<()> {
        tracing::debug!(
            target: TRACING_TARGET,
            from = %from,
            to = %to,
            "Moving file"
        );

        self.operator.rename(from, to).await?;

        Ok(())
    }

    /// Creates an OpenDAL operator based on configuration.
    #[allow(unreachable_patterns)]
    fn create_operator(config: &StorageConfig) -> StorageResult<Operator> {
        match config.backend_type {
            #[cfg(feature = "s3")]
            BackendType::S3 => {
                let mut builder = services::S3::default().bucket(&config.root);

                if let Some(ref region) = config.region {
                    builder = builder.region(region);
                }

                if let Some(ref endpoint) = config.endpoint {
                    builder = builder.endpoint(endpoint);
                }

                if let Some(ref access_key_id) = config.access_key_id {
                    builder = builder.access_key_id(access_key_id);
                }

                if let Some(ref secret_access_key) = config.secret_access_key {
                    builder = builder.secret_access_key(secret_access_key);
                }

                Operator::new(builder)
                    .map(|op| op.finish())
                    .map_err(|e| StorageError::init(e.to_string()))
            }

            #[cfg(feature = "gcs")]
            BackendType::Gcs => {
                let builder = services::Gcs::default().bucket(&config.root);

                Operator::new(builder)
                    .map(|op| op.finish())
                    .map_err(|e| StorageError::init(e.to_string()))
            }

            #[cfg(feature = "azblob")]
            BackendType::AzureBlob => {
                let mut builder = services::Azblob::default().container(&config.root);

                if let Some(ref account_name) = config.account_name {
                    builder = builder.account_name(account_name);
                }

                if let Some(ref account_key) = config.account_key {
                    builder = builder.account_key(account_key);
                }

                Operator::new(builder)
                    .map(|op| op.finish())
                    .map_err(|e| StorageError::init(e.to_string()))
            }

            #[cfg(feature = "gdrive")]
            BackendType::GoogleDrive => {
                let mut builder = services::Gdrive::default().root(&config.root);

                if let Some(ref access_token) = config.access_token {
                    builder = builder.access_token(access_token);
                }

                Operator::new(builder)
                    .map(|op| op.finish())
                    .map_err(|e| StorageError::init(e.to_string()))
            }

            #[cfg(feature = "dropbox")]
            BackendType::Dropbox => {
                let mut builder = services::Dropbox::default().root(&config.root);

                if let Some(ref access_token) = config.access_token {
                    builder = builder.access_token(access_token);
                }

                if let Some(ref refresh_token) = config.refresh_token {
                    builder = builder.refresh_token(refresh_token);
                }

                if let Some(ref client_id) = config.access_key_id {
                    builder = builder.client_id(client_id);
                }

                if let Some(ref client_secret) = config.secret_access_key {
                    builder = builder.client_secret(client_secret);
                }

                Operator::new(builder)
                    .map(|op| op.finish())
                    .map_err(|e| StorageError::init(e.to_string()))
            }

            #[cfg(feature = "onedrive")]
            BackendType::OneDrive => {
                let mut builder = services::Onedrive::default().root(&config.root);

                if let Some(ref access_token) = config.access_token {
                    builder = builder.access_token(access_token);
                }

                Operator::new(builder)
                    .map(|op| op.finish())
                    .map_err(|e| StorageError::init(e.to_string()))
            }

            // This should never be reached if the config was properly created
            // with the same features enabled
            #[allow(unreachable_patterns)]
            _ => Err(StorageError::init(format!(
                "Backend type {:?} is not supported with current features",
                config.backend_type
            ))),
        }
    }
}

/// File metadata.
#[derive(Debug, Clone)]
pub struct FileMetadata {
    /// File size in bytes.
    pub size: u64,
    /// Last modification time.
    pub last_modified: Option<jiff::Timestamp>,
    /// Content type / MIME type.
    pub content_type: Option<String>,
}

impl std::fmt::Debug for StorageBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StorageBackend")
            .field("backend_type", &self.config.backend_type)
            .field("root", &self.config.root)
            .finish()
    }
}
