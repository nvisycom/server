//! VLM service wrapper with observability.
//!
//! This module provides a wrapper around VLM implementations that adds
//! production-ready logging and tracing.

use std::sync::Arc;

use super::{BoxedStream, Request, Response, TRACING_TARGET, VlmProvider};
use crate::Result;
use crate::types::{Context, ServiceHealth, SharedContext};

/// VLM service wrapper with observability.
///
/// This wrapper adds structured logging to any VLM implementation.
/// The inner service is wrapped in `Arc` for cheap cloning.
#[derive(Clone)]
pub struct VlmService<Req = (), Resp = ()> {
    inner: Arc<dyn VlmProvider<Req, Resp>>,
    context: SharedContext,
}

impl<Req, Resp> VlmService<Req, Resp>
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    /// Create a new VLM service wrapper.
    pub fn new<P>(provider: P) -> Self
    where
        P: VlmProvider<Req, Resp> + 'static,
    {
        Self {
            inner: Arc::new(provider),
            context: SharedContext::new(),
        }
    }

    /// Create a new VLM service with a specific context.
    pub fn with_context<P>(provider: P, context: Context) -> Self
    where
        P: VlmProvider<Req, Resp> + 'static,
    {
        Self {
            inner: Arc::new(provider),
            context: SharedContext::from_context(context),
        }
    }

    /// Create a new VLM service with a shared context.
    pub fn with_shared_context<P>(provider: P, context: SharedContext) -> Self
    where
        P: VlmProvider<Req, Resp> + 'static,
    {
        Self {
            inner: Arc::new(provider),
            context,
        }
    }

    /// Create from a boxed provider.
    pub fn from_boxed(provider: Box<dyn VlmProvider<Req, Resp>>) -> Self {
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
    pub async fn process_vlm(&self, request: &Request<Req>) -> Result<Response<Resp>> {
        let start = std::time::Instant::now();

        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            image_count = request.images.len(),
            "Processing VLM request"
        );

        let result = self.inner.process_vlm(&self.context, request).await;
        let elapsed = start.elapsed();

        match &result {
            Ok(_) => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    elapsed_ms = elapsed.as_millis(),
                    "VLM processing successful"
                );
            }
            Err(error) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %error,
                    elapsed_ms = elapsed.as_millis(),
                    "VLM processing failed"
                );
            }
        }

        result
    }

    /// Process a vision-language request with streaming response.
    pub async fn process_vlm_stream(
        &self,
        request: &Request<Req>,
    ) -> Result<BoxedStream<Response<Resp>>> {
        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            "Starting VLM stream processing"
        );

        self.inner.process_vlm_stream(&self.context, request).await
    }

    /// Perform a health check on the VLM service.
    pub async fn health_check(&self) -> Result<ServiceHealth> {
        self.inner.health_check().await
    }
}
