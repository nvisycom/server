//! VLM service wrapper with observability.
//!
//! This module provides a wrapper around VLM implementations that adds
//! production-ready logging and service naming.

use std::sync::Arc;

use async_trait::async_trait;

use super::{BoxedStream, BoxedVlmProvider, Request, Response, Result, VlmProvider};
use crate::types::ServiceHealth;

/// VLM service wrapper with observability.
///
/// This wrapper adds logging and service naming to any VLM implementation.
/// The inner service is wrapped in Arc for cheap cloning.
///
/// # Type Parameters
///
/// * `Req` - The request payload type
/// * `Resp` - The response payload type
#[derive(Clone)]
pub struct Service<Req = (), Resp = ()> {
    inner: Arc<ServiceInner<Req, Resp>>,
}

struct ServiceInner<Req, Resp> {
    vlm: BoxedVlmProvider<Req, Resp>,
}

impl<Req, Resp> Service<Req, Resp>
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    /// Create a new service wrapper.
    ///
    /// # Parameters
    ///
    /// * `vlm` - VLM implementation
    pub fn new(vlm: BoxedVlmProvider<Req, Resp>) -> Self {
        Self {
            inner: Arc::new(ServiceInner {
                vlm,
            }),
        }
    }
}

#[async_trait]
impl<Req, Resp> VlmProvider<Req, Resp> for Service<Req, Resp>
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    async fn process_vlm(&self, request: &Request<Req>) -> Result<Response<Resp>> {
        tracing::debug!(
            target: super::TRACING_TARGET,
            request_id = %request.request_id,
            image_count = request.images.len(),
            "Processing VLM request"
        );

        let start = std::time::Instant::now();

        let result = self.inner.vlm.process_vlm(request).await;

        match &result {
            Ok(_) => {
                tracing::debug!(
                    target: super::TRACING_TARGET,
                    elapsed = ?start.elapsed(),
                    "VLM processing successful"
                );
            }
            Err(error) => {
                tracing::error!(
                    target: super::TRACING_TARGET,
                    error = %error,
                    elapsed = ?start.elapsed(),
                    "VLM processing failed"
                );
            }
        }

        result
    }

    async fn process_vlm_stream(
        &self,
        request: &Request<Req>,
    ) -> Result<BoxedStream<Response<Resp>>> {
        tracing::debug!(
            target: super::TRACING_TARGET,
            request_id = %request.request_id,
            "Starting VLM stream processing"
        );

        self.inner.vlm.process_vlm_stream(request).await
    }

    async fn health_check(&self) -> Result<ServiceHealth> {
        tracing::trace!(
            target: super::TRACING_TARGET,
            "Performing health check"
        );

        self.inner.vlm.health_check().await
    }
}
