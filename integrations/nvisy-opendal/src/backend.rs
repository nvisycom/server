//! Storage backend implementation.

use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use nvisy_data::{DataError, DataInput, DataOutput, DataResult, InputContext, OutputContext};
use opendal::{Operator, services};

use crate::TRACING_TARGET;
use crate::azblob::AzureBlobConfig;
use crate::config::StorageConfig;
use crate::dropbox::DropboxConfig;
use crate::gcs::GcsConfig;
use crate::gdrive::GoogleDriveConfig;
use crate::onedrive::OneDriveConfig;
use crate::s3::S3Config;

/// Unified storage backend that wraps OpenDAL operators.
#[derive(Clone)]
pub struct StorageBackend {
    operator: Operator,
    config: StorageConfig,
}

impl StorageBackend {
    /// Creates a new storage backend from configuration.
    pub async fn new(config: StorageConfig) -> DataResult<Self> {
        let operator = Self::create_operator(&config)?;

        tracing::info!(
            target: TRACING_TARGET,
            backend = %config.backend_name(),
            "Storage backend initialized"
        );

        Ok(Self { operator, config })
    }

    /// Returns the configuration for this backend.
    pub fn config(&self) -> &StorageConfig {
        &self.config
    }

    /// Returns the backend name.
    pub fn backend_name(&self) -> &'static str {
        self.config.backend_name()
    }

    /// Gets metadata for a file.
    pub async fn stat(&self, path: &str) -> DataResult<FileMetadata> {
        let meta = self
            .operator
            .stat(path)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        let last_modified = meta
            .last_modified()
            .and_then(|dt| jiff::Timestamp::from_second(dt.timestamp()).ok());

        Ok(FileMetadata {
            size: meta.content_length(),
            last_modified,
            content_type: meta.content_type().map(|s| s.to_string()),
        })
    }

    /// Copies a file from one path to another.
    pub async fn copy(&self, from: &str, to: &str) -> DataResult<()> {
        tracing::debug!(
            target: TRACING_TARGET,
            from = %from,
            to = %to,
            "Copying file"
        );

        self.operator
            .copy(from, to)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        Ok(())
    }

    /// Moves a file from one path to another.
    pub async fn rename(&self, from: &str, to: &str) -> DataResult<()> {
        tracing::debug!(
            target: TRACING_TARGET,
            from = %from,
            to = %to,
            "Moving file"
        );

        self.operator
            .rename(from, to)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        Ok(())
    }

    /// Creates an OpenDAL operator based on configuration.
    fn create_operator(config: &StorageConfig) -> DataResult<Operator> {
        match config {
            StorageConfig::S3(cfg) => Self::create_s3_operator(cfg),
            StorageConfig::Gcs(cfg) => Self::create_gcs_operator(cfg),
            StorageConfig::AzureBlob(cfg) => Self::create_azblob_operator(cfg),
            StorageConfig::GoogleDrive(cfg) => Self::create_gdrive_operator(cfg),
            StorageConfig::Dropbox(cfg) => Self::create_dropbox_operator(cfg),
            StorageConfig::OneDrive(cfg) => Self::create_onedrive_operator(cfg),
        }
    }

    fn create_s3_operator(cfg: &S3Config) -> DataResult<Operator> {
        let mut builder = services::S3::default()
            .bucket(&cfg.bucket)
            .region(&cfg.region);

        if let Some(ref endpoint) = cfg.endpoint {
            builder = builder.endpoint(endpoint);
        }

        if let Some(ref access_key_id) = cfg.access_key_id {
            builder = builder.access_key_id(access_key_id);
        }

        if let Some(ref secret_access_key) = cfg.secret_access_key {
            builder = builder.secret_access_key(secret_access_key);
        }

        if let Some(ref prefix) = cfg.prefix {
            builder = builder.root(prefix);
        }

        Operator::new(builder)
            .map(|op| op.finish())
            .map_err(|e| DataError::backend(e.to_string()))
    }

    fn create_gcs_operator(cfg: &GcsConfig) -> DataResult<Operator> {
        let mut builder = services::Gcs::default().bucket(&cfg.bucket);

        if let Some(ref credentials) = cfg.credentials {
            builder = builder.credential(credentials);
        }

        if let Some(ref prefix) = cfg.prefix {
            builder = builder.root(prefix);
        }

        Operator::new(builder)
            .map(|op| op.finish())
            .map_err(|e| DataError::backend(e.to_string()))
    }

    fn create_azblob_operator(cfg: &AzureBlobConfig) -> DataResult<Operator> {
        let mut builder = services::Azblob::default()
            .container(&cfg.container)
            .account_name(&cfg.account_name);

        if let Some(ref account_key) = cfg.account_key {
            builder = builder.account_key(account_key);
        }

        if let Some(ref prefix) = cfg.prefix {
            builder = builder.root(prefix);
        }

        Operator::new(builder)
            .map(|op| op.finish())
            .map_err(|e| DataError::backend(e.to_string()))
    }

    fn create_gdrive_operator(cfg: &GoogleDriveConfig) -> DataResult<Operator> {
        let mut builder = services::Gdrive::default().root(&cfg.root);

        if let Some(ref access_token) = cfg.access_token {
            builder = builder.access_token(access_token);
        }

        Operator::new(builder)
            .map(|op| op.finish())
            .map_err(|e| DataError::backend(e.to_string()))
    }

    fn create_dropbox_operator(cfg: &DropboxConfig) -> DataResult<Operator> {
        let mut builder = services::Dropbox::default().root(&cfg.root);

        if let Some(ref access_token) = cfg.access_token {
            builder = builder.access_token(access_token);
        }

        if let Some(ref refresh_token) = cfg.refresh_token {
            builder = builder.refresh_token(refresh_token);
        }

        if let Some(ref client_id) = cfg.client_id {
            builder = builder.client_id(client_id);
        }

        if let Some(ref client_secret) = cfg.client_secret {
            builder = builder.client_secret(client_secret);
        }

        Operator::new(builder)
            .map(|op| op.finish())
            .map_err(|e| DataError::backend(e.to_string()))
    }

    fn create_onedrive_operator(cfg: &OneDriveConfig) -> DataResult<Operator> {
        let mut builder = services::Onedrive::default().root(&cfg.root);

        if let Some(ref access_token) = cfg.access_token {
            builder = builder.access_token(access_token);
        }

        Operator::new(builder)
            .map(|op| op.finish())
            .map_err(|e| DataError::backend(e.to_string()))
    }
}

#[async_trait]
impl DataInput for StorageBackend {
    async fn read(&self, _ctx: &InputContext, path: &str) -> DataResult<Bytes> {
        tracing::debug!(
            target: TRACING_TARGET,
            path = %path,
            "Reading file"
        );

        let data = self
            .operator
            .read(path)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        tracing::debug!(
            target: TRACING_TARGET,
            path = %path,
            size = data.len(),
            "File read complete"
        );

        Ok(data.to_bytes())
    }

    async fn read_stream(
        &self,
        _ctx: &InputContext,
        path: &str,
    ) -> DataResult<Box<dyn Stream<Item = DataResult<Bytes>> + Send + Unpin>> {
        use futures::StreamExt;

        tracing::debug!(
            target: TRACING_TARGET,
            path = %path,
            "Reading file as stream"
        );

        let reader = self
            .operator
            .reader(path)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        let stream = reader
            .into_bytes_stream(0..u64::MAX)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?
            .map(|result| result.map_err(|e| DataError::backend(e.to_string())));

        Ok(Box::new(stream))
    }

    async fn exists(&self, _ctx: &InputContext, path: &str) -> DataResult<bool> {
        self.operator
            .exists(path)
            .await
            .map_err(|e| DataError::backend(e.to_string()))
    }

    async fn list(&self, _ctx: &InputContext, prefix: &str) -> DataResult<Vec<String>> {
        use futures::TryStreamExt;

        let entries: Vec<_> = self
            .operator
            .lister(prefix)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?
            .try_collect()
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        Ok(entries.into_iter().map(|e| e.path().to_string()).collect())
    }
}

#[async_trait]
impl DataOutput for StorageBackend {
    async fn write(&self, _ctx: &OutputContext, path: &str, data: Bytes) -> DataResult<()> {
        tracing::debug!(
            target: TRACING_TARGET,
            path = %path,
            size = data.len(),
            "Writing file"
        );

        self.operator
            .write(path, data)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        tracing::debug!(
            target: TRACING_TARGET,
            path = %path,
            "File write complete"
        );

        Ok(())
    }

    async fn write_stream(
        &self,
        _ctx: &OutputContext,
        path: &str,
        stream: Box<dyn Stream<Item = DataResult<Bytes>> + Send + Unpin>,
    ) -> DataResult<()> {
        use futures::StreamExt;

        tracing::debug!(
            target: TRACING_TARGET,
            path = %path,
            "Writing file from stream"
        );

        let mut writer = self
            .operator
            .writer(path)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        let mut stream = stream;
        while let Some(result) = stream.next().await {
            let chunk = result?;
            writer
                .write(chunk)
                .await
                .map_err(|e| DataError::backend(e.to_string()))?;
        }

        writer
            .close()
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        tracing::debug!(
            target: TRACING_TARGET,
            path = %path,
            "File stream write complete"
        );

        Ok(())
    }

    async fn delete(&self, _ctx: &OutputContext, path: &str) -> DataResult<()> {
        tracing::debug!(
            target: TRACING_TARGET,
            path = %path,
            "Deleting file"
        );

        self.operator
            .delete(path)
            .await
            .map_err(|e| DataError::backend(e.to_string()))?;

        tracing::debug!(
            target: TRACING_TARGET,
            path = %path,
            "File deleted"
        );

        Ok(())
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
            .field("backend", &self.config.backend_name())
            .finish()
    }
}
