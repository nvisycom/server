//! Vector store wrapper and unified API.

use nvisy_data::{
    DataResult, VectorContext, VectorData, VectorOutput, VectorSearchOptions, VectorSearchResult,
};

use crate::TRACING_TARGET;
use crate::config::VectorStoreConfig;
use crate::milvus::MilvusBackend;
use crate::pgvector::PgVectorBackend;
use crate::pinecone::PineconeBackend;
use crate::qdrant::QdrantBackend;

/// Unified vector store that wraps backend implementations.
pub struct VectorStore {
    #[allow(dead_code)]
    config: VectorStoreConfig,
    backend: Box<dyn VectorOutput>,
}

impl VectorStore {
    /// Creates a new vector store from configuration.
    pub async fn new(config: VectorStoreConfig) -> DataResult<Self> {
        let backend: Box<dyn VectorOutput> = match &config {
            VectorStoreConfig::Qdrant(cfg) => Box::new(QdrantBackend::new(cfg).await?),
            VectorStoreConfig::Milvus(cfg) => Box::new(MilvusBackend::new(cfg).await?),
            VectorStoreConfig::Pinecone(cfg) => Box::new(PineconeBackend::new(cfg).await?),
            VectorStoreConfig::PgVector(cfg) => Box::new(PgVectorBackend::new(cfg).await?),
        };

        tracing::info!(
            target: TRACING_TARGET,
            backend = %config.backend_name(),
            "Vector store initialized"
        );

        Ok(Self { config, backend })
    }

    /// Inserts vectors into a collection.
    pub async fn insert(&self, collection: &str, vectors: Vec<VectorData>) -> DataResult<()> {
        tracing::debug!(
            target: TRACING_TARGET,
            collection = %collection,
            count = %vectors.len(),
            "Inserting vectors"
        );

        let ctx = VectorContext::new(collection);
        self.backend.insert(&ctx, vectors).await
    }

    /// Searches for similar vectors.
    pub async fn search(
        &self,
        collection: &str,
        query: Vec<f32>,
        limit: usize,
    ) -> DataResult<Vec<VectorSearchResult>> {
        self.search_with_options(collection, query, limit, VectorSearchOptions::default())
            .await
    }

    /// Searches for similar vectors with options.
    pub async fn search_with_options(
        &self,
        collection: &str,
        query: Vec<f32>,
        limit: usize,
        options: VectorSearchOptions,
    ) -> DataResult<Vec<VectorSearchResult>> {
        tracing::debug!(
            target: TRACING_TARGET,
            collection = %collection,
            limit = %limit,
            "Searching vectors"
        );

        let ctx = VectorContext::new(collection);
        self.backend.search(&ctx, query, limit, options).await
    }

    /// Returns a reference to the underlying backend.
    pub fn backend(&self) -> &dyn VectorOutput {
        self.backend.as_ref()
    }
}

impl std::fmt::Debug for VectorStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorStore")
            .field("backend", &self.config.backend_name())
            .finish()
    }
}
