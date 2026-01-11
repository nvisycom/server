//! Object storage functionality using NATS JetStream.
//!
//! This module provides object storage capabilities using NATS JetStream
//! as the underlying storage mechanism, with streaming upload support and
//! on-the-fly SHA-256 hash computation.
//!
//! # Architecture
//!
//! ## Generic Store
//! - [`ObjectStore`] - Generic object store wrapper with streaming support
//!
//! ## Document Storage
//! - [`DocumentStore`] - Specialized store for document files
//! - [`DocumentKey`] - Unique key for documents (workspace + object ID)
//!
//! ## Avatar Storage
//! - [`AvatarStore`] - Specialized store for account avatars
//! - [`AvatarKey`] - Key for avatars (account ID)
//!
//! ## Thumbnail Storage
//! - [`ThumbnailStore`] - Specialized store for document thumbnails
//! - Uses [`DocumentKey`] for addressing
//!
//! ## Common Types
//! - [`PutResult`] - Result of upload operations with size and SHA-256 hash
//! - [`GetResult`] - Result of download operations with streaming reader

mod avatar_bucket;
mod avatar_key;
mod avatar_store;
mod document_bucket;
mod document_key;
mod document_store;
mod hashing_reader;
mod object_data;
mod object_store;
mod thumbnail_bucket;
mod thumbnail_store;

pub use avatar_key::AvatarKey;
pub use avatar_store::AvatarStore;
pub use document_bucket::{DocumentBucket, Files, Intermediates};
pub use document_key::DocumentKey;
pub use document_store::DocumentStore;
pub use object_data::{GetResult, PutResult};
pub use object_store::ObjectStore;
pub use thumbnail_store::ThumbnailStore;
