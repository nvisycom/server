//! Document chunk indexing pipeline.
//!
//! Provides batch embedding and storage of document chunks using pgvector.

mod indexed;

use nvisy_postgres::model::NewFileChunk;
use nvisy_postgres::query::FileChunkRepository;
use nvisy_postgres::{PgClient, Vector};

use sha2::{Digest, Sha256};
use uuid::Uuid;

pub use self::indexed::IndexedChunk;
use super::splitter::{OwnedSplitChunk, Splitter, estimate_tokens};
use crate::provider::EmbeddingProvider;
use crate::{Error, Result};

/// Indexer for batch-embedding and storing document chunks.
///
/// Handles text splitting, embedding, and storage in PostgreSQL.
pub struct Indexer {
    provider: EmbeddingProvider,
    db: PgClient,
    splitter: Splitter,
    file_id: Uuid,
}

impl Indexer {
    /// Creates a new indexer for the given file.
    pub(crate) fn new(
        provider: EmbeddingProvider,
        db: PgClient,
        splitter: Splitter,
        file_id: Uuid,
    ) -> Self {
        Self {
            provider,
            db,
            splitter,
            file_id,
        }
    }

    /// Returns the file ID.
    pub fn file_id(&self) -> Uuid {
        self.file_id
    }

    /// Indexes text by splitting, embedding, and storing chunks.
    pub async fn index(&self, text: &str) -> Result<Vec<IndexedChunk>> {
        let chunks = self.splitter.split_owned(text);
        self.index_chunks(chunks).await
    }

    /// Indexes text with page awareness.
    ///
    /// Page breaks should be indicated by form feed characters (`\x0c`).
    pub async fn index_with_pages(&self, text: &str) -> Result<Vec<IndexedChunk>> {
        let chunks = self.splitter.split_with_pages_owned(text);
        self.index_chunks(chunks).await
    }

    /// Deletes all existing chunks for the file before indexing.
    pub async fn reindex(&self, text: &str) -> Result<Vec<IndexedChunk>> {
        let chunks = self.splitter.split_owned(text);
        self.reindex_chunks(chunks).await
    }

    /// Deletes all existing chunks for the file before indexing with page awareness.
    pub async fn reindex_with_pages(&self, text: &str) -> Result<Vec<IndexedChunk>> {
        let chunks = self.splitter.split_with_pages_owned(text);
        self.reindex_chunks(chunks).await
    }

    async fn index_chunks(&self, chunks: Vec<OwnedSplitChunk>) -> Result<Vec<IndexedChunk>> {
        if chunks.is_empty() {
            return Ok(vec![]);
        }

        // Extract texts for embedding
        let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();

        // Batch embed all texts
        let embeddings = self
            .provider
            .embed_texts(texts)
            .await
            .map_err(|e| Error::embedding(format!("failed to embed chunks: {e}")))?;

        if embeddings.len() != chunks.len() {
            return Err(Error::embedding(format!(
                "embedding count mismatch: expected {}, got {}",
                chunks.len(),
                embeddings.len()
            )));
        }

        // Prepare new chunk records
        let model_name = self.provider.model_name();

        let new_chunks: Vec<NewFileChunk> = chunks
            .iter()
            .zip(embeddings.iter())
            .enumerate()
            .map(|(idx, (chunk, embedding))| {
                let content_bytes = chunk.text.as_bytes();
                let content_sha256 = Sha256::digest(content_bytes).to_vec();
                let content_size = content_bytes.len() as i32;

                // Convert f64 embeddings to f32 for pgvector
                let embedding_vec: Vec<f32> = embedding.vec.iter().map(|&x| x as f32).collect();

                let metadata = serde_json::json!({
                    "start_offset": chunk.metadata.start_offset,
                    "end_offset": chunk.metadata.end_offset,
                    "page": chunk.metadata.page,
                });

                NewFileChunk {
                    file_id: self.file_id,
                    chunk_index: Some(idx as i32),
                    content_sha256,
                    content_size: Some(content_size),
                    token_count: Some(estimate_tokens(&chunk.text) as i32),
                    embedding: Vector::from(embedding_vec),
                    embedding_model: model_name.to_owned(),
                    metadata: Some(metadata),
                }
            })
            .collect();

        // Store in database
        let mut conn = self
            .db
            .get_connection()
            .await
            .map_err(|e| Error::retrieval(format!("failed to get connection: {e}")))?;

        let created = conn
            .create_file_chunks(new_chunks)
            .await
            .map_err(|e| Error::retrieval(format!("failed to create chunks: {e}")))?;

        Ok(created.into_iter().map(IndexedChunk::from).collect())
    }

    async fn reindex_chunks(&self, chunks: Vec<OwnedSplitChunk>) -> Result<Vec<IndexedChunk>> {
        // Delete existing chunks first
        let mut conn = self
            .db
            .get_connection()
            .await
            .map_err(|e| Error::retrieval(format!("failed to get connection: {e}")))?;

        let deleted = conn
            .delete_file_chunks(self.file_id)
            .await
            .map_err(|e| Error::retrieval(format!("failed to delete chunks: {e}")))?;

        if deleted > 0 {
            tracing::debug!(file_id = %self.file_id, deleted, "Deleted existing chunks");
        }

        drop(conn);

        self.index_chunks(chunks).await
    }
}
