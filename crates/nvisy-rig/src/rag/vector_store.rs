//! Vector store implementation using PostgreSQL with pgvector.
//!
//! Provides rig-core compatible [`VectorStoreIndex`] and [`InsertDocuments`]
//! implementations backed by PostgreSQL for document chunk storage and
//! similarity search.

use nvisy_postgres::model::NewFileChunk;
use nvisy_postgres::query::FileChunkRepository;
use nvisy_postgres::{PgClient, Vector};
use rig::embeddings::{Embedding, TextEmbedder};
use rig::one_or_many::OneOrMany;
use rig::vector_store::request::{SearchFilter, VectorSearchRequest};
use rig::vector_store::{InsertDocuments, VectorStoreError, VectorStoreIndex};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::SearchScope;
use crate::provider::EmbeddingProvider;

/// PostgreSQL-backed vector store for document chunks.
///
/// Implements rig-core's [`VectorStoreIndex`] and [`InsertDocuments`] traits,
/// enabling integration with rig's agent and pipeline systems.
#[derive(Clone)]
pub struct PgVectorStore {
    provider: EmbeddingProvider,
    db: PgClient,
    scope: SearchScope,
    min_score: Option<f64>,
}

impl PgVectorStore {
    /// Creates a new vector store with the given scope.
    pub fn new(provider: EmbeddingProvider, db: PgClient, scope: SearchScope) -> Self {
        Self {
            provider,
            db,
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

    /// Returns the embedding provider.
    pub fn provider(&self) -> &EmbeddingProvider {
        &self.provider
    }
}

/// A document that can be stored in the vector store.
///
/// Contains the text content and metadata for a document chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkDocument {
    /// The text content of the chunk.
    pub text: String,
    /// The file ID this chunk belongs to.
    pub file_id: Uuid,
    /// The chunk index within the file.
    pub chunk_index: u32,
    /// Start byte offset in the source file.
    pub start_offset: u32,
    /// End byte offset in the source file.
    pub end_offset: u32,
    /// Optional page number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
}

impl ChunkDocument {
    /// Creates a new chunk document.
    pub fn new(
        text: impl Into<String>,
        file_id: Uuid,
        chunk_index: u32,
        start_offset: u32,
        end_offset: u32,
    ) -> Self {
        Self {
            text: text.into(),
            file_id,
            chunk_index,
            start_offset,
            end_offset,
            page: None,
        }
    }

    /// Sets the page number.
    pub fn with_page(mut self, page: u32) -> Self {
        self.page = Some(page);
        self
    }
}

impl rig::Embed for ChunkDocument {
    fn embed(&self, embedder: &mut TextEmbedder) -> Result<(), rig::embeddings::EmbedError> {
        embedder.embed(self.text.clone());
        Ok(())
    }
}

/// Filter type for PostgreSQL vector store queries.
///
/// Supports filtering by file ID and workspace scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PgFilter {
    /// Filter by exact file ID match.
    FileId(Uuid),
    /// Filter by workspace ID.
    WorkspaceId(Uuid),
    /// Combine filters with AND logic.
    And(Box<PgFilter>, Box<PgFilter>),
    /// Combine filters with OR logic.
    Or(Box<PgFilter>, Box<PgFilter>),
}

impl SearchFilter for PgFilter {
    type Value = serde_json::Value;

    fn eq(key: impl AsRef<str>, value: Self::Value) -> Self {
        match key.as_ref() {
            "file_id" => {
                if let Some(id) = value.as_str().and_then(|s| Uuid::parse_str(s).ok()) {
                    Self::FileId(id)
                } else {
                    // Fallback: treat as file ID filter with nil UUID
                    Self::FileId(Uuid::nil())
                }
            }
            "workspace_id" => {
                if let Some(id) = value.as_str().and_then(|s| Uuid::parse_str(s).ok()) {
                    Self::WorkspaceId(id)
                } else {
                    Self::WorkspaceId(Uuid::nil())
                }
            }
            _ => Self::FileId(Uuid::nil()),
        }
    }

    fn gt(_key: impl AsRef<str>, _value: Self::Value) -> Self {
        // Greater-than not meaningful for our use case
        Self::FileId(Uuid::nil())
    }

    fn lt(_key: impl AsRef<str>, _value: Self::Value) -> Self {
        // Less-than not meaningful for our use case
        Self::FileId(Uuid::nil())
    }

    fn and(self, rhs: Self) -> Self {
        Self::And(Box::new(self), Box::new(rhs))
    }

    fn or(self, rhs: Self) -> Self {
        Self::Or(Box::new(self), Box::new(rhs))
    }
}

impl InsertDocuments for PgVectorStore {
    async fn insert_documents<Doc: Serialize + rig::Embed + Send>(
        &self,
        documents: Vec<(Doc, OneOrMany<Embedding>)>,
    ) -> Result<(), VectorStoreError> {
        if documents.is_empty() {
            return Ok(());
        }

        let model_name = self.provider.model_name();

        let new_chunks: Vec<NewFileChunk> = documents
            .into_iter()
            .filter_map(|(doc, embeddings)| {
                // Serialize the document to extract fields
                let json = serde_json::to_value(&doc).ok()?;

                let text = json.get("text")?.as_str()?;
                let file_id = json
                    .get("file_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok())?;
                let chunk_index = json.get("chunk_index").and_then(|v| v.as_u64())? as i32;
                let start_offset = json.get("start_offset").and_then(|v| v.as_u64())? as u32;
                let end_offset = json.get("end_offset").and_then(|v| v.as_u64())? as u32;
                let page = json.get("page").and_then(|v| v.as_u64()).map(|p| p as u32);

                // Get the first embedding
                let embedding = embeddings.first();
                let embedding_vec: Vec<f32> = embedding.vec.iter().map(|&x| x as f32).collect();

                let content_bytes = text.as_bytes();
                let content_sha256 = Sha256::digest(content_bytes).to_vec();
                let content_size = content_bytes.len() as i32;

                let metadata = serde_json::json!({
                    "index": chunk_index,
                    "start_offset": start_offset,
                    "end_offset": end_offset,
                    "page": page,
                });

                Some(NewFileChunk {
                    file_id,
                    chunk_index: Some(chunk_index),
                    content_sha256,
                    content_size: Some(content_size),
                    token_count: None,
                    embedding: Vector::from(embedding_vec),
                    embedding_model: model_name.to_owned(),
                    metadata: Some(metadata),
                })
            })
            .collect();

        if new_chunks.is_empty() {
            return Ok(());
        }

        let mut conn = self.db.get_connection().await.map_err(|e| {
            VectorStoreError::DatastoreError(Box::new(std::io::Error::other(format!(
                "failed to get connection: {e}"
            ))))
        })?;

        conn.create_file_chunks(new_chunks).await.map_err(|e| {
            VectorStoreError::DatastoreError(Box::new(std::io::Error::other(format!(
                "failed to create chunks: {e}"
            ))))
        })?;

        Ok(())
    }
}

impl VectorStoreIndex for PgVectorStore {
    type Filter = PgFilter;

    async fn top_n<T: for<'a> Deserialize<'a> + Send>(
        &self,
        req: VectorSearchRequest<Self::Filter>,
    ) -> Result<Vec<(f64, String, T)>, VectorStoreError> {
        let query = req.query();
        let limit = req.samples() as i64;
        let min_score = req.threshold().or(self.min_score).unwrap_or(0.0);

        // Embed the query
        let embedding = self.provider.embed_text(query).await.map_err(|e| {
            VectorStoreError::DatastoreError(Box::new(std::io::Error::other(format!(
                "embedding failed: {e}"
            ))))
        })?;

        let query_vector: Vector = embedding
            .vec
            .iter()
            .map(|&x| x as f32)
            .collect::<Vec<_>>()
            .into();

        let mut conn = self.db.get_connection().await.map_err(|e| {
            VectorStoreError::DatastoreError(Box::new(std::io::Error::other(format!(
                "failed to get connection: {e}"
            ))))
        })?;

        // Use the scope to determine which search method to use
        let scored_chunks = match &self.scope {
            SearchScope::Files(file_ids) => {
                conn.search_scored_chunks_in_files(query_vector, file_ids, min_score, limit)
                    .await
            }
            SearchScope::Workspace(workspace_id) => {
                conn.search_scored_chunks_in_workspace(
                    query_vector,
                    *workspace_id,
                    min_score,
                    limit,
                )
                .await
            }
        }
        .map_err(|e| {
            VectorStoreError::DatastoreError(Box::new(std::io::Error::other(format!(
                "vector search failed: {e}"
            ))))
        })?;

        // Convert to rig format
        let results: Vec<(f64, String, T)> = scored_chunks
            .into_iter()
            .filter_map(|scored| {
                let chunk = scored.chunk;
                let id = chunk.id.to_string();

                // Build a document representation from metadata
                let doc_json = serde_json::json!({
                    "file_id": chunk.file_id.to_string(),
                    "chunk_index": chunk.chunk_index,
                    "metadata": chunk.metadata,
                });

                let doc: T = serde_json::from_value(doc_json).ok()?;
                Some((scored.score, id, doc))
            })
            .collect();

        Ok(results)
    }

    async fn top_n_ids(
        &self,
        req: VectorSearchRequest<Self::Filter>,
    ) -> Result<Vec<(f64, String)>, VectorStoreError> {
        let query = req.query();
        let limit = req.samples() as i64;
        let min_score = req.threshold().or(self.min_score).unwrap_or(0.0);

        // Embed the query
        let embedding = self.provider.embed_text(query).await.map_err(|e| {
            VectorStoreError::DatastoreError(Box::new(std::io::Error::other(format!(
                "embedding failed: {e}"
            ))))
        })?;

        let query_vector: Vector = embedding
            .vec
            .iter()
            .map(|&x| x as f32)
            .collect::<Vec<_>>()
            .into();

        let mut conn = self.db.get_connection().await.map_err(|e| {
            VectorStoreError::DatastoreError(Box::new(std::io::Error::other(format!(
                "failed to get connection: {e}"
            ))))
        })?;

        let scored_chunks = match &self.scope {
            SearchScope::Files(file_ids) => {
                conn.search_scored_chunks_in_files(query_vector, file_ids, min_score, limit)
                    .await
            }
            SearchScope::Workspace(workspace_id) => {
                conn.search_scored_chunks_in_workspace(
                    query_vector,
                    *workspace_id,
                    min_score,
                    limit,
                )
                .await
            }
        }
        .map_err(|e| {
            VectorStoreError::DatastoreError(Box::new(std::io::Error::other(format!(
                "vector search failed: {e}"
            ))))
        })?;

        let results: Vec<(f64, String)> = scored_chunks
            .into_iter()
            .map(|scored| (scored.score, scored.chunk.id.to_string()))
            .collect();

        Ok(results)
    }
}
