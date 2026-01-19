//! Vector store trait and implementations.

use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::TRACING_TARGET;
use crate::config::VectorStoreConfig;
use crate::error::VectorResult;
use crate::milvus::MilvusBackend;
use crate::pgvector::PgVectorBackend;
use crate::pinecone::PineconeBackend;
use crate::qdrant::QdrantBackend;

/// Vector data to be stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorData {
    /// Unique identifier for the vector.
    pub id: String,
    /// The embedding vector.
    pub vector: Vec<f32>,
    /// Optional metadata.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl VectorData {
    /// Creates a new vector data with an ID and embedding.
    pub fn new(id: impl Into<String>, vector: Vec<f32>) -> Self {
        Self {
            id: id.into(),
            vector,
            metadata: HashMap::new(),
        }
    }

    /// Adds metadata to the vector.
    pub fn with_metadata(
        mut self,
        metadata: impl IntoIterator<Item = (impl Into<String>, serde_json::Value)>,
    ) -> Self {
        self.metadata = metadata.into_iter().map(|(k, v)| (k.into(), v)).collect();
        self
    }

    /// Adds a single metadata field.
    pub fn with_field(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Search result from a vector query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Vector ID.
    pub id: String,
    /// Similarity score.
    pub score: f32,
    /// The vector (if requested).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector: Option<Vec<f32>>,
    /// Associated metadata.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Search options.
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    /// Include vectors in results.
    pub include_vectors: bool,
    /// Include metadata in results.
    pub include_metadata: bool,
    /// Metadata filter (backend-specific JSON).
    pub filter: Option<serde_json::Value>,
    /// Namespace/partition (for backends that support it).
    pub namespace: Option<String>,
}

impl SearchOptions {
    /// Creates default search options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Include vectors in results.
    pub fn with_vectors(mut self) -> Self {
        self.include_vectors = true;
        self
    }

    /// Include metadata in results.
    pub fn with_metadata(mut self) -> Self {
        self.include_metadata = true;
        self
    }

    /// Set a metadata filter.
    pub fn with_filter(mut self, filter: serde_json::Value) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Set the namespace.
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }
}

/// Trait for vector store backends.
#[async_trait]
pub trait VectorStoreBackend: Send + Sync {
    /// Creates or ensures a collection exists.
    async fn create_collection(&self, name: &str, dimensions: usize) -> VectorResult<()>;

    /// Deletes a collection.
    async fn delete_collection(&self, name: &str) -> VectorResult<()>;

    /// Checks if a collection exists.
    async fn collection_exists(&self, name: &str) -> VectorResult<bool>;

    /// Upserts vectors into a collection.
    async fn upsert(&self, collection: &str, vectors: Vec<VectorData>) -> VectorResult<()>;

    /// Searches for similar vectors.
    async fn search(
        &self,
        collection: &str,
        query: Vec<f32>,
        limit: usize,
        options: SearchOptions,
    ) -> VectorResult<Vec<SearchResult>>;

    /// Deletes vectors by their IDs.
    async fn delete(&self, collection: &str, ids: Vec<String>) -> VectorResult<()>;

    /// Gets vectors by their IDs.
    async fn get(&self, collection: &str, ids: Vec<String>) -> VectorResult<Vec<VectorData>>;
}

/// Unified vector store that wraps backend implementations.
pub struct VectorStore {
    #[allow(dead_code)]
    config: VectorStoreConfig,
    #[allow(dead_code)]
    backend: Box<dyn VectorStoreBackend>,
}

impl VectorStore {
    /// Creates a new vector store from configuration.
    pub async fn new(config: VectorStoreConfig) -> VectorResult<Self> {
        let backend: Box<dyn VectorStoreBackend> = match &config {
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

    /// Creates or ensures a collection exists.
    pub async fn create_collection(&self, name: &str, dimensions: usize) -> VectorResult<()> {
        tracing::debug!(
            target: TRACING_TARGET,
            collection = %name,
            dimensions = %dimensions,
            "Creating collection"
        );
        self.backend.create_collection(name, dimensions).await
    }

    /// Deletes a collection.
    pub async fn delete_collection(&self, name: &str) -> VectorResult<()> {
        tracing::debug!(
            target: TRACING_TARGET,
            collection = %name,
            "Deleting collection"
        );
        self.backend.delete_collection(name).await
    }

    /// Checks if a collection exists.
    pub async fn collection_exists(&self, name: &str) -> VectorResult<bool> {
        self.backend.collection_exists(name).await
    }

    /// Upserts vectors into a collection.
    pub async fn upsert(&self, collection: &str, vectors: Vec<VectorData>) -> VectorResult<()> {
        tracing::debug!(
            target: TRACING_TARGET,
            collection = %collection,
            count = %vectors.len(),
            "Upserting vectors"
        );
        self.backend.upsert(collection, vectors).await
    }

    /// Searches for similar vectors.
    pub async fn search(
        &self,
        collection: &str,
        query: Vec<f32>,
        limit: usize,
    ) -> VectorResult<Vec<SearchResult>> {
        self.search_with_options(collection, query, limit, SearchOptions::default())
            .await
    }

    /// Searches for similar vectors with options.
    pub async fn search_with_options(
        &self,
        collection: &str,
        query: Vec<f32>,
        limit: usize,
        options: SearchOptions,
    ) -> VectorResult<Vec<SearchResult>> {
        tracing::debug!(
            target: TRACING_TARGET,
            collection = %collection,
            limit = %limit,
            "Searching vectors"
        );
        self.backend.search(collection, query, limit, options).await
    }

    /// Deletes vectors by their IDs.
    pub async fn delete(&self, collection: &str, ids: Vec<String>) -> VectorResult<()> {
        tracing::debug!(
            target: TRACING_TARGET,
            collection = %collection,
            count = %ids.len(),
            "Deleting vectors"
        );
        self.backend.delete(collection, ids).await
    }

    /// Gets vectors by their IDs.
    pub async fn get(&self, collection: &str, ids: Vec<String>) -> VectorResult<Vec<VectorData>> {
        tracing::debug!(
            target: TRACING_TARGET,
            collection = %collection,
            count = %ids.len(),
            "Getting vectors"
        );
        self.backend.get(collection, ids).await
    }
}

impl std::fmt::Debug for VectorStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorStore")
            .field("backend", &self.config.backend_name())
            .finish()
    }
}
