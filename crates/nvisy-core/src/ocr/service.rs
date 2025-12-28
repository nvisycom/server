//! OCR service wrapper with observability.
//!
//! This module provides a wrapper around OCR implementations that adds
//! production-ready logging and tracing.

use std::fmt;
use std::sync::Arc;

use jiff::Timestamp;

use super::{BatchRequest, BatchResponse, OcrProvider, Request, Response, TRACING_TARGET};
use crate::Result;
use crate::types::{Context, ServiceHealth, SharedContext};

/// OCR service wrapper with observability.
///
/// This wrapper adds structured logging to any OCR implementation.
/// The inner service is wrapped in `Arc` for cheap cloning.
#[derive(Clone)]
pub struct OcrService {
    inner: Arc<dyn OcrProvider>,
    context: SharedContext,
}

impl fmt::Debug for OcrService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OcrService")
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl OcrService {
    /// Create a new OCR service wrapper.
    pub fn new<P>(provider: P) -> Self
    where
        P: OcrProvider + 'static,
    {
        Self {
            inner: Arc::new(provider),
            context: SharedContext::new(),
        }
    }

    /// Create a new OCR service with a specific context.
    pub fn with_context<P>(provider: P, context: Context) -> Self
    where
        P: OcrProvider + 'static,
    {
        Self {
            inner: Arc::new(provider),
            context: SharedContext::from_context(context),
        }
    }

    /// Create a new OCR service with a shared context.
    pub fn with_shared_context<P>(provider: P, context: SharedContext) -> Self
    where
        P: OcrProvider + 'static,
    {
        Self {
            inner: Arc::new(provider),
            context,
        }
    }

    /// Create from a boxed provider.
    pub fn from_boxed(provider: Box<dyn OcrProvider>) -> Self {
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
    pub async fn process_ocr(&self, request: &Request) -> Result<Response> {
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            document_size = request.document_size(),
            "Processing OCR request"
        );

        let result = self.inner.process_ocr(&self.context, request).await;
        let elapsed = Timestamp::now().duration_since(started_at);

        match &result {
            Ok(response) => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    request_id = %request.request_id,
                    response_id = %response.response_id,
                    text_len = response.text.len(),
                    elapsed_ms = elapsed.as_millis(),
                    "OCR processing successful"
                );
            }
            Err(error) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    request_id = %request.request_id,
                    error = %error,
                    elapsed_ms = elapsed.as_millis(),
                    "OCR processing failed"
                );
            }
        }

        result
    }

    /// Process a batch of OCR requests.
    pub async fn process_ocr_batch(&self, request: &BatchRequest) -> Result<BatchResponse> {
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET,
            batch_size = request.len(),
            "Processing batch OCR request"
        );

        let result = self.inner.process_ocr_batch(&self.context, request).await;
        let elapsed = Timestamp::now().duration_since(started_at);

        match &result {
            Ok(response) => {
                tracing::debug!(
                    target: TRACING_TARGET,
                    batch_id = %response.batch_id,
                    count = response.len(),
                    elapsed_ms = elapsed.as_millis(),
                    "Batch OCR completed"
                );
            }
            Err(error) => {
                tracing::error!(
                    target: TRACING_TARGET,
                    error = %error,
                    elapsed_ms = elapsed.as_millis(),
                    "Batch OCR failed"
                );
            }
        }

        result
    }

    /// Perform a health check on the OCR service.
    pub async fn health_check(&self) -> Result<ServiceHealth> {
        self.inner.health_check().await
    }
}
