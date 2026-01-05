#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod annotation;
mod content;
mod context;
mod document;
mod message;
mod service;

pub mod embedding;
pub mod language;
pub mod optical;

pub use annotation::{
    Annotation, AnnotationRelation, AnnotationSet, AnnotationType, BoundingBox, RelationType,
    TextSpan,
};
pub use content::Content;
pub use context::{Context, SharedContext, UsageStats};
pub use document::{Document, DocumentId, DocumentMetadata};
pub use embedding::{
    EmbeddingBatchRequest, EmbeddingBatchResponse, EmbeddingFormat, EmbeddingRequest,
    EmbeddingResponse,
};
pub use language::{VlmBatchRequest, VlmBatchResponse, VlmRequest, VlmResponse, VlmUsage};
pub use message::{Chat, Message, MessageRole};
pub use nvisy_core::types::{ServiceHealth, ServiceStatus, Timing};
pub use nvisy_core::{Error, ErrorKind, Result};
pub use optical::{OcrBatchRequest, OcrBatchResponse, OcrRequest, OcrResponse, TextExtraction};
pub use service::InferenceService;

/// Tracing target for inference operations.
pub const TRACING_TARGET: &str = "nvisy_inference";

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
