//! OCR service wrapper with observability.
//!
//! This module provides a wrapper around OCR implementations that adds
//! production-ready logging and tracing.

use std::sync::Arc;

use super::{BoxedStream, OcrProvider, Request, Response, TRACING_TARGET};
use crate::Result;
use crate::types::{Context, ServiceHealth, SharedContext};

/// OCR service wrapper with observability.
///
/// This wrapper adds structured logging to any OCR implementation.
/// The inner service is wrapped in `Arc` for cheap cloning.
#[derive(Clone)]
pub struct OcrService<Req = (), Resp = ()> {
    inner: Arc<dyn OcrProvider<Req, Resp>>,
    context: SharedContext,
}

impl<Req, Resp> OcrService<Req, Resp>
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    /// Create a new OCR service wrapper.
    pub fn new<P>(provider: P) -> Self
    where
        P: OcrProvider<Req, Resp> + 'static,
    {
        Self {
            inner: Arc::new(provider),
            context: SharedContext::new(),
        }
    }

    /// Create a new OCR service with a specific context.
    pub fn with_context<P>(provider: P, context: Context) -> Self
    where
        P: OcrProvider<Req, Resp> + 'static,
    {
        Self {
            inner: Arc::new(provider),
            context: SharedContext::from_context(context),
        }
    }

    /// Create a new OCR service with a shared context.
    pub fn with_shared_context<P>(provider: P, context: SharedContext) -> Self
    where
        P: OcrProvider<Req, Resp> + 'static,
    {
        Self {
            inner: Arc::new(provider),
            context,
        }
    }

    /// Create from a boxed provider.
    pub fn from_boxed(provider: Box<dyn OcrProvider<Req, Resp>>) -> Self {
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

    /// Process an image or document with OCR.
    pub async fn process_ocr(&self, request: Request<Req>) -> Result<Response<Resp>> {
        let start = std::time::Instant::now();

        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            "Processing OCR request"
        );

        let result = self.inner.process_ocr(&self.context, request).await;
        let elapsed = start.elapsed();

        match &result {
            Ok(response) => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    response_id = %response.response_id,
                    elapsed_ms = elapsed.as_millis(),
                    "OCR processing successful"
                );
            }
            Err(error) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %error,
                    elapsed_ms = elapsed.as_millis(),
                    "OCR processing failed"
                );
            }
        }

        result
    }

    /// Process an image or document with OCR using streaming responses.
    pub async fn process_ocr_stream(
        &self,
        request: Request<Req>,
    ) -> Result<BoxedStream<Response<Resp>>> {
        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            "Starting OCR stream processing"
        );

        self.inner.process_ocr_stream(&self.context, request).await
    }

    /// Perform a health check on the OCR service.
    pub async fn health_check(&self) -> Result<ServiceHealth> {
        self.inner.health_check().await
    }
}
