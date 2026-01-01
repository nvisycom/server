//! Unified inference service with observability.
//!
//! This module provides [`InferenceService`] which wraps inference providers
//! and adds production-ready logging and tracing.

use std::fmt;
use std::sync::Arc;

use jiff::Timestamp;

use super::{
    EmbeddingBatchRequest, EmbeddingBatchResponse, EmbeddingRequest, EmbeddingResponse,
    InferenceProvider, OcrBatchRequest, OcrBatchResponse, OcrRequest, OcrResponse, Result,
    TRACING_TARGET, VlmBatchRequest, VlmBatchResponse, VlmRequest, VlmResponse,
};
use crate::types::{Context, ServiceHealth, SharedContext};

/// Unified inference service with observability.
///
/// This service wraps any provider implementing [`InferenceProvider`] and adds
/// structured logging for all operations.
#[derive(Clone)]
pub struct InferenceService {
    provider: Arc<dyn InferenceProvider>,
    context: SharedContext,
}

impl fmt::Debug for InferenceService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InferenceService")
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}

impl InferenceService {
    /// Create a new inference service from a provider.
    pub fn from_provider<P>(provider: P) -> Self
    where
        P: InferenceProvider + 'static,
    {
        Self {
            provider: Arc::new(provider),
            context: SharedContext::new(),
        }
    }

    /// Create a new inference service from a provider with shared context.
    pub fn from_provider_with_context<P>(provider: P, context: SharedContext) -> Self
    where
        P: InferenceProvider + 'static,
    {
        Self {
            provider: Arc::new(provider),
            context,
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

    /// Create a new service with a specific context.
    pub fn with_context(mut self, context: Context) -> Self {
        self.context = SharedContext::from_context(context);
        self
    }

    /// Generate an embedding for the provided input.
    pub async fn generate_embedding(
        &self,
        request: &EmbeddingRequest,
    ) -> Result<EmbeddingResponse> {
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            "Processing embedding request"
        );

        let result = self
            .provider
            .generate_embedding(&self.context, request)
            .await;
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
    pub async fn generate_embedding_batch(
        &self,
        request: &EmbeddingBatchRequest,
    ) -> Result<EmbeddingBatchResponse> {
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET,
            batch_size = request.len(),
            "Processing batch embedding request"
        );

        let result = self
            .provider
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

    /// Process an image or document with OCR.
    pub async fn process_ocr(&self, request: &OcrRequest) -> Result<OcrResponse> {
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            document_size = request.document_size(),
            "Processing OCR request"
        );

        let result = self.provider.process_ocr(&self.context, request).await;
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
    pub async fn process_ocr_batch(&self, request: &OcrBatchRequest) -> Result<OcrBatchResponse> {
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET,
            batch_size = request.len(),
            "Processing batch OCR request"
        );

        let result = self
            .provider
            .process_ocr_batch(&self.context, request)
            .await;
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

    /// Process a vision-language request.
    pub async fn process_vlm(&self, request: &VlmRequest) -> Result<VlmResponse> {
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            document_count = request.document_count(),
            prompt_length = request.prompt_length(),
            "Processing VLM request"
        );

        let result = self.provider.process_vlm(&self.context, request).await;
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
    pub async fn process_vlm_batch(&self, request: &VlmBatchRequest) -> Result<VlmBatchResponse> {
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET,
            batch_size = request.len(),
            total_documents = request.total_documents(),
            "Processing batch VLM request"
        );

        let result = self
            .provider
            .process_vlm_batch(&self.context, request)
            .await;
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

    /// Perform a health check on the inference service.
    pub async fn health_check(&self) -> Result<ServiceHealth> {
        self.provider.health_check().await
    }

    /// Create a mock inference service for testing.
    #[cfg(feature = "test-utils")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
    pub fn mock() -> Self {
        Self::from_provider(super::MockProvider::default())
    }

    /// Create a mock inference service with custom configuration.
    #[cfg(feature = "test-utils")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
    pub fn mock_with_config(config: super::MockConfig) -> Self {
        Self::from_provider(super::MockProvider::new(config))
    }
}
