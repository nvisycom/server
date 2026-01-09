//! Embedding generation types and operations.
//!
//! This module provides types for text and document embedding generation,
//! supporting both single and batch operations.

mod request;
mod response;

pub use request::{EmbeddingBatchRequest, EmbeddingRequest};
pub use response::{EmbeddingBatchResponse, EmbeddingResponse};

use crate::Result;
use crate::service::Context;

/// Provider trait for embedding generation.
///
/// Implement this trait to create custom embedding providers.
#[async_trait::async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate an embedding for the provided input.
    async fn generate_embedding(
        &self,
        context: &Context,
        request: &EmbeddingRequest,
    ) -> Result<EmbeddingResponse>;
}

/// Extension trait for batch embedding operations.
///
/// Provides default implementations for batch processing.
#[async_trait::async_trait]
pub trait EmbeddingProviderExt: EmbeddingProvider {
    /// Generate embeddings for a batch of inputs.
    ///
    /// The default implementation processes requests concurrently.
    async fn generate_embedding_batch(
        &self,
        context: &Context,
        request: &EmbeddingBatchRequest,
    ) -> Result<EmbeddingBatchResponse> {
        let requests = request.iter_requests();
        let futures: Vec<_> = requests
            .iter()
            .map(|req| self.generate_embedding(context, req))
            .collect();

        let results = futures_util::future::join_all(futures).await;

        let mut responses = Vec::with_capacity(results.len());
        for result in results {
            responses.push(result?);
        }

        Ok(EmbeddingBatchResponse::new(responses))
    }
}

// Blanket implementation for all EmbeddingProvider implementations
impl<T: EmbeddingProvider + ?Sized> EmbeddingProviderExt for T {}
