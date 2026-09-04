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

mod client;
mod error;
mod key;

pub use client::{BlobStore, GetObject, S3Config};
pub use error::{Error, Result};
pub use key::{
    AccountAvatarKey, ArtifactKey, AuditKey, Bucket, FileKey, ObjectKey, WorkspaceAvatarKey,
};
