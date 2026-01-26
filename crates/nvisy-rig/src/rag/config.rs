//! RAG system configuration.

use std::num::NonZeroU32;

/// Configuration for the RAG system.
#[derive(Debug, Clone)]
pub struct RagConfig {
    /// Maximum chunk size in characters for text splitting.
    pub max_chunk_characters: u32,

    /// Number of characters to overlap between chunks.
    pub chunk_overlap_characters: Option<NonZeroU32>,

    /// Whether to trim whitespace from chunks.
    pub trim_whitespace: bool,

    /// Maximum chunks to retrieve per query.
    pub max_results: u32,

    /// Minimum similarity score (0.0 to 1.0). If `None`, no filtering is applied.
    pub min_score: Option<f64>,
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            max_chunk_characters: 1000,
            chunk_overlap_characters: None,
            trim_whitespace: true,
            max_results: 5,
            min_score: None,
        }
    }
}
