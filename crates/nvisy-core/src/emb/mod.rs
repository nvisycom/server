//! Embeddings service abstractions.
//!
//! This module provides traits and types for generating embeddings from text,
//! documents, and other content. It supports various embedding operations including
//! text embedding, multimodal embedding, and batch processing.

pub mod context;
pub mod request;
pub mod response;
pub mod service;

pub use context::{Context, ProcessingOptions};
pub use request::{ContentEmbeddingRequest, EncodingFormat, Request, RequestOptions};
pub use response::{BatchResponse, BatchStats, EmbeddingData, EmbeddingResult, Response};
pub use service::EmbeddingService;

use crate::types::ServiceHealth;
pub use crate::{Error, ErrorKind, Result};

/// Type alias for a boxed embedding provider with specific request and response types.
pub type BoxedEmbeddingProvider<Req = (), Resp = ()> =
    Box<dyn EmbeddingProvider<Req, Resp> + Send + Sync>;

/// Tracing target for embedding operations.
pub const TRACING_TARGET: &str = "nvisy_core::emb";

/// Core trait for embedding operations.
///
/// This trait is generic over request (`Req`) and response (`Resp`) types,
/// allowing implementations to define their own specific data structures
/// while maintaining a consistent interface.
///
/// # Type Parameters
///
/// * `Req` - The request payload type specific to the embedding implementation
/// * `Resp` - The response payload type specific to the embedding implementation
#[async_trait::async_trait]
pub trait EmbeddingProvider<Req, Resp>: Send + Sync {
    /// Generate embeddings for the provided input.
    ///
    /// This method takes ownership of the request to allow efficient processing
    /// without unnecessary cloning.
    ///
    /// # Parameters
    ///
    /// * `request` - Embedding request containing the input and processing options
    ///
    /// # Returns
    ///
    /// Returns a `Response<Resp>` containing the generated embeddings and metadata.
    async fn generate_embedding(&self, request: Request<Req>) -> Result<Response<Resp>>;

    /// Perform a health check on the embedding service.
    ///
    /// # Returns
    ///
    /// Returns service health information including status, response time, and metrics.
    async fn health_check(&self) -> Result<ServiceHealth>;
}
