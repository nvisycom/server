//! Chunk transformer configuration.

use serde::{Deserialize, Serialize};

/// Configuration for chunking content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkConfig {
    /// Chunking strategy.
    #[serde(flatten)]
    pub strategy: ChunkStrategy,
}

/// Chunking strategy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "strategy", rename_all = "snake_case")]
pub enum ChunkStrategy {
    /// Chunk by character count.
    Character {
        /// Maximum chunk size in characters.
        max_characters: u32,
        /// Overlap between chunks in characters.
        #[serde(default)]
        overlap: u32,
    },
    /// Chunk by page boundaries.
    Page {
        /// Maximum pages per chunk.
        #[serde(default = "default_max_pages")]
        max_pages: u32,
        /// Overlap between chunks in pages.
        #[serde(default)]
        overlap: u32,
    },
    /// Chunk by document sections/headings.
    Section {
        /// Maximum sections per chunk.
        #[serde(default = "default_max_sections")]
        max_sections: u32,
        /// Overlap between chunks in sections.
        #[serde(default)]
        overlap: u32,
    },
    /// Chunk by semantic similarity.
    Similarity {
        /// Maximum chunk size in characters.
        max_characters: u32,
        /// Similarity score threshold (0.0 to 1.0).
        #[serde(default = "default_score")]
        score: f32,
    },
}

fn default_max_pages() -> u32 {
    1
}

fn default_max_sections() -> u32 {
    1
}

fn default_score() -> f32 {
    0.5
}
