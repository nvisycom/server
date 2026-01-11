//! Semantic search over document chunks.
//!
//! Provides vector similarity search via pgvector with content retrieval from NATS.

mod retrieved;
mod scope;
mod store;

use std::collections::HashMap;

use nvisy_nats::object::{DocumentKey, DocumentStore, Files};
use nvisy_postgres::model::ScoredDocumentChunk;
use nvisy_postgres::query::DocumentChunkRepository;
use nvisy_postgres::{PgClient, Vector};
use rig::embeddings::EmbeddingModel;
use tokio::io::AsyncReadExt;
use uuid::Uuid;

pub use self::retrieved::{ChunkMetadata, RetrievedChunk};
pub use self::scope::SearchScope;
pub use self::store::{ChunkResult, ChunkVectorStore};
use crate::{Error, Result};
use crate::service::provider::EmbeddingProvider;

/// Semantic search service for document chunks.
///
/// Provides vector similarity search with optional content retrieval.
/// The service is cheap to clone and can be shared across threads.
#[derive(Clone)]
pub struct SearchService {
    provider: EmbeddingProvider,
    db: PgClient,
    files: DocumentStore<Files>,
    min_score: f64,
}

impl SearchService {
    /// Creates a new search service.
    pub fn new(provider: EmbeddingProvider, db: PgClient, files: DocumentStore<Files>) -> Self {
        Self {
            provider,
            db,
            files,
            min_score: 0.7,
        }
    }

    /// Sets the minimum similarity score threshold.
    pub fn with_min_score(mut self, min_score: f64) -> Self {
        self.min_score = min_score;
        self
    }

    /// Searches for relevant chunks without loading content.
    pub async fn search(
        &self,
        scope: SearchScope,
        query: &str,
        limit: u32,
    ) -> Result<Vec<RetrievedChunk>> {
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

        let scored_chunks: Vec<ScoredDocumentChunk> = match &scope {
            SearchScope::Files(file_ids) => {
                conn.search_scored_chunks_in_files(
                    query_vector,
                    file_ids,
                    self.min_score,
                    limit as i64,
                )
                .await
            }
            SearchScope::Documents(doc_ids) => {
                conn.search_scored_chunks_in_documents(
                    query_vector,
                    doc_ids,
                    self.min_score,
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
    pub async fn search_with_content(
        &self,
        scope: SearchScope,
        query: &str,
        limit: u32,
    ) -> Result<Vec<RetrievedChunk>> {
        let mut chunks = self.search(scope, query, limit).await?;
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
        let key = DocumentKey::from_parts(Uuid::nil(), file_id);

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

    /// Creates a scoped service for searching within specific files or documents.
    pub fn scoped(&self, scope: SearchScope) -> ScopedSearchService<'_> {
        ScopedSearchService {
            service: self,
            scope,
        }
    }
}

/// A search service scoped to specific files or documents.
///
/// Created via [`SearchService::scoped`]. Provides the same search methods
/// but with the scope pre-configured.
pub struct ScopedSearchService<'a> {
    service: &'a SearchService,
    scope: SearchScope,
}

impl ScopedSearchService<'_> {
    /// Returns the search scope.
    pub fn scope(&self) -> &SearchScope {
        &self.scope
    }

    /// Searches for relevant chunks without loading content.
    pub async fn search(&self, query: &str, limit: u32) -> Result<Vec<RetrievedChunk>> {
        self.service.search(self.scope.clone(), query, limit).await
    }

    /// Searches for relevant chunks and loads their content.
    pub async fn search_with_content(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<RetrievedChunk>> {
        self.service
            .search_with_content(self.scope.clone(), query, limit)
            .await
    }

    /// Loads content for retrieved chunks from NATS.
    pub async fn load_content(&self, chunks: &mut [RetrievedChunk]) -> Result<()> {
        self.service.load_content(chunks).await
    }
}
