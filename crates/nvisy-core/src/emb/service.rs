//! Embedding service implementation.
//!
//! This module provides concrete implementation of embedding services
//! that implement the [`EmbeddingProvider`] trait defined in the parent module.

use async_trait::async_trait;

use super::{EmbeddingProvider, EmbeddingRequest, EmbeddingResponse, Result};
use crate::types::ServiceHealth;

/// Concrete embedding service implementation.
///
/// This service provides a standard implementation of the [`EmbeddingProvider`] trait
/// that can be used as a base for specific embedding service implementations.
///
/// # Type Parameters
///
/// * `Req` - The request payload type specific to the embedding implementation
/// * `Resp` - The response payload type specific to the embedding implementation
///
/// # Example
///
/// ```rust,ignore
/// use nvisy_core::emb::{Service, EmbeddingRequest, EmbeddingResponse};
///
/// let service = Service::new(config);
/// let request = EmbeddingRequest::builder()
///     .input("Hello, world!")
///     .model("text-embedding-ada-002")
///     .build()?;
///
/// let response = service.embed(&request).await?;
/// ```
pub struct Service<Req, Resp> {
    _phantom_req: std::marker::PhantomData<Req>,
    _phantom_resp: std::marker::PhantomData<Resp>,
}

impl<Req, Resp> Service<Req, Resp> {
    /// Creates a new embedding service instance.
    ///
    /// # Returns
    ///
    /// A new [`Service`] instance ready to process embedding requests.
    pub fn new() -> Self {
        Self {
            _phantom_req: std::marker::PhantomData,
            _phantom_resp: std::marker::PhantomData,
        }
    }
}

impl<Req, Resp> Default for Service<Req, Resp> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl<Req, Resp> EmbeddingProvider<Req, Resp> for Service<Req, Resp>
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    async fn embed(&self, _request: &EmbeddingRequest) -> Result<EmbeddingResponse> {
        // This is a placeholder implementation.
        // Concrete implementations should override this method.
        todo!("Implement embedding generation for specific provider")
    }

    async fn health_check(&self) -> Result<ServiceHealth> {
        // This is a placeholder implementation.
        // Concrete implementations should override this method.
        todo!("Implement health check for specific provider")
    }
}
