//! Document chunk indexing pipeline.
//!
//! Provides batch embedding and storage of document chunks using pgvector.

mod indexed;

use nvisy_postgres::model::NewDocumentChunk;
use nvisy_postgres::query::DocumentChunkRepository;
use nvisy_postgres::{PgClient, Vector};
use rig::embeddings::EmbeddingModel;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub use self::indexed::IndexedChunk;
use super::OwnedSplitChunk;
use crate::Result;
use crate::service::provider::EmbeddingProvider;

/// Indexer for batch-embedding and storing document chunks.
///
/// Uses batched embedding requests to the model provider for efficiency,
/// then stores the chunks with their embeddings in PostgreSQL.
///
/// # Example
///
/// ```ignore
/// let indexer = Indexer::new(provider, db, file_id);
/// let indexed = indexer.index_chunks(chunks).await?;
/// println!("Indexed {} chunks", indexed.len());
/// ```
pub struct Indexer {
    provider: EmbeddingProvider,
    db: PgClient,
    file_id: Uuid,
    embedding_model_name: Option<String>,
}

impl Indexer {
    /// Creates a new indexer for the given file.
    pub fn new(provider: EmbeddingProvider, db: PgClient, file_id: Uuid) -> Self {
        Self {
            provider,
            db,
            file_id,
            embedding_model_name: None,
        }
    }

    /// Sets the embedding model name to store in metadata.
    pub fn with_model_name(mut self, name: impl Into<String>) -> Self {
        self.embedding_model_name = Some(name.into());
        self
    }

    /// Indexes chunks by embedding and storing them in the database.
    ///
    /// This method:
    /// 1. Extracts text from all chunks
    /// 2. Batch-embeds them using the provider's `embed_texts` method
    /// 3. Creates database records with embeddings
    ///
    /// Returns the indexed chunk metadata.
    pub async fn index_chunks(self, chunks: Vec<OwnedSplitChunk>) -> Result<Vec<IndexedChunk>> {
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
            .map_err(|e| crate::Error::embedding(format!("failed to embed chunks: {e}")))?;

        if embeddings.len() != chunks.len() {
            return Err(crate::Error::embedding(format!(
                "embedding count mismatch: expected {}, got {}",
                chunks.len(),
                embeddings.len()
            )));
        }

        // Prepare new chunk records
        let model_name = self
            .embedding_model_name
            .unwrap_or_else(|| "unknown".to_string());

        let new_chunks: Vec<NewDocumentChunk> = chunks
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

                NewDocumentChunk {
                    file_id: self.file_id,
                    chunk_index: Some(idx as i32),
                    content_sha256,
                    content_size: Some(content_size),
                    token_count: None, // Could be computed if needed
                    embedding: Vector::from(embedding_vec),
                    embedding_model: Some(model_name.clone()),
                    metadata: Some(metadata),
                }
            })
            .collect();

        // Store in database
        let mut conn = self
            .db
            .get_connection()
            .await
            .map_err(|e| crate::Error::retrieval(format!("failed to get connection: {e}")))?;

        let created = conn
            .create_document_chunks(new_chunks)
            .await
            .map_err(|e| crate::Error::retrieval(format!("failed to create chunks: {e}")))?;

        // Return indexed chunk metadata
        let indexed = created.into_iter().map(IndexedChunk::from).collect();

        Ok(indexed)
    }

    /// Indexes a single text by splitting, embedding, and storing chunks.
    ///
    /// Convenience method that combines splitting and indexing.
    pub async fn index_text(
        self,
        text: &str,
        splitter: &super::TextSplitterService,
    ) -> Result<Vec<IndexedChunk>> {
        let chunks = splitter.split_owned(text);
        self.index_chunks(chunks).await
    }

    /// Deletes all existing chunks for the file before indexing.
    ///
    /// Use this for re-indexing a file that may have changed.
    pub async fn reindex_chunks(self, chunks: Vec<OwnedSplitChunk>) -> Result<Vec<IndexedChunk>> {
        // Delete existing chunks first
        let mut conn = self
            .db
            .get_connection()
            .await
            .map_err(|e| crate::Error::retrieval(format!("failed to get connection: {e}")))?;

        let deleted = conn
            .delete_document_file_chunks(self.file_id)
            .await
            .map_err(|e| crate::Error::retrieval(format!("failed to delete chunks: {e}")))?;

        if deleted > 0 {
            tracing::debug!(file_id = %self.file_id, deleted, "Deleted existing chunks");
        }

        drop(conn);

        // Now index the new chunks
        self.index_chunks(chunks).await
    }
}
