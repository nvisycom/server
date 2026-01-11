//! RAG (Retrieval-Augmented Generation) module.
//!
//! Provides document indexing and semantic search over document chunks.
//!
//! # Modules
//!
//! - [`indexer`] - Batch embedding and storage of document chunks
//! - [`search`] - Semantic similarity search with content retrieval
//! - [`splitter`] - Text splitting for chunking documents
//!
//! # Security
//!
//! All searches must be scoped to specific files or documents via [`SearchScope`].
//!
//! # Example
//!
//! ```ignore
//! use nvisy_rig::rag::{RagService, SearchScope};
//!
//! let rag = RagService::new(embedding_provider, pg, &nats).await?;
//!
//! // Index a file
//! let chunks = rag.split_text(&content);
//! let indexed = rag.indexer(file_id).index_chunks(chunks).await?;
//!
//! // Search within a document
//! let results = rag
//!     .search(SearchScope::document(doc_id), "How does auth work?", 5)
//!     .await?;
//! ```

mod config;
pub mod indexer;
pub mod search;
pub mod splitter;

use std::sync::Arc;

use nvisy_nats::NatsClient;
use nvisy_nats::object::{DocumentStore, Files};
use nvisy_postgres::PgClient;
use uuid::Uuid;

pub use self::config::RagConfig;
pub use self::indexer::{IndexedChunk, Indexer};
pub use self::search::{
    ChunkMetadata, ChunkResult, ChunkVectorStore, RetrievedChunk, SearchScope, SearchService,
};
pub use self::splitter::{OwnedSplitChunk, SplitChunk, SplitMetadata, TextSplitterService};
use crate::Result;
use crate::service::provider::EmbeddingProvider;

/// High-level RAG service for document indexing and semantic search.
///
/// Encapsulates:
/// - Text splitting for document chunking
/// - Batch embedding and storage via [`Indexer`]
/// - Vector search via pgvector with [`SearchService`]
/// - Content retrieval from NATS
///
/// The service is cheap to clone and can be shared across threads.
///
/// # Example
///
/// ```ignore
/// let rag = RagService::new(embedding_provider, pg, &nats).await?;
///
/// // Index a file
/// let chunks = rag.split_text(&content);
/// rag.indexer(file_id).index_chunks(chunks).await?;
///
/// // Search
/// let results = rag
///     .search(SearchScope::document(doc_id), "query", 5)
///     .await?;
/// ```
#[derive(Clone)]
pub struct RagService {
    inner: Arc<RagServiceInner>,
}

struct RagServiceInner {
    provider: EmbeddingProvider,
    db: PgClient,
    files: DocumentStore<Files>,
    splitter: TextSplitterService,
    config: RagConfig,
}

impl RagService {
    /// Creates a new RAG service.
    pub async fn new(provider: EmbeddingProvider, db: PgClient, nats: &NatsClient) -> Result<Self> {
        Self::with_config(provider, db, nats, RagConfig::default()).await
    }

    /// Creates a new RAG service with custom configuration.
    pub async fn with_config(
        provider: EmbeddingProvider,
        db: PgClient,
        nats: &NatsClient,
        config: RagConfig,
    ) -> Result<Self> {
        let files = nats
            .document_store::<Files>()
            .await
            .map_err(|e| crate::Error::retrieval(format!("failed to open document store: {e}")))?;

        let splitter = TextSplitterService::new(config.max_chunk_characters, config.trim_chunks);

        Ok(Self {
            inner: Arc::new(RagServiceInner {
                provider,
                db,
                files,
                splitter,
                config,
            }),
        })
    }

    /// Returns the configuration.
    pub fn config(&self) -> &RagConfig {
        &self.inner.config
    }

    /// Returns a reference to the text splitter.
    pub fn splitter(&self) -> &TextSplitterService {
        &self.inner.splitter
    }

    /// Returns a reference to the embedding provider.
    pub fn provider(&self) -> &EmbeddingProvider {
        &self.inner.provider
    }

    /// Creates an indexer for batch-embedding and storing document chunks.
    ///
    /// The indexer uses batched embedding requests for efficiency.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let chunks = rag.split_text(&content);
    /// let indexed = rag.indexer(file_id)
    ///     .with_model_name("nomic-embed-text")
    ///     .index_chunks(chunks)
    ///     .await?;
    /// ```
    pub fn indexer(&self, file_id: Uuid) -> Indexer {
        Indexer::new(self.inner.provider.clone(), self.inner.db.clone(), file_id)
    }

    /// Splits text into chunks.
    pub fn split_text(&self, text: &str) -> Vec<OwnedSplitChunk> {
        self.inner.splitter.split_owned(text)
    }

    /// Splits text with page break awareness.
    pub fn split_text_with_pages(&self, text: &str) -> Vec<OwnedSplitChunk> {
        self.inner
            .splitter
            .split_with_pages(text)
            .into_iter()
            .map(|c| c.into_owned())
            .collect()
    }

    /// Returns the search service for direct access to search functionality.
    pub fn search_service(&self) -> SearchService {
        SearchService::new(
            self.inner.provider.clone(),
            self.inner.db.clone(),
            self.inner.files.clone(),
        )
        .with_min_score(self.inner.config.min_score)
    }

    /// Searches for relevant chunks without loading content.
    pub async fn search(
        &self,
        scope: SearchScope,
        query: &str,
        limit: u32,
    ) -> Result<Vec<RetrievedChunk>> {
        self.search_service().search(scope, query, limit).await
    }

    /// Searches for relevant chunks and loads their content.
    pub async fn search_with_content(
        &self,
        scope: SearchScope,
        query: &str,
        limit: u32,
    ) -> Result<Vec<RetrievedChunk>> {
        self.search_service()
            .search_with_content(scope, query, limit)
            .await
    }

    /// Loads content for retrieved chunks from NATS.
    pub async fn load_content(&self, chunks: &mut [RetrievedChunk]) -> Result<()> {
        self.search_service().load_content(chunks).await
    }

    /// Creates a scoped service for searching within specific files or documents.
    pub fn scoped(&self, scope: SearchScope) -> ScopedRagService<'_> {
        ScopedRagService {
            service: self,
            scope,
        }
    }
}

/// A RAG service scoped to specific files or documents.
///
/// Created via [`RagService::scoped`]. Provides the same search methods
/// but with the scope pre-configured.
pub struct ScopedRagService<'a> {
    service: &'a RagService,
    scope: SearchScope,
}

impl ScopedRagService<'_> {
    /// Returns the search scope.
    pub fn scope(&self) -> &SearchScope {
        &self.scope
    }

    /// Returns the configuration.
    pub fn config(&self) -> &RagConfig {
        self.service.config()
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
