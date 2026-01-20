//! Object storage functionality using NATS JetStream.
//!
//! This module provides object storage capabilities using NATS JetStream
//! as the underlying storage mechanism, with streaming upload support and
//! on-the-fly SHA-256 hash computation.
//!
//! # Architecture
//!
//! ## Store
//! - [`ObjectStore<B, K>`] - Type-safe object store with bucket and key configuration
//!
//! ## Key Types
//! - [`FileKey`] - Unique key for files (workspace + object ID)
//! - [`AccountKey`] - Key for account-scoped objects (account ID)
//!
//! ## Bucket Types
//! - [`FilesBucket`] - Primary file storage (no expiration)
//! - [`IntermediatesBucket`] - Temporary processing artifacts (7 day TTL)
//! - [`ThumbnailsBucket`] - Document thumbnails (no expiration)
//! - [`AvatarsBucket`] - Account avatars (no expiration)
//!
//! ## Common Types
//! - [`PutResult`] - Result of upload operations with size and SHA-256 hash
//! - [`GetResult`] - Result of download operations with streaming reader

mod hashing_reader;
mod object_bucket;
mod object_data;
mod object_key;
mod object_store;

pub use object_bucket::{
    AvatarsBucket, FilesBucket, IntermediatesBucket, ObjectBucket, ThumbnailsBucket,
};
pub use object_data::{GetResult, PutResult};
pub use object_key::{AccountKey, FileKey, ObjectKey};
pub use object_store::ObjectStore;
