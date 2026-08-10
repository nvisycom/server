//! Object storage functionality using NATS JetStream.
//!
//! This module provides object storage capabilities using NATS JetStream
//! as the underlying storage mechanism, with streaming upload support and
//! on-the-fly SHA-256 hash computation.
//!
//! # Architecture
//!
//! ## Store
//! - [`ObjectStore<B>`] - Type-safe object store keyed by its bucket's key type
//!
//! ## Key Types
//! - [`FileKey`] - Unique key for files (workspace + object ID)
//! - [`AccountAvatarKey`] - Key for account-scoped objects (account ID)
//! - [`AuditKey`] - Key for redaction audit objects
//!
//! ## Bucket Types
//! - [`FilesBucket`] - Primary file storage (no expiration)
//! - [`AuditBucket`] - Redaction audits (per-workspace retention)
//! - [`ThumbnailsBucket`] - Document thumbnails (no expiration)
//! - [`AvatarsBucket`] - Account avatars (no expiration)
//!
//! ## Common Types
//! - [`PutResult`] - Result of upload operations with size and SHA-256 hash
//! - [`GetResult`] - Result of download operations with streaming reader

mod object_bucket;
mod object_data;
mod object_key;
mod object_store;

pub use object_bucket::{
    AuditBucket, AvatarsBucket, FilesBucket, ObjectBucket, ThumbnailsBucket, WorkspaceAvatarsBucket,
};
pub use object_data::{GetResult, PutResult};
pub use object_key::{AccountAvatarKey, AuditKey, FileKey, ObjectKey, WorkspaceAvatarKey};
pub use object_store::ObjectStore;
