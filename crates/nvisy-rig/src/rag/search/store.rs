//! Vector store implementation using pgvector.

use nvisy_postgres::model::DocumentChunk;
use nvisy_postgres::query::DocumentChunkRepository;
use nvisy_postgres::{PgClient, Vector};
use rig::embeddings::EmbeddingModel;
use rig::vector_store::{VectorStoreError, VectorStoreIndex};
use serde::Deserialize;
use uuid::Uuid;

use super::retrieved::ChunkMetadata;
use super::scope::SearchScope;
use crate::service::provider::EmbeddingProvider;

/// Vector store for document chunks using pgvector.
///
/// Implements rig's `VectorStoreIndex` trait for semantic search.
///
/// # Security
///
/// Search is always scoped to specific files or documents.
#[derive(Clone)]
pub struct ChunkVectorStore {
    provider: EmbeddingProvider,
    db: PgClient,
    scope: SearchScope,
}

impl ChunkVectorStore {
    /// Creates a new vector store with the given scope.
    pub fn new(provider: EmbeddingProvider, db: PgClient, scope: SearchScope) -> Self {
        Self {
            provider,
            db,
            scope,
        }
    }

    /// Returns a reference to the embedding provider.
    pub fn provider(&self) -> &EmbeddingProvider {
        &self.provider
    }

    /// Returns a reference to the database client.
    pub fn db(&self) -> &PgClient {
        &self.db
    }

    /// Returns the search scope.
    pub fn scope(&self) -> &SearchScope {
        &self.scope
    }
}

impl VectorStoreIndex for ChunkVectorStore {
    async fn top_n<T: for<'a> Deserialize<'a> + Send>(
        &self,
        query: &str,
        n: usize,
    ) -> Result<Vec<(f64, String, T)>, VectorStoreError> {
        let embedding = self
            .provider
            .embed_text(query)
            .await
            .map_err(VectorStoreError::EmbeddingError)?;

        let query_vector: Vector = embedding
            .vec
            .iter()
            .map(|&x| x as f32)
            .collect::<Vec<_>>()
            .into();

        let mut conn = self
            .db
            .get_connection()
            .await
            .map_err(|e| VectorStoreError::DatastoreError(e.into()))?;

        let chunks: Vec<DocumentChunk> = match &self.scope {
            SearchScope::Files(file_ids) => {
                conn.search_similar_document_chunks_in_files(query_vector, file_ids, n as i64)
                    .await
            }
            SearchScope::Documents(doc_ids) => {
                conn.search_similar_document_chunks_in_documents(query_vector, doc_ids, n as i64)
                    .await
            }
        }
        .map_err(|e| VectorStoreError::DatastoreError(e.into()))?;

        chunks
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| {
                let distance = i as f64 * 0.1;
                let id = chunk.id.to_string();

                let doc: T = serde_json::from_value(serde_json::to_value(&ChunkResult {
                    id: chunk.id,
                    file_id: chunk.file_id,
                    chunk_index: chunk.chunk_index,
                    content_size: chunk.content_size,
                    token_count: chunk.token_count,
                    metadata: chunk.metadata,
                })?)
                .map_err(VectorStoreError::JsonError)?;

                Ok((distance, id, doc))
            })
            .collect()
    }

    async fn top_n_ids(
        &self,
        query: &str,
        n: usize,
    ) -> Result<Vec<(f64, String)>, VectorStoreError> {
        let embedding = self
            .provider
            .embed_text(query)
            .await
            .map_err(VectorStoreError::EmbeddingError)?;

        let query_vector: Vector = embedding
            .vec
            .iter()
            .map(|&x| x as f32)
            .collect::<Vec<_>>()
            .into();

        let mut conn = self
            .db
            .get_connection()
            .await
            .map_err(|e| VectorStoreError::DatastoreError(e.into()))?;

        let chunks: Vec<DocumentChunk> = match &self.scope {
            SearchScope::Files(file_ids) => {
                conn.search_similar_document_chunks_in_files(query_vector, file_ids, n as i64)
                    .await
            }
            SearchScope::Documents(doc_ids) => {
                conn.search_similar_document_chunks_in_documents(query_vector, doc_ids, n as i64)
                    .await
            }
        }
        .map_err(|e| VectorStoreError::DatastoreError(e.into()))?;

        Ok(chunks
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| (i as f64 * 0.1, chunk.id.to_string()))
            .collect())
    }
}

/// Serializable chunk result for rig's VectorStoreIndex.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChunkResult {
    pub id: Uuid,
    pub file_id: Uuid,
    pub chunk_index: i32,
    pub content_size: i32,
    pub token_count: i32,
    pub metadata: serde_json::Value,
}

impl ChunkResult {
    /// Extracts chunk metadata from the JSON metadata field.
    pub fn chunk_metadata(&self) -> ChunkMetadata {
        ChunkMetadata::from_json(&self.metadata, self.chunk_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_result_metadata_extraction() {
        let result = ChunkResult {
            id: Uuid::nil(),
            file_id: Uuid::nil(),
            chunk_index: 2,
            content_size: 500,
            token_count: 100,
            metadata: serde_json::json!({
                "start_offset": 1000,
                "end_offset": 1500,
                "page": 5,
                "section": "Introduction"
            }),
        };

        let meta = result.chunk_metadata();
        assert_eq!(meta.start_offset, 1000);
        assert_eq!(meta.end_offset, 1500);
        assert_eq!(meta.page, Some(5));
        assert_eq!(meta.section, Some("Introduction".to_string()));
        assert_eq!(meta.chunk_index, 2);
    }

    #[test]
    fn chunk_result_metadata_defaults() {
        let result = ChunkResult {
            id: Uuid::nil(),
            file_id: Uuid::nil(),
            chunk_index: 0,
            content_size: 500,
            token_count: 100,
            metadata: serde_json::json!({}),
        };

        let meta = result.chunk_metadata();
        assert_eq!(meta.start_offset, 0);
        assert_eq!(meta.end_offset, 0);
        assert_eq!(meta.page, None);
        assert_eq!(meta.section, None);
    }
}
