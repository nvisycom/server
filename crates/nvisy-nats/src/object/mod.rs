//! Object storage functionality using NATS JetStream.
//!
//! This module provides document storage capabilities using NATS JetStream
//! as the underlying storage mechanism, with streaming upload support and
//! on-the-fly SHA-256 hash computation.
//!
//! # Architecture
//!
//! - [`ObjectStore`] - Generic object store wrapper with streaming support
//! - [`DocumentStore`] - Specialized store for document files using [`DocumentKey`]
//! - [`DocumentBucket`] - Bucket configuration for document storage
//! - [`DocumentKey`] - Unique key for document objects (workspace + object ID)
//! - [`PutResult`] - Result of upload operations with size and SHA-256 hash
//! - [`GetResult`] - Result of download operations with streaming reader

mod document_bucket;
mod document_key;
mod document_store;
mod hashing_reader;
mod object_data;
mod object_store;

pub use document_bucket::DocumentBucket;
pub use document_key::DocumentKey;
pub use document_store::DocumentStore;
pub use object_data::{GetResult, PutResult};
pub use object_store::ObjectStore;
