//! Vector store configuration types.

use serde::{Deserialize, Serialize};

// Re-export configs from backend modules
pub use crate::milvus::MilvusConfig;
pub use crate::pgvector::{PgVectorConfig, PgVectorDistanceMetric, PgVectorIndexType};
pub use crate::pinecone::PineconeConfig;
pub use crate::qdrant::QdrantConfig;

/// Vector store backend configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum VectorStoreConfig {
    /// Qdrant vector database.
    Qdrant(QdrantConfig),
    /// Milvus vector database.
    Milvus(MilvusConfig),
    /// Pinecone managed vector database.
    Pinecone(PineconeConfig),
    /// PostgreSQL with pgvector extension.
    PgVector(PgVectorConfig),
}

impl VectorStoreConfig {
    /// Returns the backend name as a static string.
    pub fn backend_name(&self) -> &'static str {
        match self {
            Self::Qdrant(_) => "qdrant",
            Self::Milvus(_) => "milvus",
            Self::Pinecone(_) => "pinecone",
            Self::PgVector(_) => "pgvector",
        }
    }
}
