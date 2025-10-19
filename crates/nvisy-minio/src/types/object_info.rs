//! Object information structures for MinIO storage.
//!
//! This module provides data structures for representing object metadata
//! and properties in MinIO object storage.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Information about a MinIO object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectInfo {
    /// Object key/path.
    pub key: String,
    /// Object size in bytes.
    pub size: u64,
    /// Last modified timestamp.
    pub last_modified: OffsetDateTime,
    /// ETag of the object.
    pub etag: Option<String>,
    /// Content type/MIME type.
    pub content_type: Option<String>,
    /// Object metadata.
    pub metadata: HashMap<String, String>,
    /// Object tags.
    pub tags: HashMap<String, String>,
}

impl ObjectInfo {
    /// Creates a new ObjectInfo.
    pub fn new(key: impl Into<String>, size: u64, last_modified: OffsetDateTime) -> Self {
        Self {
            key: key.into(),
            size,
            last_modified,
            etag: None,
            content_type: None,
            metadata: HashMap::new(),
            tags: HashMap::new(),
        }
    }

    /// Sets the ETag.
    pub fn with_etag(mut self, etag: impl Into<String>) -> Self {
        self.etag = Some(etag.into());
        self
    }

    /// Sets the content type.
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    /// Sets metadata.
    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = metadata;
        self
    }

    /// Sets tags.
    pub fn with_tags(mut self, tags: HashMap<String, String>) -> Self {
        self.tags = tags;
        self
    }
}
