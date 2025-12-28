//! VLM service wrapper with observability.
//!
//! This module provides a wrapper around VLM implementations that adds
//! production-ready logging and tracing.

use std::fmt;
use std::sync::Arc;

use jiff::Timestamp;

use super::{BatchRequest, BatchResponse, Request, Response, TRACING_TARGET, VlmProvider};
use crate::Result;
use crate::types::{Context, ServiceHealth, SharedContext};

/// VLM service wrapper with observability.
///
/// This wrapper adds structured logging to any VLM implementation.
/// The inner service is wrapped in `Arc` for cheap cloning.
#[derive(Clone)]
pub struct VlmService {
    inner: Arc<dyn VlmProvider>,
    context: SharedContext,
}

impl fmt::Debug for VlmService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VlmService")
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl VlmService {
    /// Create a new VLM service wrapper.
    pub fn new<P>(provider: P) -> Self
    where
        P: VlmProvider + 'static,
    {
        Self {
            inner: Arc::new(provider),
            context: SharedContext::new(),
        }
    }

    /// Create a new VLM service with a specific context.
    pub fn with_context<P>(provider: P, context: Context) -> Self
    where
        P: VlmProvider + 'static,
    {
        Self {
            inner: Arc::new(provider),
            context: SharedContext::from_context(context),
        }
    }

    /// Create a new VLM service with a shared context.
    pub fn with_shared_context<P>(provider: P, context: SharedContext) -> Self
    where
        P: VlmProvider + 'static,
    {
        Self {
            inner: Arc::new(provider),
            context,
        }
    }

    /// Create from a boxed provider.
    pub fn from_boxed(provider: Box<dyn VlmProvider>) -> Self {
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

    /// Process a vision-language request.
    pub async fn process_vlm(&self, request: &Request) -> Result<Response> {
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            document_count = request.document_count(),
            prompt_length = request.prompt_length(),
            "Processing VLM request"
        );

        let result = self.inner.process_vlm(&self.context, request).await;
        let elapsed = Timestamp::now().duration_since(started_at);

        match &result {
            Ok(response) => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    request_id = %request.request_id,
                    response_id = %response.response_id,
                    content_length = response.content_length(),
                    elapsed_ms = elapsed.as_millis(),
                    "VLM processing successful"
                );
            }
            Err(error) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    request_id = %request.request_id,
                    error = %error,
                    elapsed_ms = elapsed.as_millis(),
                    "VLM processing failed"
                );
            }
        }

        result
    }

    /// Process a batch of VLM requests.
    pub async fn process_vlm_batch(&self, request: &BatchRequest) -> Result<BatchResponse> {
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET,
            batch_size = request.len(),
            total_documents = request.total_documents(),
            "Processing batch VLM request"
        );

        let result = self.inner.process_vlm_batch(&self.context, request).await;
        let elapsed = Timestamp::now().duration_since(started_at);

        match &result {
            Ok(response) => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    batch_id = %response.batch_id,
                    count = response.len(),
                    elapsed_ms = elapsed.as_millis(),
                    "Batch VLM completed"
                );
            }
            Err(error) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %error,
                    elapsed_ms = elapsed.as_millis(),
                    "Batch VLM failed"
                );
            }
        }

        result
    }

    /// Perform a health check on the VLM service.
    pub async fn health_check(&self) -> Result<ServiceHealth> {
        self.inner.health_check().await
    }
}
