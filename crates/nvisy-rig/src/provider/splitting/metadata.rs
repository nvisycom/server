//! Split chunk metadata.

use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

/// Metadata about a chunk's location in the source document.
///
/// This is the unified chunk metadata type used throughout the system:
/// - Created during text splitting with offset information
/// - Stored in the database with the chunk
/// - Retrieved during search operations
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkMetadata {
    /// Chunk index within the source (0-based).
    pub index: u32,

    /// Start byte offset in the source text.
    pub start_offset: u32,

    /// End byte offset in the source text.
    pub end_offset: u32,

    /// Page number (1-indexed, if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<NonZeroU32>,

    /// Section or heading the chunk belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
}

impl ChunkMetadata {
    /// Creates metadata with index and offset information.
    pub fn new(index: u32, start_offset: u32, end_offset: u32) -> Self {
        Self {
            index,
            start_offset,
            end_offset,
            page: None,
            section: None,
        }
    }

    /// Creates metadata from JSON (used when loading from database).
    ///
    /// The `index` parameter overrides any index value in the JSON.
    pub fn from_json(json: &serde_json::Value, index: u32) -> Self {
        let mut metadata: Self = serde_json::from_value(json.clone()).unwrap_or_default();
        metadata.index = index;
        metadata
    }

    /// Sets the page number.
    pub fn with_page(mut self, page: NonZeroU32) -> Self {
        self.page = Some(page);
        self
    }

    /// Sets the section name.
    pub fn with_section(mut self, section: impl Into<String>) -> Self {
        self.section = Some(section.into());
        self
    }

    /// Returns the byte length of the chunk.
    pub fn byte_len(&self) -> u32 {
        self.end_offset.saturating_sub(self.start_offset)
    }

    /// Returns the byte range for content extraction.
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.start_offset as usize..self.end_offset as usize
    }

    /// Returns a location string for display (e.g., "page 5, 'Introduction', chunk 3").
    pub fn location_string(&self) -> String {
        let mut parts = Vec::new();

        if let Some(page) = self.page {
            parts.push(format!("page {page}"));
        }

        if let Some(section) = &self.section {
            parts.push(format!("'{section}'"));
        }

        parts.push(format!("chunk {}", self.index + 1));

        parts.join(", ")
    }
}
