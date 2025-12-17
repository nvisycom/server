//! OCR service wrapper with observability.
//!
//! This module provides a wrapper around OCR implementations that adds
//! production-ready logging and service naming.

use std::sync::Arc;

use async_trait::async_trait;

use super::{BoxedOcr, BoxedStream, Ocr, Request, Response, Result};
use crate::health::ServiceHealth;

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
    ocr: BoxedOcr<Req, Resp>,
    service_name: String,
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
    /// * `service_name` - Name for logging and identification
    pub fn new(ocr: BoxedOcr<Req, Resp>, service_name: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(ServiceInner {
                ocr,
                service_name: service_name.into(),
            }),
        }
    }

    /// Get the service name.
    pub fn service_name(&self) -> &str {
        &self.inner.service_name
    }
}

#[async_trait]
impl<Req, Resp> Ocr<Req, Resp> for OcrService<Req, Resp>
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    async fn process_with_ocr(&self, request: Request<Req>) -> Result<Response<Resp>> {
        tracing::debug!(
            target: crate::TRACING_TARGET_OCR,
            service = %self.inner.service_name,
            request_id = %request.request_id,
            "Processing OCR request"
        );

        let start = std::time::Instant::now();

        let result = self.inner.ocr.process_with_ocr(request).await;

        match &result {
            Ok(response) => {
                tracing::debug!(
                    target: crate::TRACING_TARGET_OCR,
                    service = %self.inner.service_name,
                    response_id = %response.response_id,
                    elapsed = ?start.elapsed(),
                    "OCR processing successful"
                );
            }
            Err(error) => {
                tracing::error!(
                    target: crate::TRACING_TARGET_OCR,
                    service = %self.inner.service_name,
                    error = %error,
                    elapsed = ?start.elapsed(),
                    "OCR processing failed"
                );
            }
        }

        result
    }

    async fn process_stream(&self, request: Request<Req>) -> Result<BoxedStream<Response<Resp>>> {
        tracing::debug!(
            target: crate::TRACING_TARGET_OCR,
            service = %self.inner.service_name,
            request_id = %request.request_id,
            "Starting OCR stream processing"
        );

        self.inner.ocr.process_stream(request).await
    }

    async fn health_check(&self) -> Result<ServiceHealth> {
        tracing::trace!(
            target: crate::TRACING_TARGET_OCR,
            service = %self.inner.service_name,
            "Performing health check"
        );

        self.inner.ocr.health_check().await
    }
}
