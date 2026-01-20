//! Vector output trait for inserting into vector stores.

use async_trait::async_trait;

use crate::error::DataResult;
use crate::types::{VectorData, VectorSearchResult};

/// Context for vector operations.
#[derive(Debug, Clone, Default)]
pub struct VectorContext {
    /// The collection/index/namespace to operate on.
    pub collection: String,
    /// Additional options as key-value pairs.
    pub options: std::collections::HashMap<String, String>,
}

impl VectorContext {
    /// Creates a new context with the given collection name.
    pub fn new(collection: impl Into<String>) -> Self {
        Self {
            collection: collection.into(),
            options: std::collections::HashMap::new(),
        }
    }

    /// Adds an option.
    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }
}

/// Options for vector search operations.
#[derive(Debug, Clone, Default)]
pub struct VectorSearchOptions {
    /// Whether to include the vector data in results.
    pub include_vectors: bool,
    /// Whether to include metadata in results.
    pub include_metadata: bool,
    /// Optional filter (backend-specific format).
    pub filter: Option<serde_json::Value>,
}

impl VectorSearchOptions {
    /// Creates new search options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Include vectors in the results.
    pub fn with_vectors(mut self) -> Self {
        self.include_vectors = true;
        self
    }

    /// Include metadata in the results.
    pub fn with_metadata(mut self) -> Self {
        self.include_metadata = true;
        self
    }

    /// Set a filter for the search.
    pub fn with_filter(mut self, filter: serde_json::Value) -> Self {
        self.filter = Some(filter);
        self
    }
}

/// Trait for inserting vectors into vector stores.
#[async_trait]
pub trait VectorOutput: Send + Sync {
    /// Inserts vectors into the specified collection.
    ///
    /// If vectors with the same IDs already exist, they may be overwritten
    /// (behavior depends on the backend).
    async fn insert(&self, ctx: &VectorContext, vectors: Vec<VectorData>) -> DataResult<()>;

    /// Searches for similar vectors.
    async fn search(
        &self,
        ctx: &VectorContext,
        query: Vec<f32>,
        limit: usize,
        options: VectorSearchOptions,
    ) -> DataResult<Vec<VectorSearchResult>>;
}
