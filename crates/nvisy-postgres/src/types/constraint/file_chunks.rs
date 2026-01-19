//! File chunks table constraint violations.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use super::ConstraintCategory;

/// File chunks table constraint violations.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, Display, EnumIter, EnumString)]
#[serde(into = "String", try_from = "String")]
pub enum FileChunkConstraints {
    // Chunk position constraints
    #[strum(serialize = "file_chunks_chunk_index_min")]
    ChunkIndexMin,

    // Content constraints
    #[strum(serialize = "file_chunks_content_sha256_length")]
    ContentSha256Length,
    #[strum(serialize = "file_chunks_content_size_min")]
    ContentSizeMin,
    #[strum(serialize = "file_chunks_token_count_min")]
    TokenCountMin,

    // Embedding constraints
    #[strum(serialize = "file_chunks_embedding_model_format")]
    EmbeddingModelFormat,

    // Metadata constraints
    #[strum(serialize = "file_chunks_metadata_size")]
    MetadataSize,

    // Chronological constraints
    #[strum(serialize = "file_chunks_updated_after_created")]
    UpdatedAfterCreated,

    // Uniqueness constraints
    #[strum(serialize = "file_chunks_file_chunk_unique")]
    FileChunkUnique,
}

impl FileChunkConstraints {
    /// Creates a new [`FileChunkConstraints`] from the constraint name.
    pub fn new(constraint: &str) -> Option<Self> {
        constraint.parse().ok()
    }

    /// Returns the category of this constraint violation.
    pub fn categorize(&self) -> ConstraintCategory {
        match self {
            FileChunkConstraints::ChunkIndexMin
            | FileChunkConstraints::ContentSha256Length
            | FileChunkConstraints::ContentSizeMin
            | FileChunkConstraints::TokenCountMin
            | FileChunkConstraints::EmbeddingModelFormat
            | FileChunkConstraints::MetadataSize => ConstraintCategory::Validation,

            FileChunkConstraints::UpdatedAfterCreated => ConstraintCategory::Chronological,

            FileChunkConstraints::FileChunkUnique => ConstraintCategory::Uniqueness,
        }
    }
}

impl From<FileChunkConstraints> for String {
    #[inline]
    fn from(val: FileChunkConstraints) -> Self {
        val.to_string()
    }
}

impl TryFrom<String> for FileChunkConstraints {
    type Error = strum::ParseError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
