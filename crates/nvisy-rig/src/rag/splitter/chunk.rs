//! Split chunk types.

use super::SplitMetadata;

/// A chunk produced by the text splitter (borrows from source text).
#[derive(Debug)]
pub struct SplitChunk<'a> {
    /// The chunk text content (borrowed from original).
    pub text: &'a str,

    /// Metadata about the chunk's position.
    pub metadata: SplitMetadata,
}

impl SplitChunk<'_> {
    /// Converts to an owned chunk.
    pub fn into_owned(self) -> OwnedSplitChunk {
        OwnedSplitChunk {
            text: self.text.to_string(),
            metadata: self.metadata,
        }
    }
}

/// An owned version of SplitChunk.
#[derive(Debug, Clone)]
pub struct OwnedSplitChunk {
    /// The chunk text content.
    pub text: String,

    /// Metadata about the chunk's position.
    pub metadata: SplitMetadata,
}
