//! Split chunk types.

use super::ChunkMetadata;

/// A chunk produced by the text splitter (borrows from source text).
#[derive(Debug)]
pub struct Chunk<'a> {
    /// The chunk text content (borrowed from original).
    pub text: &'a str,
    /// Metadata about the chunk's position.
    pub metadata: ChunkMetadata,
}

impl<'a> Chunk<'a> {
    /// Creates a new chunk.
    pub fn new(text: &'a str, metadata: ChunkMetadata) -> Self {
        Self { text, metadata }
    }

    /// Converts to an owned chunk.
    pub fn into_owned(self) -> OwnedChunk {
        OwnedChunk {
            text: self.text.to_string(),
            metadata: self.metadata,
        }
    }
}

/// An owned version of Chunk.
#[derive(Debug, Clone)]
pub struct OwnedChunk {
    /// The chunk text content.
    pub text: String,
    /// Metadata about the chunk's position.
    pub metadata: ChunkMetadata,
}

impl OwnedChunk {
    /// Creates a new owned chunk.
    pub fn new(text: String, metadata: ChunkMetadata) -> Self {
        Self { text, metadata }
    }
}
