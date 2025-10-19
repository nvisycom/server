//! Upload context for MinIO operations.
//!
//! This module provides the UploadContext struct for tracking
//! upload operation metadata and parameters.

use std::collections::HashMap;

/// Context information for upload operations.
#[derive(Debug, Clone)]
pub struct UploadContext {
    /// Bucket name
    pub bucket: String,
    /// Object key
    pub key: String,
    /// Object size in bytes
    pub size: u64,
    /// Content type
    pub content_type: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl UploadContext {
    /// Creates a new UploadContext.
    pub fn new(bucket: impl Into<String>, key: impl Into<String>, size: u64) -> Self {
        Self {
            bucket: bucket.into(),
            key: key.into(),
            size,
            content_type: None,
            metadata: HashMap::new(),
        }
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

    /// Adds a single metadata entry.
    pub fn with_metadata_entry(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}
