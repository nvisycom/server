//! Storage configuration types.

use serde::{Deserialize, Serialize};

/// Storage backend type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendType {
    /// Amazon S3 compatible storage.
    #[cfg(feature = "s3")]
    S3,

    /// Google Cloud Storage.
    #[cfg(feature = "gcs")]
    Gcs,

    /// Azure Blob Storage.
    #[cfg(feature = "azblob")]
    AzureBlob,

    /// Google Drive.
    #[cfg(feature = "gdrive")]
    GoogleDrive,

    /// Dropbox.
    #[cfg(feature = "dropbox")]
    Dropbox,

    /// OneDrive.
    #[cfg(feature = "onedrive")]
    OneDrive,
}

/// Configuration for a storage backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Type of storage backend.
    pub backend_type: BackendType,

    /// Root path or bucket/container name.
    pub root: String,

    /// Region (for cloud storage).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,

    /// Endpoint URL (for S3-compatible storage).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,

    /// Access key ID / Client ID (for cloud storage).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_key_id: Option<String>,

    /// Secret access key / Client secret (for cloud storage).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_access_key: Option<String>,

    /// Account name (for Azure Blob Storage).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_name: Option<String>,

    /// Account key (for Azure Blob Storage).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_key: Option<String>,

    /// OAuth access token (for Google Drive, Dropbox, OneDrive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,

    /// OAuth refresh token (for Google Drive, Dropbox, OneDrive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

impl StorageConfig {
    /// Creates an S3 storage configuration.
    #[cfg(feature = "s3")]
    pub fn s3(bucket: impl Into<String>, region: impl Into<String>) -> Self {
        Self {
            backend_type: BackendType::S3,
            root: bucket.into(),
            region: Some(region.into()),
            endpoint: None,
            access_key_id: None,
            secret_access_key: None,
            account_name: None,
            account_key: None,
            access_token: None,
            refresh_token: None,
        }
    }

    /// Creates an S3-compatible storage configuration with custom endpoint.
    #[cfg(feature = "s3")]
    pub fn s3_compatible(
        bucket: impl Into<String>,
        endpoint: impl Into<String>,
        region: impl Into<String>,
    ) -> Self {
        Self {
            backend_type: BackendType::S3,
            root: bucket.into(),
            region: Some(region.into()),
            endpoint: Some(endpoint.into()),
            access_key_id: None,
            secret_access_key: None,
            account_name: None,
            account_key: None,
            access_token: None,
            refresh_token: None,
        }
    }

    /// Creates a GCS storage configuration.
    #[cfg(feature = "gcs")]
    pub fn gcs(bucket: impl Into<String>) -> Self {
        Self {
            backend_type: BackendType::Gcs,
            root: bucket.into(),
            region: None,
            endpoint: None,
            access_key_id: None,
            secret_access_key: None,
            account_name: None,
            account_key: None,
            access_token: None,
            refresh_token: None,
        }
    }

    /// Creates an Azure Blob Storage configuration.
    #[cfg(feature = "azblob")]
    pub fn azure_blob(container: impl Into<String>, account_name: impl Into<String>) -> Self {
        Self {
            backend_type: BackendType::AzureBlob,
            root: container.into(),
            region: None,
            endpoint: None,
            access_key_id: None,
            secret_access_key: None,
            account_name: Some(account_name.into()),
            account_key: None,
            access_token: None,
            refresh_token: None,
        }
    }

    /// Creates a Google Drive storage configuration.
    #[cfg(feature = "gdrive")]
    pub fn google_drive(root: impl Into<String>) -> Self {
        Self {
            backend_type: BackendType::GoogleDrive,
            root: root.into(),
            region: None,
            endpoint: None,
            access_key_id: None,
            secret_access_key: None,
            account_name: None,
            account_key: None,
            access_token: None,
            refresh_token: None,
        }
    }

    /// Creates a Dropbox storage configuration.
    #[cfg(feature = "dropbox")]
    pub fn dropbox(root: impl Into<String>) -> Self {
        Self {
            backend_type: BackendType::Dropbox,
            root: root.into(),
            region: None,
            endpoint: None,
            access_key_id: None,
            secret_access_key: None,
            account_name: None,
            account_key: None,
            access_token: None,
            refresh_token: None,
        }
    }

    /// Creates a OneDrive storage configuration.
    #[cfg(feature = "onedrive")]
    pub fn onedrive(root: impl Into<String>) -> Self {
        Self {
            backend_type: BackendType::OneDrive,
            root: root.into(),
            region: None,
            endpoint: None,
            access_key_id: None,
            secret_access_key: None,
            account_name: None,
            account_key: None,
            access_token: None,
            refresh_token: None,
        }
    }

    /// Sets the access credentials for S3/GCS.
    #[cfg(any(feature = "s3", feature = "gcs"))]
    pub fn with_credentials(
        mut self,
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
    ) -> Self {
        self.access_key_id = Some(access_key_id.into());
        self.secret_access_key = Some(secret_access_key.into());
        self
    }

    /// Sets the Azure account key.
    #[cfg(feature = "azblob")]
    pub fn with_account_key(mut self, account_key: impl Into<String>) -> Self {
        self.account_key = Some(account_key.into());
        self
    }

    /// Sets the OAuth access token for OAuth-based backends.
    #[cfg(any(feature = "gdrive", feature = "dropbox", feature = "onedrive"))]
    pub fn with_access_token(mut self, access_token: impl Into<String>) -> Self {
        self.access_token = Some(access_token.into());
        self
    }

    /// Sets the OAuth refresh token for OAuth-based backends.
    #[cfg(any(feature = "gdrive", feature = "dropbox", feature = "onedrive"))]
    pub fn with_refresh_token(mut self, refresh_token: impl Into<String>) -> Self {
        self.refresh_token = Some(refresh_token.into());
        self
    }

    /// Sets the client credentials for OAuth-based backends.
    #[cfg(any(feature = "gdrive", feature = "dropbox", feature = "onedrive"))]
    pub fn with_client_credentials(
        mut self,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self {
        self.access_key_id = Some(client_id.into());
        self.secret_access_key = Some(client_secret.into());
        self
    }
}
