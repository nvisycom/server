//! Embedding service wrapper with observability.
//!
//! This module provides a wrapper around embedding implementations that adds
//! production-ready logging and service naming.

use std::sync::Arc;

use async_trait::async_trait;

use super::{BoxedEmbeddingProvider, EmbeddingProvider, Request, Response, Result};
use crate::types::ServiceHealth;

/// Embedding service wrapper with observability.
///
/// This wrapper adds logging and service naming to any embedding implementation.
/// The inner service is wrapped in Arc for cheap cloning.
///
/// # Type Parameters
///
/// * `Req` - The request payload type
/// * `Resp` - The response payload type
#[derive(Clone)]
pub struct EmbeddingService<Req = (), Resp = ()> {
    inner: Arc<ServiceInner<Req, Resp>>,
}

struct ServiceInner<Req, Resp> {
    embedding: BoxedEmbeddingProvider<Req, Resp>,
}

impl<Req, Resp> EmbeddingService<Req, Resp>
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    /// Create a new embedding service wrapper.
    ///
    /// # Parameters
    ///
    /// * `embedding` - Embedding implementation
    pub fn new(embedding: BoxedEmbeddingProvider<Req, Resp>) -> Self {
        Self {
            inner: Arc::new(ServiceInner { embedding }),
        }
    }
}

#[async_trait]
impl<Req, Resp> EmbeddingProvider<Req, Resp> for EmbeddingService<Req, Resp>
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    async fn generate_embedding(&self, request: Request<Req>) -> Result<Response<Resp>> {
        tracing::debug!(
            target: super::TRACING_TARGET,
            request_id = %request.request_id,
            "Processing embedding request"
        );

        let start = std::time::Instant::now();

        let result = self.inner.embedding.generate_embedding(request).await;

        match &result {
            Ok(response) => {
                tracing::debug!(
                    target: super::TRACING_TARGET,
                    response_id = %response.response_id,
                    elapsed = ?start.elapsed(),
                    "Embedding generation successful"
                );
            }
            Err(error) => {
                tracing::error!(
                    target: super::TRACING_TARGET,
                    error = %error,
                    elapsed = ?start.elapsed(),
                    "Embedding generation failed"
                );
            }
        }

        result
    }

    async fn health_check(&self) -> Result<ServiceHealth> {
        tracing::trace!(
            target: super::TRACING_TARGET,
            "Performing health check"
        );

        self.inner.embedding.health_check().await
    }
}
