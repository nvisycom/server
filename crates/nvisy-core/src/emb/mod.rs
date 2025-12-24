//! Embeddings service abstractions.
//!
//! This module provides foundational abstractions for embedding services in the Nvisy ecosystem.
//! It defines core traits and types for text and multimodal embedding operations without depending
//! on any concrete implementations.

pub mod context;
pub mod request;
pub mod response;
pub mod service;

pub use context::Context;
pub use request::{EmbeddingRequest, EncodingFormat};
pub use response::{EmbeddingData, EmbeddingResponse, EmbeddingUsage};
pub use service::EmbeddingService;

use crate::types::ServiceHealth;
pub use crate::{Error, ErrorKind, Result};

/// Type alias for a boxed embedding service with specific request and response types.
pub type BoxedEmbeddingProvider<Req, Resp> = Box<dyn EmbeddingProvider<Req, Resp> + Send + Sync>;

/// Tracing target for embedding operations.
pub const TRACING_TARGET: &str = "nvisy_core::emb";

/// Core trait for embedding service operations.
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
    /// Generates embeddings for the provided input.
    ///
    /// This method takes an [`EmbeddingRequest`] containing the input text/images
    /// and model configuration, and returns an [`EmbeddingResponse`] with the
    /// generated embeddings.
    ///
    /// # Parameters
    ///
    /// - `request`: The embedding request containing input and configuration
    ///
    /// # Returns
    ///
    /// A [`Result`] containing the embedding response on success, or an error
    /// if the operation failed. The response includes the embeddings, usage
    /// information, and metadata.
    async fn generate_embedding(&self, request: &EmbeddingRequest) -> Result<EmbeddingResponse>;

    /// Performs a health check on the embedding service.
    ///
    /// This method verifies that the service is reachable and properly configured.
    /// It's typically used for monitoring and diagnostics purposes.
    ///
    /// # Returns
    ///
    /// Returns service health information including status, response time, and metrics.
    async fn health_check(&self) -> Result<ServiceHealth>;
}
