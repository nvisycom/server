#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

//! First-party S3-compatible blob storage.
//!
//! Holds Nvisy's own objects — uploaded files, detection audits, enrichment
//! intermediates, redacted output, and avatars — in a single S3 bucket, one
//! logical [`Bucket`] per key prefix. Backed by the AWS S3 SDK, so it targets AWS
//! S3 or any S3-compatible server (RustFS, MinIO, R2, …) by endpoint.
//!
//! Objects are already encrypted by the caller before they reach [`BlobStore`],
//! so the store only ever sees ciphertext.

mod bucket;
mod config;
mod connect;
mod error;
mod health;
mod key;
mod store;

pub use bucket::Bucket;
pub use config::S3Config;
pub use error::{S3Error, S3Result};
pub use key::{AccountAvatarKey, AuditKey, FileKey, ObjectKey, WorkspaceAvatarKey};
pub use store::{BlobStore, GetObject};
