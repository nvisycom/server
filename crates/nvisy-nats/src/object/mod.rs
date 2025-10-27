//! Object storage functionality using NATS JetStream.
//!
//! This module provides object storage capabilities for files and binary data
//! using NATS JetStream as the underlying storage mechanism.

pub mod content_data;
pub mod content_source;
pub mod document_store;
pub mod object_headers;
pub mod object_key;
pub mod object_key_data;
pub mod object_metadata;
pub mod object_store;

// Re-export main types
pub use content_data::ContentData;
pub use content_source::ContentSource;
pub use document_store::DocumentFileStore;
pub use object_headers::ObjectHeaders;
pub use object_key::{DocumentLabel, InputFiles, IntermediateFiles, ObjectKey, OutputFiles};
pub use object_key_data::ObjectKeyData;
pub use object_metadata::ObjectMetadata;
pub use object_store::{ObjectStore, ObjectStoreStats, PutResult};
