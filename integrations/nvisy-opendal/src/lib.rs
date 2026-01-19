//! Storage backends using OpenDAL.
//!
//! This crate provides storage implementations that implement the
//! [`DataInput`] and [`DataOutput`] traits from `nvisy-data`.

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod azblob;
pub mod dropbox;
pub mod gcs;
pub mod gdrive;
pub mod onedrive;
pub mod s3;

mod backend;
mod config;

pub use backend::{FileMetadata, StorageBackend};
pub use config::{
    AzureBlobConfig, DropboxConfig, GcsConfig, GoogleDriveConfig, OneDriveConfig, S3Config,
    StorageConfig,
};

// Re-export types from nvisy-data for convenience
pub use nvisy_data::{DataError, DataInput, DataOutput, DataResult, InputContext, OutputContext};

/// Tracing target for storage operations.
pub const TRACING_TARGET: &str = "nvisy_opendal";
