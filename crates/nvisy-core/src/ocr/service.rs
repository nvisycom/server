//! OCR service wrapper with observability.
//!
//! This module provides a wrapper around OCR implementations that adds
//! production-ready logging and service naming.

use std::sync::Arc;

use async_trait::async_trait;

use super::{BoxedOcrProvider, BoxedStream, OcrProvider, Request, Response, Result};
use crate::types::ServiceHealth;

/// OCR service wrapper with observability.
///
/// This wrapper adds logging and service naming to any OCR implementation.
/// The inner service is wrapped in Arc for cheap cloning.
///
/// # Type Parameters
///
/// * `Req` - The request payload type
/// * `Resp` - The response payload type
#[derive(Clone)]
pub struct OcrService<Req = (), Resp = ()> {
    inner: Arc<ServiceInner<Req, Resp>>,
}

struct ServiceInner<Req, Resp> {
    ocr: BoxedOcrProvider<Req, Resp>,
}

impl<Req, Resp> OcrService<Req, Resp>
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    /// Create a new service wrapper.
    ///
    /// # Parameters
    ///
    /// * `ocr` - OCR implementation
    pub fn new(ocr: BoxedOcrProvider<Req, Resp>) -> Self {
        Self {
            inner: Arc::new(ServiceInner { ocr }),
        }
    }
}

#[async_trait]
impl<Req, Resp> OcrProvider<Req, Resp> for OcrService<Req, Resp>
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    async fn process_ocr(&self, request: Request<Req>) -> Result<Response<Resp>> {
        tracing::debug!(
            target: super::TRACING_TARGET,
            request_id = %request.request_id,
            "Processing OCR request"
        );

        let start = std::time::Instant::now();

        let result = self.inner.ocr.process_ocr(request).await;

        match &result {
            Ok(response) => {
                tracing::debug!(
                    target: super::TRACING_TARGET,
                    response_id = %response.response_id,
                    elapsed = ?start.elapsed(),
                    "OCR processing successful"
                );
            }
            Err(error) => {
                tracing::error!(
                    target: super::TRACING_TARGET,
                    error = %error,
                    elapsed = ?start.elapsed(),
                    "OCR processing failed"
                );
            }
        }

        result
    }

    async fn process_ocr_stream(
        &self,
        request: Request<Req>,
    ) -> Result<BoxedStream<Response<Resp>>> {
        tracing::debug!(
            target: super::TRACING_TARGET,
            request_id = %request.request_id,
            "Starting OCR stream processing"
        );

        self.inner.ocr.process_ocr_stream(request).await
    }

    async fn health_check(&self) -> Result<ServiceHealth> {
        tracing::trace!(
            target: super::TRACING_TARGET,
            "Performing health check"
        );

        self.inner.ocr.health_check().await
    }
}
