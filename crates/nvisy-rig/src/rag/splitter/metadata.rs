//! Split chunk metadata.

use serde::{Deserialize, Serialize};

/// Metadata about a split chunk's location in the source text.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SplitMetadata {
    /// Page number (1-indexed, if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,

    /// Start byte offset in the source text.
    pub start_offset: u32,

    /// End byte offset in the source text.
    pub end_offset: u32,

    /// Chunk index within the source (0-based).
    pub chunk_index: u32,
}

impl SplitMetadata {
    /// Creates metadata with offset information.
    pub fn new(chunk_index: u32, start_offset: u32, end_offset: u32) -> Self {
        Self {
            page: None,
            start_offset,
            end_offset,
            chunk_index,
        }
    }
}
