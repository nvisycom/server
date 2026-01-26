//! Retrieved chunk types for search results.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Re-export ChunkMetadata from the canonical location
pub use crate::provider::splitting::ChunkMetadata;

/// A retrieved chunk with content and similarity score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedChunk {
    /// Chunk ID from the database.
    pub id: Uuid,

    /// Parent file ID.
    pub file_id: Uuid,

    /// Similarity score (0.0 to 1.0, higher is more similar).
    pub score: f64,

    /// Chunk metadata (offsets, page, section).
    pub metadata: ChunkMetadata,

    /// The actual text content (retrieved from NATS).
    /// This is `None` until content is fetched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

impl RetrievedChunk {
    /// Creates a new retrieved chunk without content.
    pub fn new(id: Uuid, file_id: Uuid, score: f64, metadata: ChunkMetadata) -> Self {
        Self {
            id,
            file_id,
            score,
            metadata,
            content: None,
        }
    }

    /// Sets the content after retrieval from NATS.
    pub fn with_content(mut self, content: String) -> Self {
        self.content = Some(content);
        self
    }

    /// Returns whether content has been loaded.
    pub fn has_content(&self) -> bool {
        self.content.is_some()
    }

    /// Returns the content, or a placeholder if not loaded.
    pub fn content_or_placeholder(&self) -> &str {
        self.content
            .as_deref()
            .unwrap_or("[Content not yet loaded]")
    }
}
