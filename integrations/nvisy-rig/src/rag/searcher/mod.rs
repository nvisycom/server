//! Semantic search over document chunks.
//!
//! Provides vector similarity search via pgvector with content retrieval from NATS.

mod retrieved;
mod scope;

use std::collections::HashMap;

use nvisy_nats::object::{FileKey, FilesBucket, ObjectStore};
use nvisy_postgres::model::ScoredFileChunk;
use nvisy_postgres::query::FileChunkRepository;
use nvisy_postgres::{PgClient, Vector};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

pub use self::retrieved::{ChunkMetadata, RetrievedChunk};
pub use self::scope::SearchScope;
use crate::provider::EmbeddingProvider;
use crate::{Error, Result};

/// Semantic search service for document chunks.
///
/// Provides vector similarity search with optional content retrieval.
pub struct Searcher {
    provider: EmbeddingProvider,
    db: PgClient,
    files: ObjectStore<FilesBucket, FileKey>,
    scope: SearchScope,
    min_score: Option<f64>,
}

impl Searcher {
    /// Creates a new search service.
    pub(crate) fn new(
        provider: EmbeddingProvider,
        db: PgClient,
        files: ObjectStore<FilesBucket, FileKey>,
        scope: SearchScope,
    ) -> Self {
        Self {
            provider,
            db,
            files,
            scope,
            min_score: None,
        }
    }

    /// Sets the minimum similarity score threshold.
    pub fn with_min_score(mut self, min_score: f64) -> Self {
        self.min_score = Some(min_score);
        self
    }

    /// Returns the search scope.
    pub fn scope(&self) -> &SearchScope {
        &self.scope
    }

    /// Searches for relevant chunks without loading content.
    pub async fn query(&self, query: &str, limit: u32) -> Result<Vec<RetrievedChunk>> {
        let embedding = self
            .provider
            .embed_text(query)
            .await
            .map_err(|e| Error::embedding(format!("failed to embed query: {e}")))?;

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
            .map_err(|e| Error::retrieval(format!("failed to get connection: {e}")))?;

        let min_score = self.min_score.unwrap_or(0.0);

        let scored_chunks: Vec<ScoredFileChunk> = match &self.scope {
            SearchScope::Files(file_ids) => {
                conn.search_scored_chunks_in_files(query_vector, file_ids, min_score, limit as i64)
                    .await
            }
            SearchScope::Workspace(workspace_id) => {
                conn.search_scored_chunks_in_workspace(
                    query_vector,
                    *workspace_id,
                    min_score,
                    limit as i64,
                )
                .await
            }
        }
        .map_err(|e| Error::retrieval(format!("vector search failed: {e}")))?;

        let chunks = scored_chunks
            .into_iter()
            .map(|scored| {
                let chunk = scored.chunk;
                let metadata = ChunkMetadata::from_json(&chunk.metadata, chunk.chunk_index);
                RetrievedChunk::new(chunk.id, chunk.file_id, scored.score, metadata)
            })
            .collect();

        Ok(chunks)
    }

    /// Searches for relevant chunks and loads their content.
    pub async fn query_with_content(&self, query: &str, limit: u32) -> Result<Vec<RetrievedChunk>> {
        let mut chunks = self.query(query, limit).await?;
        self.load_content(&mut chunks).await?;
        Ok(chunks)
    }

    /// Loads content for retrieved chunks from NATS.
    pub async fn load_content(&self, chunks: &mut [RetrievedChunk]) -> Result<()> {
        let mut by_file: HashMap<Uuid, Vec<usize>> = HashMap::new();
        for (idx, chunk) in chunks.iter().enumerate() {
            if chunk.content.is_none() {
                by_file.entry(chunk.file_id).or_default().push(idx);
            }
        }

        for (file_id, indices) in by_file {
            let file_content = match self.fetch_file(file_id).await {
                Ok(content) => content,
                Err(e) => {
                    tracing::warn!(file_id = %file_id, error = %e, "Failed to fetch file");
                    continue;
                }
            };

            for idx in indices {
                let chunk = &mut chunks[idx];
                let range = chunk.metadata.byte_range();

                if range.end <= file_content.len() {
                    let text = String::from_utf8_lossy(&file_content[range]).into_owned();
                    chunk.content = Some(text);
                }
            }
        }

        Ok(())
    }

    async fn fetch_file(&self, file_id: Uuid) -> Result<Vec<u8>> {
        let key = FileKey::from_parts(Uuid::nil(), file_id);

        let mut result = self
            .files
            .get(&key)
            .await
            .map_err(|e| Error::retrieval(format!("failed to get file: {e}")))?
            .ok_or_else(|| Error::retrieval(format!("file not found: {file_id}")))?;

        let mut content = Vec::with_capacity(result.size());
        result
            .reader()
            .read_to_end(&mut content)
            .await
            .map_err(|e| Error::retrieval(format!("failed to read file: {e}")))?;

        Ok(content)
    }
}
