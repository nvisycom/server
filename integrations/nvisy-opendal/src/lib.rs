#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod azblob;
pub mod dropbox;
pub mod gcs;
pub mod gdrive;
pub mod onedrive;
pub mod s3;

mod backend;
mod config;
mod error;

pub use backend::{FileMetadata, StorageBackend};
pub use config::{
    AzureBlobConfig, DropboxConfig, GcsConfig, GoogleDriveConfig, OneDriveConfig, S3Config,
    StorageConfig,
};
pub use error::{StorageError, StorageResult};

/// Tracing target for storage operations.
pub const TRACING_TARGET: &str = "nvisy_opendal";
