//! Content payload with source identity and optional content-type.

use bytes::Bytes;

use super::ContentSource;

/// A blob of content together with its [`ContentSource`] identity and an
/// optional MIME content-type.
pub struct ContentData {
    /// The unique source identifier for this content.
    pub content_source: ContentSource,
    data: Bytes,
    content_type: Option<String>,
}

impl ContentData {
    /// Create a new [`ContentData`] from a source and raw bytes.
    pub fn new(source: ContentSource, data: Bytes) -> Self {
        Self {
            content_source: source,
            data,
            content_type: None,
        }
    }

    /// Attach a content-type (MIME) to this content.
    pub fn with_content_type(mut self, ct: impl Into<String>) -> Self {
        self.content_type = Some(ct.into());
        self
    }

    /// Return the content-type, if set.
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    /// Return a clone of the underlying [`Bytes`].
    pub fn to_bytes(&self) -> Bytes {
        self.data.clone()
    }

    /// Return a byte-slice view of the content.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}
