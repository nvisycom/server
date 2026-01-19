//! Indexed chunk result type.

use nvisy_postgres::model::FileChunk;
use uuid::Uuid;

/// Result of indexing a single chunk.
#[derive(Debug, Clone)]
pub struct IndexedChunk {
    /// Database ID of the created chunk.
    pub id: Uuid,
    /// Index of the chunk within the file.
    pub chunk_index: i32,
    /// Size of the chunk content in bytes.
    pub content_size: i32,
    /// Number of tokens in the chunk.
    pub token_count: i32,
}

impl From<FileChunk> for IndexedChunk {
    fn from(chunk: FileChunk) -> Self {
        Self {
            id: chunk.id,
            chunk_index: chunk.chunk_index,
            content_size: chunk.content_size,
            token_count: chunk.token_count,
        }
    }
}
