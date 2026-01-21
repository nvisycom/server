//! Document chunk indexing pipeline.

mod indexed;

use nvisy_postgres::model::NewFileChunk;
use nvisy_postgres::query::FileChunkRepository;
use nvisy_postgres::{PgClient, Vector};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub use self::indexed::IndexedChunk;
use crate::provider::{EmbeddingProvider, OwnedChunk, TextSplitter};
use crate::{Error, Result};

/// Indexer for batch-embedding and storing document chunks.
pub struct Indexer {
    provider: EmbeddingProvider,
    db: PgClient,
    splitter: TextSplitter,
    file_id: Uuid,
}

impl Indexer {
    /// Creates a new indexer for the given file.
    pub(crate) fn new(
        provider: EmbeddingProvider,
        db: PgClient,
        splitter: TextSplitter,
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
    #[tracing::instrument(skip(self, text), fields(file_id = %self.file_id, text_len = text.len()))]
    pub async fn index(&self, text: &str) -> Result<Vec<IndexedChunk>> {
        let chunks = self.splitter.split_owned(text);
        self.index_chunks(chunks).await
    }

    /// Indexes text with page awareness.
    #[tracing::instrument(skip(self, text), fields(file_id = %self.file_id, text_len = text.len()))]
    pub async fn index_with_pages(&self, text: &str) -> Result<Vec<IndexedChunk>> {
        let chunks = self.splitter.split_with_pages_owned(text);
        self.index_chunks(chunks).await
    }

    /// Deletes all existing chunks for the file before indexing.
    #[tracing::instrument(skip(self, text), fields(file_id = %self.file_id, text_len = text.len()))]
    pub async fn reindex(&self, text: &str) -> Result<Vec<IndexedChunk>> {
        let chunks = self.splitter.split_owned(text);
        self.reindex_chunks(chunks).await
    }

    /// Deletes all existing chunks for the file before indexing with page awareness.
    #[tracing::instrument(skip(self, text), fields(file_id = %self.file_id, text_len = text.len()))]
    pub async fn reindex_with_pages(&self, text: &str) -> Result<Vec<IndexedChunk>> {
        let chunks = self.splitter.split_with_pages_owned(text);
        self.reindex_chunks(chunks).await
    }

    async fn index_chunks(&self, chunks: Vec<OwnedChunk>) -> Result<Vec<IndexedChunk>> {
        if chunks.is_empty() {
            tracing::debug!("no chunks to index");
            return Ok(vec![]);
        }

        let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
        let chunk_count = texts.len();

        tracing::debug!(chunk_count, "embedding chunks");
        let embeddings = self.provider.embed_texts(texts).await?;

        if embeddings.len() != chunk_count {
            return Err(Error::config(format!(
                "embedding count mismatch: expected {}, got {}",
                chunk_count,
                embeddings.len()
            )));
        }

        let model_name = self.provider.model_name();

        let new_chunks: Vec<NewFileChunk> = chunks
            .iter()
            .zip(embeddings.iter())
            .enumerate()
            .map(|(idx, (chunk, embedding))| {
                let content_bytes = chunk.text.as_bytes();
                let content_sha256 = Sha256::digest(content_bytes).to_vec();
                let content_size = content_bytes.len() as i32;

                let embedding_vec: Vec<f32> = embedding.vec.iter().map(|&x| x as f32).collect();

                let metadata = serde_json::json!({
                    "index": chunk.metadata.index,
                    "start_offset": chunk.metadata.start_offset,
                    "end_offset": chunk.metadata.end_offset,
                    "page": chunk.metadata.page,
                });

                NewFileChunk {
                    file_id: self.file_id,
                    chunk_index: Some(idx as i32),
                    content_sha256,
                    content_size: Some(content_size),
                    token_count: None,
                    embedding: Vector::from(embedding_vec),
                    embedding_model: model_name.to_owned(),
                    metadata: Some(metadata),
                }
            })
            .collect();

        let mut conn = self
            .db
            .get_connection()
            .await
            .map_err(|e| Error::retrieval(format!("failed to get connection: {e}")))?;

        let created = conn
            .create_file_chunks(new_chunks)
            .await
            .map_err(|e| Error::retrieval(format!("failed to create chunks: {e}")))?;

        tracing::debug!(created_count = created.len(), "stored chunks");
        Ok(created.into_iter().map(IndexedChunk::from).collect())
    }

    async fn reindex_chunks(&self, chunks: Vec<OwnedChunk>) -> Result<Vec<IndexedChunk>> {
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
            tracing::debug!(deleted, "deleted existing chunks");
        }

        drop(conn);
        self.index_chunks(chunks).await
    }
}
