//! Milvus backend implementation.
//!
//! This is a stub implementation. The Milvus SDK API differs significantly
//! from the interface we designed. A full implementation would require
//! adapting to the actual milvus-sdk-rust API.

use async_trait::async_trait;

use super::MilvusConfig;
use crate::TRACING_TARGET;
use crate::error::{VectorError, VectorResult};
use crate::store::{SearchOptions, SearchResult, VectorData, VectorStoreBackend};

/// Milvus backend implementation.
pub struct MilvusBackend {
    #[allow(dead_code)]
    config: MilvusConfig,
}

impl MilvusBackend {
    /// Creates a new Milvus backend.
    pub async fn new(config: &MilvusConfig) -> VectorResult<Self> {
        tracing::debug!(
            target: TRACING_TARGET,
            host = %config.host,
            port = %config.port,
            "Milvus backend initialized (stub implementation)"
        );

        Ok(Self {
            config: config.clone(),
        })
    }
}

#[async_trait]
impl VectorStoreBackend for MilvusBackend {
    async fn create_collection(&self, name: &str, dimensions: usize) -> VectorResult<()> {
        tracing::warn!(
            target: TRACING_TARGET,
            collection = %name,
            dimensions = %dimensions,
            "Milvus create_collection is a stub - not yet implemented"
        );
        Err(VectorError::backend(
            "Milvus backend is not yet implemented",
        ))
    }

    async fn delete_collection(&self, name: &str) -> VectorResult<()> {
        tracing::warn!(
            target: TRACING_TARGET,
            collection = %name,
            "Milvus delete_collection is a stub - not yet implemented"
        );
        Err(VectorError::backend(
            "Milvus backend is not yet implemented",
        ))
    }

    async fn collection_exists(&self, name: &str) -> VectorResult<bool> {
        tracing::warn!(
            target: TRACING_TARGET,
            collection = %name,
            "Milvus collection_exists is a stub - not yet implemented"
        );
        Err(VectorError::backend(
            "Milvus backend is not yet implemented",
        ))
    }

    async fn upsert(&self, collection: &str, vectors: Vec<VectorData>) -> VectorResult<()> {
        tracing::warn!(
            target: TRACING_TARGET,
            collection = %collection,
            count = %vectors.len(),
            "Milvus upsert is a stub - not yet implemented"
        );
        Err(VectorError::backend(
            "Milvus backend is not yet implemented",
        ))
    }

    async fn search(
        &self,
        collection: &str,
        _query: Vec<f32>,
        _limit: usize,
        _options: SearchOptions,
    ) -> VectorResult<Vec<SearchResult>> {
        tracing::warn!(
            target: TRACING_TARGET,
            collection = %collection,
            "Milvus search is a stub - not yet implemented"
        );
        Err(VectorError::backend(
            "Milvus backend is not yet implemented",
        ))
    }

    async fn delete(&self, collection: &str, ids: Vec<String>) -> VectorResult<()> {
        tracing::warn!(
            target: TRACING_TARGET,
            collection = %collection,
            count = %ids.len(),
            "Milvus delete is a stub - not yet implemented"
        );
        Err(VectorError::backend(
            "Milvus backend is not yet implemented",
        ))
    }

    async fn get(&self, collection: &str, ids: Vec<String>) -> VectorResult<Vec<VectorData>> {
        tracing::warn!(
            target: TRACING_TARGET,
            collection = %collection,
            count = %ids.len(),
            "Milvus get is a stub - not yet implemented"
        );
        Err(VectorError::backend(
            "Milvus backend is not yet implemented",
        ))
    }
}
