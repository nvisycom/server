//! Chunking strategy configurations for text splitting.

use serde::{Deserialize, Serialize};

/// Chunking strategy configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "strategy", rename_all = "snake_case")]
pub enum ChunkingStrategy {
    /// Split by character count.
    Character(CharacterChunkingConfig),
    /// Split by sentences.
    Sentence(SentenceChunkingConfig),
    /// Split by paragraphs.
    Paragraph(ParagraphChunkingConfig),
    /// Split by page boundaries (for PDFs).
    Page(PageChunkingConfig),
    /// Split by document structure/titles.
    Title(TitleChunkingConfig),
    /// Recursive splitting with fallback strategies.
    Recursive(RecursiveChunkingConfig),
    /// Semantic/similarity-based chunking.
    Semantic(SemanticChunkingConfig),
    /// Contextual chunking with LLM-assisted boundaries.
    Contextual(ContextualChunkingConfig),
}

/// Character-based chunking configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterChunkingConfig {
    /// Maximum chunk size in characters.
    pub max_size: usize,
    /// Overlap between chunks in characters.
    #[serde(default)]
    pub overlap: usize,
    /// Separator to split on (defaults to whitespace).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub separator: Option<String>,
    /// Whether to trim whitespace from chunks.
    #[serde(default = "default_true")]
    pub trim: bool,
}

impl CharacterChunkingConfig {
    /// Creates a new character chunking config.
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            overlap: 0,
            separator: None,
            trim: true,
        }
    }

    /// Sets the overlap.
    pub fn with_overlap(mut self, overlap: usize) -> Self {
        self.overlap = overlap;
        self
    }

    /// Sets the separator.
    pub fn with_separator(mut self, separator: impl Into<String>) -> Self {
        self.separator = Some(separator.into());
        self
    }
}

/// Sentence-based chunking configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SentenceChunkingConfig {
    /// Maximum number of sentences per chunk.
    pub max_sentences: usize,
    /// Overlap in sentences.
    #[serde(default)]
    pub overlap_sentences: usize,
    /// Maximum chunk size in characters (soft limit).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_size: Option<usize>,
}

impl SentenceChunkingConfig {
    /// Creates a new sentence chunking config.
    pub fn new(max_sentences: usize) -> Self {
        Self {
            max_sentences,
            overlap_sentences: 0,
            max_size: None,
        }
    }

    /// Sets the overlap.
    pub fn with_overlap(mut self, overlap: usize) -> Self {
        self.overlap_sentences = overlap;
        self
    }
}

/// Paragraph-based chunking configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParagraphChunkingConfig {
    /// Maximum number of paragraphs per chunk.
    pub max_paragraphs: usize,
    /// Maximum chunk size in characters (soft limit).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_size: Option<usize>,
    /// Minimum paragraph length to consider (filters short lines).
    #[serde(default)]
    pub min_paragraph_length: usize,
}

impl ParagraphChunkingConfig {
    /// Creates a new paragraph chunking config.
    pub fn new(max_paragraphs: usize) -> Self {
        Self {
            max_paragraphs,
            max_size: None,
            min_paragraph_length: 0,
        }
    }
}

/// Page-based chunking configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageChunkingConfig {
    /// Maximum number of pages per chunk.
    #[serde(default = "default_one")]
    pub max_pages: usize,
    /// Whether to preserve page boundaries exactly.
    #[serde(default = "default_true")]
    pub preserve_boundaries: bool,
}

impl Default for PageChunkingConfig {
    fn default() -> Self {
        Self {
            max_pages: 1,
            preserve_boundaries: true,
        }
    }
}

/// Title/heading-based chunking configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TitleChunkingConfig {
    /// Heading levels to split on (1 = h1, 2 = h2, etc.).
    #[serde(default = "default_heading_levels")]
    pub heading_levels: Vec<u8>,
    /// Whether to include the heading in each chunk.
    #[serde(default = "default_true")]
    pub include_heading: bool,
    /// Maximum chunk size in characters (soft limit).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_size: Option<usize>,
}

impl Default for TitleChunkingConfig {
    fn default() -> Self {
        Self {
            heading_levels: default_heading_levels(),
            include_heading: true,
            max_size: None,
        }
    }
}

/// Recursive chunking configuration with fallback strategies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecursiveChunkingConfig {
    /// Maximum chunk size in characters.
    pub max_size: usize,
    /// Overlap between chunks.
    #[serde(default)]
    pub overlap: usize,
    /// Separators to try in order (from most to least preferred).
    #[serde(default = "default_recursive_separators")]
    pub separators: Vec<String>,
}

impl RecursiveChunkingConfig {
    /// Creates a new recursive chunking config.
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            overlap: 0,
            separators: default_recursive_separators(),
        }
    }

    /// Sets the overlap.
    pub fn with_overlap(mut self, overlap: usize) -> Self {
        self.overlap = overlap;
        self
    }

    /// Sets custom separators.
    pub fn with_separators(mut self, separators: Vec<String>) -> Self {
        self.separators = separators;
        self
    }
}

/// Semantic/similarity-based chunking configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticChunkingConfig {
    /// Similarity threshold for splitting (0.0-1.0).
    /// Lower values = more aggressive splitting.
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: f32,
    /// Minimum chunk size in characters.
    #[serde(default = "default_min_chunk_size")]
    pub min_size: usize,
    /// Maximum chunk size in characters.
    #[serde(default = "default_max_chunk_size")]
    pub max_size: usize,
    /// Embedding model to use for similarity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_model: Option<String>,
}

impl Default for SemanticChunkingConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: default_similarity_threshold(),
            min_size: default_min_chunk_size(),
            max_size: default_max_chunk_size(),
            embedding_model: None,
        }
    }
}

/// Contextual chunking using LLM to determine boundaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextualChunkingConfig {
    /// LLM model to use for boundary detection.
    pub model: String,
    /// Maximum chunk size in characters.
    #[serde(default = "default_max_chunk_size")]
    pub max_size: usize,
    /// Custom prompt for boundary detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_prompt: Option<String>,
}

impl ContextualChunkingConfig {
    /// Creates a new contextual chunking config.
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            max_size: default_max_chunk_size(),
            custom_prompt: None,
        }
    }
}

// Default value functions

fn default_true() -> bool {
    true
}

fn default_one() -> usize {
    1
}

fn default_heading_levels() -> Vec<u8> {
    vec![1, 2, 3]
}

fn default_recursive_separators() -> Vec<String> {
    vec![
        "\n\n".to_string(), // Paragraphs
        "\n".to_string(),   // Lines
        ". ".to_string(),   // Sentences
        ", ".to_string(),   // Clauses
        " ".to_string(),    // Words
    ]
}

fn default_similarity_threshold() -> f32 {
    0.5
}

fn default_min_chunk_size() -> usize {
    100
}

fn default_max_chunk_size() -> usize {
    1000
}
