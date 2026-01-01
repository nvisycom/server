//! AI inference abstractions for embeddings, OCR, and vision language models.
//!
//! This module provides unified traits and types for AI inference operations:
//! - **Embeddings**: Generate vector embeddings from text, documents, and chats
//! - **OCR**: Extract text from images and documents
//! - **VLM**: Vision-language model operations for multimodal AI
//!
//! # Example
//!
//! ```rust,ignore
//! use nvisy_service::inference::{
//!     InferenceProvider, InferenceService,
//!     EmbeddingRequest, OcrRequest, VlmRequest,
//! };
//!
//! // Create a unified service with a provider
//! let service = InferenceService::from_provider(my_provider);
//!
//! // Use individual methods
//! let embedding = service.generate_embedding(&request).await?;
//! let ocr_result = service.process_ocr(&request).await?;
//! let vlm_result = service.process_vlm(&request).await?;
//! ```

#[cfg(feature = "test-utils")]
#[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
mod mock;
mod service;

pub mod request;
pub mod response;

#[cfg(feature = "test-utils")]
#[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
pub use mock::{MockConfig, MockProvider};
pub use request::{
    EmbeddingBatchRequest, EmbeddingRequest, OcrBatchRequest, OcrRequest, VlmBatchRequest,
    VlmRequest,
};
pub use response::{
    EmbeddingBatchResponse, EmbeddingFormat, EmbeddingResponse, OcrBatchResponse, OcrResponse,
    TextExtraction, VlmBatchResponse, VlmResponse, VlmUsage,
};
pub use service::InferenceService;

use crate::types::{ServiceHealth, SharedContext};
pub use crate::{Error, Result};

/// Tracing target for inference operations.
pub const TRACING_TARGET: &str = "nvisy_service::inference";

/// Unified trait for AI inference operations.
///
/// This trait provides embedding generation, OCR processing, and vision-language
/// model capabilities. Implement this trait to create custom inference providers.
///
/// Default batch implementations process requests concurrently.
#[async_trait::async_trait]
pub trait InferenceProvider: Send + Sync {
    /// Generate an embedding for the provided input.
    async fn generate_embedding(
        &self,
        context: &SharedContext,
        request: &EmbeddingRequest,
    ) -> Result<EmbeddingResponse>;

    /// Generate embeddings for a batch of inputs.
    ///
    /// The default implementation processes requests concurrently.
    async fn generate_embedding_batch(
        &self,
        context: &SharedContext,
        request: &EmbeddingBatchRequest,
    ) -> Result<EmbeddingBatchResponse> {
        let requests = request.iter_requests();
        let futures: Vec<_> = requests
            .iter()
            .map(|req| self.generate_embedding(context, req))
            .collect();

        let results = futures_util::future::join_all(futures).await;

        let mut responses = Vec::with_capacity(results.len());
        for result in results {
            responses.push(result?);
        }

        Ok(EmbeddingBatchResponse::new(responses))
    }

    /// Process an image or document with OCR to extract text.
    async fn process_ocr(
        &self,
        context: &SharedContext,
        request: &OcrRequest,
    ) -> Result<OcrResponse>;

    /// Process a batch of OCR requests.
    ///
    /// The default implementation processes requests concurrently.
    async fn process_ocr_batch(
        &self,
        context: &SharedContext,
        request: &OcrBatchRequest,
    ) -> Result<OcrBatchResponse> {
        let requests = request.iter_requests();
        let futures: Vec<_> = requests
            .iter()
            .map(|req| self.process_ocr(context, req))
            .collect();

        let results = futures_util::future::join_all(futures).await;

        let mut responses = Vec::with_capacity(results.len());
        for result in results {
            responses.push(result?);
        }

        Ok(OcrBatchResponse::new(responses))
    }

    /// Process a vision-language request and return a response.
    async fn process_vlm(
        &self,
        context: &SharedContext,
        request: &VlmRequest,
    ) -> Result<VlmResponse>;

    /// Process a batch of VLM requests.
    ///
    /// The default implementation processes requests concurrently.
    async fn process_vlm_batch(
        &self,
        context: &SharedContext,
        request: &VlmBatchRequest,
    ) -> Result<VlmBatchResponse> {
        let requests = request.iter_requests();
        let futures: Vec<_> = requests
            .iter()
            .map(|req| self.process_vlm(context, req))
            .collect();

        let results = futures_util::future::join_all(futures).await;

        let mut responses = Vec::with_capacity(results.len());
        for result in results {
            responses.push(result?);
        }

        Ok(VlmBatchResponse::new(responses))
    }

    /// Perform a health check on the inference service.
    async fn health_check(&self) -> Result<ServiceHealth>;
}
