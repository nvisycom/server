#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod backend;
mod config;
mod error;

#[doc(hidden)]
pub mod prelude;

pub use backend::{FileMetadata, StorageBackend};
pub use config::{BackendType, StorageConfig};
pub use error::{StorageError, StorageResult};

/// Tracing target for storage operations.
pub const TRACING_TARGET: &str = "nvisy_opendal";
