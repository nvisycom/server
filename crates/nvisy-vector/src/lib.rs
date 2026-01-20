//! Vector store backends for nvisy.
//!
//! This crate provides vector store implementations that implement the
//! [`VectorOutput`] trait from `nvisy-data`.

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod milvus;
pub mod pgvector;
pub mod pinecone;
pub mod qdrant;

mod config;
mod store;

pub use config::{
    MilvusConfig, PgVectorConfig, PgVectorDistanceMetric, PgVectorIndexType, PineconeConfig,
    QdrantConfig, VectorStoreConfig,
};
// Re-export types from nvisy-data for convenience
pub use nvisy_data::{
    DataError, DataResult, VectorContext, VectorData, VectorOutput, VectorSearchOptions,
    VectorSearchResult,
};
pub use store::VectorStore;

/// Tracing target for vector store operations.
pub const TRACING_TARGET: &str = "nvisy_vector";
