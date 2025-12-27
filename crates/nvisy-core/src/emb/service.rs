//! Embedding service wrapper with observability.
//!
//! This module provides a wrapper around embedding implementations that adds
//! production-ready logging and tracing.

use std::fmt;
use std::sync::Arc;

use jiff::Timestamp;

use super::{BatchRequest, BatchResponse, EmbeddingProvider, Request, Response, TRACING_TARGET};
use crate::Result;
use crate::types::{Context, ServiceHealth, SharedContext};

/// Embedding service wrapper with observability.
///
/// This wrapper adds structured logging to any embedding implementation.
/// The inner service is wrapped in `Arc` for cheap cloning.
#[derive(Clone)]
pub struct EmbeddingService {
    inner: Arc<dyn EmbeddingProvider>,
    context: SharedContext,
}

impl fmt::Debug for EmbeddingService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EmbeddingService")
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl EmbeddingService {
    /// Create a new embedding service wrapper.
    pub fn new<P>(provider: P) -> Self
    where
        P: EmbeddingProvider + 'static,
    {
        Self {
            inner: Arc::new(provider),
            context: SharedContext::new(),
        }
    }

    /// Create a new embedding service with a specific context.
    pub fn with_context<P>(provider: P, context: Context) -> Self
    where
        P: EmbeddingProvider + 'static,
    {
        Self {
            inner: Arc::new(provider),
            context: SharedContext::from_context(context),
        }
    }

    /// Create a new embedding service with a shared context.
    pub fn with_shared_context<P>(provider: P, context: SharedContext) -> Self
    where
        P: EmbeddingProvider + 'static,
    {
        Self {
            inner: Arc::new(provider),
            context,
        }
    }

    /// Create from a boxed provider.
    pub fn from_boxed(provider: Box<dyn EmbeddingProvider>) -> Self {
        Self {
            inner: Arc::from(provider),
            context: SharedContext::new(),
        }
    }

    /// Get a reference to the shared context.
    pub fn context(&self) -> &SharedContext {
        &self.context
    }

    /// Replace the context.
    pub fn set_context(&mut self, context: SharedContext) {
        self.context = context;
    }

    /// Generate an embedding for the provided input.
    pub async fn generate_embedding(&self, request: &Request) -> Result<Response> {
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            "Processing embedding request"
        );

        let result = self.inner.generate_embedding(&self.context, request).await;
        let elapsed = Timestamp::now().duration_since(started_at);

        match &result {
            Ok(response) => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    request_id = %request.request_id,
                    response_id = %response.response_id,
                    dimensions = response.dimensions(),
                    elapsed_ms = elapsed.as_millis(),
                    "Embedding generation successful"
                );
            }
            Err(error) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    request_id = %request.request_id,
                    error = %error,
                    elapsed_ms = elapsed.as_millis(),
                    "Embedding generation failed"
                );
            }
        }

        result
    }

    /// Generate embeddings for a batch of inputs.
    pub async fn generate_embedding_batch(&self, request: &BatchRequest) -> Result<BatchResponse> {
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET,
            batch_size = request.len(),
            "Processing batch embedding request"
        );

        let result = self
            .inner
            .generate_embedding_batch(&self.context, request)
            .await;
        let elapsed = Timestamp::now().duration_since(started_at);

        match &result {
            Ok(response) => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    batch_id = %response.batch_id,
                    count = response.len(),
                    elapsed_ms = elapsed.as_millis(),
                    "Batch embedding completed"
                );
            }
            Err(error) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %error,
                    elapsed_ms = elapsed.as_millis(),
                    "Batch embedding failed"
                );
            }
        }

        result
    }

    /// Perform a health check on the embedding service.
    pub async fn health_check(&self) -> Result<ServiceHealth> {
        self.inner.health_check().await
    }
}
