//! Storage configuration types.

use serde::{Deserialize, Serialize};

// Re-export configs from backend modules
pub use crate::azblob::AzureBlobConfig;
pub use crate::dropbox::DropboxConfig;
pub use crate::gcs::GcsConfig;
pub use crate::gdrive::GoogleDriveConfig;
pub use crate::onedrive::OneDriveConfig;
pub use crate::s3::S3Config;

/// Storage backend configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum StorageConfig {
    /// Amazon S3 compatible storage.
    S3(S3Config),
    /// Google Cloud Storage.
    Gcs(GcsConfig),
    /// Azure Blob Storage.
    AzureBlob(AzureBlobConfig),
    /// Google Drive.
    GoogleDrive(GoogleDriveConfig),
    /// Dropbox.
    Dropbox(DropboxConfig),
    /// OneDrive.
    OneDrive(OneDriveConfig),
}

impl StorageConfig {
    /// Returns the backend name as a static string.
    pub fn backend_name(&self) -> &'static str {
        match self {
            Self::S3(_) => "s3",
            Self::Gcs(_) => "gcs",
            Self::AzureBlob(_) => "azblob",
            Self::GoogleDrive(_) => "gdrive",
            Self::Dropbox(_) => "dropbox",
            Self::OneDrive(_) => "onedrive",
        }
    }
}
