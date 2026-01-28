//! Workspace file chunk model for PostgreSQL database operations.

use diesel::prelude::*;
use jiff_diesel::Timestamp;
use pgvector::Vector;
use uuid::Uuid;

use crate::schema::file_chunks;
use crate::types::{HasCreatedAt, HasUpdatedAt};

/// Workspace file chunk model representing a text segment from a file.
///
/// Chunks are used for semantic search via vector embeddings. Each chunk
/// represents a portion of a file with its embedding vector for
/// similarity search.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = file_chunks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorkspaceFileChunk {
    /// Unique chunk identifier.
    pub id: Uuid,
    /// Reference to the file this chunk belongs to.
    pub file_id: Uuid,
    /// Zero-based index of this chunk within the file.
    pub chunk_index: i32,
    /// SHA-256 hash of the chunk content.
    pub content_sha256: Vec<u8>,
    /// Size of the chunk content in bytes.
    pub content_size: i32,
    /// Number of tokens in the chunk.
    pub token_count: i32,
    /// Vector embedding for semantic search (1536 dimensions for OpenAI ada-002).
    pub embedding: Vector,
    /// Model used to generate the embedding.
    pub embedding_model: String,
    /// Additional metadata (JSON).
    pub metadata: serde_json::Value,
    /// Timestamp when the chunk was created.
    pub created_at: Timestamp,
    /// Timestamp when the chunk was last updated.
    pub updated_at: Timestamp,
}

/// Data for creating a new workspace file chunk.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = file_chunks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewWorkspaceFileChunk {
    /// File ID (required).
    pub file_id: Uuid,
    /// Chunk index within the file.
    pub chunk_index: Option<i32>,
    /// SHA-256 hash of the chunk content.
    pub content_sha256: Vec<u8>,
    /// Size of the chunk content in bytes.
    pub content_size: Option<i32>,
    /// Token count.
    pub token_count: Option<i32>,
    /// Vector embedding (required).
    pub embedding: Vector,
    /// Embedding model name (required).
    pub embedding_model: String,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
}

/// Data for updating a workspace file chunk.
#[derive(Debug, Clone, Default, AsChangeset)]
#[diesel(table_name = file_chunks)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateWorkspaceFileChunk {
    /// Token count.
    pub token_count: Option<i32>,
    /// Vector embedding.
    pub embedding: Option<Vector>,
    /// Embedding model name.
    pub embedding_model: Option<String>,
    /// Metadata.
    pub metadata: Option<serde_json::Value>,
}

impl WorkspaceFileChunk {
    /// Returns whether the chunk has custom metadata.
    pub fn has_metadata(&self) -> bool {
        !self.metadata.as_object().is_none_or(|obj| obj.is_empty())
    }

    /// Returns the embedding dimensions.
    pub fn embedding_dimensions(&self) -> usize {
        self.embedding.as_slice().len()
    }
}

impl HasCreatedAt for WorkspaceFileChunk {
    fn created_at(&self) -> jiff::Timestamp {
        self.created_at.into()
    }
}

impl HasUpdatedAt for WorkspaceFileChunk {
    fn updated_at(&self) -> jiff::Timestamp {
        self.updated_at.into()
    }
}

/// A workspace file chunk with its similarity score.
///
/// Returned from similarity search queries.
#[derive(Debug, Clone)]
pub struct ScoredWorkspaceFileChunk {
    /// The file chunk.
    pub chunk: WorkspaceFileChunk,
    /// Similarity score (0.0 to 1.0, higher is more similar).
    pub score: f64,
}

impl ScoredWorkspaceFileChunk {
    /// Returns a reference to the chunk.
    pub fn chunk(&self) -> &WorkspaceFileChunk {
        &self.chunk
    }

    /// Returns the similarity score.
    pub fn score(&self) -> f64 {
        self.score
    }

    /// Consumes self and returns the inner chunk.
    pub fn into_chunk(self) -> WorkspaceFileChunk {
        self.chunk
    }
}
