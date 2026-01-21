//! Split chunk metadata.

use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

/// Metadata about a split chunk's location in the source text.
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
}

impl ChunkMetadata {
    /// Creates metadata with offset information.
    pub fn new(index: u32, start_offset: u32, end_offset: u32) -> Self {
        Self {
            index,
            start_offset,
            end_offset,
            page: None,
        }
    }

    /// Sets the page number.
    pub fn with_page(mut self, page: NonZeroU32) -> Self {
        self.page = Some(page);
        self
    }

    /// Returns the byte length of the chunk.
    pub fn byte_len(&self) -> u32 {
        self.end_offset - self.start_offset
    }
}
