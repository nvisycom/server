//! Object storage functionality using NATS JetStream.
//!
//! This module provides object storage capabilities for files and binary data
//! using NATS JetStream as the underlying storage mechanism.

pub mod store;
pub mod types;

// Re-export main types
pub use store::ObjectStore;
pub use types::{GetResult, ObjectInfo, ObjectMeta, PutResult};
