//! Document chunks table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// Document chunks table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum DocumentChunkConstraints {
    // Chunk position constraints
    #[strum(serialize = "document_chunks_chunk_index_min")]
    ChunkIndexMin,

    // Content constraints
    #[strum(serialize = "document_chunks_content_sha256_length")]
    ContentSha256Length,
    #[strum(serialize = "document_chunks_content_size_min")]
    ContentSizeMin,
    #[strum(serialize = "document_chunks_token_count_min")]
    TokenCountMin,

    // Embedding constraints
    #[strum(serialize = "document_chunks_embedding_model_format")]
    EmbeddingModelFormat,

    // Metadata constraints
    #[strum(serialize = "document_chunks_metadata_size")]
    MetadataSize,

    // Chronological constraints
    #[strum(serialize = "document_chunks_updated_after_created")]
    UpdatedAfterCreated,

    // Uniqueness constraints
    #[strum(serialize = "document_chunks_file_chunk_unique")]
    FileChunkUnique,
}

impl DocumentChunkConstraints {
    /// Creates a new [`DocumentChunkConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            DocumentChunkConstraints::ChunkIndexMin
            | DocumentChunkConstraints::ContentSha256Length
            | DocumentChunkConstraints::ContentSizeMin
            | DocumentChunkConstraints::TokenCountMin
            | DocumentChunkConstraints::EmbeddingModelFormat
            | DocumentChunkConstraints::MetadataSize => ConstraintCategory::Validation,

            DocumentChunkConstraints::UpdatedAfterCreated => ConstraintCategory::Chronological,

            DocumentChunkConstraints::FileChunkUnique => ConstraintCategory::Uniqueness,
        }
    }
}

impl From<DocumentChunkConstraints> for String {
    #[inline]
    fn from(val: DocumentChunkConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for DocumentChunkConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
