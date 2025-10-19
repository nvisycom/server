//! Download context for MinIO operations.
//!
//! This module provides the DownloadContext struct for tracking
//! download operation metadata and parameters.

/// Context information for download operations.
#[derive(Debug, Clone)]
pub struct DownloadContext {
    /// Bucket name
    pub bucket: String,
    /// Object key
    pub key: String,
    /// Object size in bytes (if known)
    pub size: Option<u64>,
    /// Content type (if known)
    pub content_type: Option<String>,
}

impl DownloadContext {
    /// Creates a new DownloadContext.
    pub fn new(bucket: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            key: key.into(),
            size: None,
            content_type: None,
        }
    }

    /// Sets the object size.
    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    /// Sets the content type.
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }
}
