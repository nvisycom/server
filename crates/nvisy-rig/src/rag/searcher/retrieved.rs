//! Retrieved chunk types for search results.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Metadata about a chunk's location in the source document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkMetadata {
    /// Page number (1-indexed, if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,

    /// Section or heading the chunk belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,

    /// Start byte offset in the source file.
    pub start_offset: u32,

    /// End byte offset in the source file.
    pub end_offset: u32,

    /// Chunk index within the file (0-based).
    pub chunk_index: u32,
}

impl ChunkMetadata {
    /// Creates metadata with offset information.
    pub fn new(chunk_index: u32, start_offset: u32, end_offset: u32) -> Self {
        Self {
            page: None,
            section: None,
            start_offset,
            end_offset,
            chunk_index,
        }
    }

    /// Creates metadata from JSON and chunk index.
    pub fn from_json(json: &serde_json::Value, chunk_index: i32) -> Self {
        let start_offset = json
            .get("start_offset")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let end_offset = json.get("end_offset").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

        let page = json.get("page").and_then(|v| v.as_u64()).map(|p| p as u32);

        let section = json
            .get("section")
            .and_then(|v| v.as_str())
            .map(String::from);

        Self {
            page,
            section,
            start_offset,
            end_offset,
            chunk_index: chunk_index as u32,
        }
    }

    /// Sets the page number.
    pub fn with_page(mut self, page: u32) -> Self {
        self.page = Some(page);
        self
    }

    /// Sets the section name.
    pub fn with_section(mut self, section: impl Into<String>) -> Self {
        self.section = Some(section.into());
        self
    }

    /// Returns the byte range for content extraction.
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.start_offset as usize..self.end_offset as usize
    }

    /// Returns the content length in bytes.
    pub fn content_len(&self) -> u32 {
        self.end_offset.saturating_sub(self.start_offset)
    }

    /// Returns a location string for display.
    pub fn location_string(&self) -> String {
        let mut parts = Vec::new();

        if let Some(page) = self.page {
            parts.push(format!("page {page}"));
        }

        if let Some(section) = &self.section {
            parts.push(format!("'{section}'"));
        }

        parts.push(format!("chunk {}", self.chunk_index + 1));

        parts.join(", ")
    }
}

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
