//! Unified inference service with observability.
//!
//! This module provides [`InferenceService`] which wraps inference providers
//! and adds production-ready logging and tracing.

use std::fmt;
use std::sync::Arc;

use jiff::Timestamp;
use nvisy_core::Result;
use nvisy_core::types::ServiceHealth;

use super::{Context, InferenceProvider};
use crate::TRACING_TARGET;
use crate::embedding::{
    EmbeddingBatchRequest, EmbeddingBatchResponse, EmbeddingProviderExt, EmbeddingRequest,
    EmbeddingResponse,
};
use crate::language::{
    LanguageProviderExt, VlmBatchRequest, VlmBatchResponse, VlmRequest, VlmResponse,
};
use crate::optical::{
    OcrBatchRequest, OcrBatchResponse, OcrRequest, OcrResponse, OpticalProviderExt,
};

/// Unified inference service with observability.
///
/// This service wraps any provider implementing [`InferenceProvider`] and adds
/// structured logging for all operations.
#[derive(Clone)]
pub struct InferenceService {
    provider: Arc<dyn InferenceProvider>,
}

impl fmt::Debug for InferenceService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InferenceService").finish_non_exhaustive()
    }
}

impl InferenceService {
    /// Create a new inference service from a provider.
    pub fn new<P>(provider: P) -> Self
    where
        P: InferenceProvider + 'static,
    {
        Self {
            provider: Arc::new(provider),
        }
    }

    /// Generate an embedding for the provided input.
    pub async fn generate_embedding(
        &self,
        context: &Context,
        request: &EmbeddingRequest,
    ) -> Result<EmbeddingResponse> {
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            workspace_id = %context.workspace_id,
            "Processing embedding request"
        );

        let result = self.provider.generate_embedding(context, request).await;
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
        context: &Context,
        request: &EmbeddingBatchRequest,
    ) -> Result<EmbeddingBatchResponse> {
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET,
            batch_size = request.len(),
            workspace_id = %context.workspace_id,
            "Processing batch embedding request"
        );

        let result = self
            .provider
            .generate_embedding_batch(context, request)
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
    pub async fn process_ocr(
        &self,
        context: &Context,
        request: &OcrRequest,
    ) -> Result<OcrResponse> {
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            document_size = request.document_size(),
            workspace_id = %context.workspace_id,
            "Processing OCR request"
        );

        let result = self.provider.process_ocr(context, request).await;
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
    pub async fn process_ocr_batch(
        &self,
        context: &Context,
        request: &OcrBatchRequest,
    ) -> Result<OcrBatchResponse> {
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET,
            batch_size = request.len(),
            workspace_id = %context.workspace_id,
            "Processing batch OCR request"
        );

        let result = self.provider.process_ocr_batch(context, request).await;
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
    pub async fn process_vlm(
        &self,
        context: &Context,
        request: &VlmRequest,
    ) -> Result<VlmResponse> {
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET,
            request_id = %request.request_id,
            document_count = request.document_count(),
            prompt_length = request.prompt_length(),
            workspace_id = %context.workspace_id,
            "Processing VLM request"
        );

        let result = self.provider.process_vlm(context, request).await;
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
    pub async fn process_vlm_batch(
        &self,
        context: &Context,
        request: &VlmBatchRequest,
    ) -> Result<VlmBatchResponse> {
        let started_at = Timestamp::now();

        tracing::debug!(
            target: TRACING_TARGET,
            batch_size = request.len(),
            total_documents = request.total_documents(),
            workspace_id = %context.workspace_id,
            "Processing batch VLM request"
        );

        let result = self.provider.process_vlm_batch(context, request).await;
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
}
