//! RAG (Retrieval-Augmented Generation) module.
//!
//! Provides document indexing and semantic search over document chunks.
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
//! let indexed = rag.indexer(file_id).index(&content).await?;
//!
//! // Search within a document
//! let results = rag
//!     .search(SearchScope::document(doc_id))
//!     .query("How does auth work?", 5)
//!     .await?;
//! ```

mod config;
mod indexer;
mod searcher;
mod splitter;

use std::sync::Arc;

use nvisy_nats::NatsClient;
use nvisy_nats::object::{FileKey, FilesBucket, ObjectStore};
use nvisy_postgres::PgClient;
use uuid::Uuid;

pub use self::config::RagConfig;
pub use self::indexer::{IndexedChunk, Indexer};
pub use self::searcher::{ChunkMetadata, RetrievedChunk, SearchScope, Searcher};
use self::splitter::Splitter;
pub use self::splitter::estimate_tokens;
use crate::Result;
use crate::provider::EmbeddingProvider;

/// High-level RAG service for document indexing and semantic search.
///
/// The service is cheap to clone and can be shared across threads.
#[derive(Clone)]
pub struct RagService {
    inner: Arc<RagServiceInner>,
}

struct RagServiceInner {
    provider: EmbeddingProvider,
    db: PgClient,
    files: ObjectStore<FilesBucket, FileKey>,
    config: RagConfig,
}

impl RagService {
    /// Creates a new RAG service.
    pub async fn new(
        config: RagConfig,
        provider: EmbeddingProvider,
        db: PgClient,
        nats: NatsClient,
    ) -> Result<Self> {
        let files = nats
            .object_store::<FilesBucket, FileKey>()
            .await
            .map_err(|e| crate::Error::retrieval(format!("failed to open file store: {e}")))?;

        let inner = RagServiceInner {
            provider,
            db,
            files,
            config,
        };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Returns the configuration.
    pub fn config(&self) -> &RagConfig {
        &self.inner.config
    }

    /// Creates an indexer for a specific file.
    pub fn indexer(&self, file_id: Uuid) -> Indexer {
        let splitter = Splitter::new(
            self.inner.config.max_chunk_characters,
            self.inner.config.chunk_overlap,
            self.inner.config.trim_chunks,
        );

        Indexer::new(
            self.inner.provider.clone(),
            self.inner.db.clone(),
            splitter,
            file_id,
        )
    }

    /// Creates a search service for specific files or documents.
    pub fn search(&self, scope: SearchScope) -> Searcher {
        let searcher = Searcher::new(
            self.inner.provider.clone(),
            self.inner.db.clone(),
            self.inner.files.clone(),
            scope,
        );

        match self.inner.config.min_score {
            Some(min_score) => searcher.with_min_score(min_score),
            None => searcher,
        }
    }
}
