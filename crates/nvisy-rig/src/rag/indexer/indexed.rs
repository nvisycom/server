//! Indexed chunk result type.

use nvisy_postgres::model::DocumentChunk;
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

impl From<DocumentChunk> for IndexedChunk {
    fn from(chunk: DocumentChunk) -> Self {
        Self {
            id: chunk.id,
            chunk_index: chunk.chunk_index,
            content_size: chunk.content_size,
            token_count: chunk.token_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_chunk_fields() {
        let chunk = IndexedChunk {
            id: Uuid::nil(),
            chunk_index: 0,
            content_size: 100,
            token_count: 25,
        };

        assert_eq!(chunk.chunk_index, 0);
        assert_eq!(chunk.content_size, 100);
        assert_eq!(chunk.token_count, 25);
    }
}
