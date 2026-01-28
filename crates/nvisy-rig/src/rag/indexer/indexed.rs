//! Indexed chunk result type.

use nvisy_postgres::model::WorkspaceFileChunk;
use uuid::Uuid;

/// Result of indexing a single chunk.
#[derive(Debug, Clone)]
pub struct IndexedChunk {
    /// Database ID of the created chunk.
    pub id: Uuid,
    /// Index of the chunk within the file (0-based).
    pub index: u32,
    /// Size of the chunk content in bytes.
    pub content_size: u32,
    /// Number of tokens in the chunk.
    pub token_count: u32,
}

impl From<WorkspaceFileChunk> for IndexedChunk {
    fn from(chunk: WorkspaceFileChunk) -> Self {
        Self {
            id: chunk.id,
            index: chunk.chunk_index as u32,
            content_size: chunk.content_size as u32,
            token_count: chunk.token_count as u32,
        }
    }
}
