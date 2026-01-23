//! Context for vector database operations.

/// Context for vector database operations (Qdrant, Pinecone, Milvus, pgvector).
#[derive(Debug, Clone, Default)]
pub struct VectorContext {
    /// Target collection name.
    pub collection: Option<String>,
}

impl VectorContext {
    /// Creates a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the collection name.
    pub fn with_collection(mut self, collection: impl Into<String>) -> Self {
        self.collection = Some(collection.into());
        self
    }
}
