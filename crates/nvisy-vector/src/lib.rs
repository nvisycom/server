#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod milvus;
pub mod pgvector;
pub mod pinecone;
pub mod qdrant;

mod config;
mod error;
mod store;

pub use config::{
    MilvusConfig, PgVectorConfig, PgVectorDistanceMetric, PgVectorIndexType, PineconeConfig,
    QdrantConfig, VectorStoreConfig,
};
pub use error::{VectorError, VectorResult};
pub use store::{SearchOptions, SearchResult, VectorData, VectorStore, VectorStoreBackend};

/// Tracing target for vector store operations.
pub const TRACING_TARGET: &str = "nvisy_vector";
