//! Embeddings service abstractions.
//!
//! This module provides foundational abstractions for embedding services in the Nvisy ecosystem.
//! It defines core traits and types for text and multimodal embedding operations without depending
//! on any concrete implementations.
//!
//! # Overview
//!
//! The embeddings module supports various types of embeddings:
//!
//! - **Text Embeddings**: Convert text into dense vector representations
//! - **Image Embeddings**: Convert images into dense vector representations
//! - **Multimodal Embeddings**: Convert text and images into aligned vector spaces
//!
//! # Architecture
//!
//! The module follows a layered architecture:
//!
//! - **Service Layer**: [`EmbeddingService`] trait for embedding operations
//! - **Request/Response Layer**: Structured types for embedding requests and responses
//! - **Error Handling**: Comprehensive error types with retry policies
//! - **Context Management**: Request context and configuration management
//!
//! # Error Handling
//!
//! All embedding operations return [`Result<T, Error>`](Result) where errors are classified
//! into retryable and non-retryable categories with appropriate retry policies.
//!
//! # Example
//!
//! ```rust,ignore
//! use nvisy_core::emb::{EmbeddingService, EmbeddingRequest};
//!
//! async fn generate_embeddings(
//!     service: &impl EmbeddingService,
//!     text: &str,
//! ) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
//!     let request = EmbeddingRequest::builder()
//!         .input(text)
//!         .model("text-embedding-ada-002")
//!         .build()?;
//!
//!     let response = service.embed(request).await?;
//!     Ok(response.embeddings[0].embedding.clone())
//! }
//! ```

use std::sync::Arc;

use async_trait::async_trait;

pub mod context;
pub mod error;
pub mod request;
pub mod response;
pub mod service;

pub use context::EmbeddingContext;
pub use error::{Error, ErrorKind, Result};
pub use request::{EmbeddingInput, EmbeddingRequest};
pub use response::{EmbeddingData, EmbeddingResponse, EmbeddingResponseBuilder, EmbeddingUsage};
pub use service::Service as EmbeddingService;

use crate::types::ServiceHealth;

/// Type alias for a boxed embedding service with specific request and response types.
pub type BoxedEmbedding<Req, Resp> = Arc<dyn EmbeddingProvider<Req, Resp> + Send + Sync>;

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
#[async_trait]
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
    async fn embed(&self, request: &EmbeddingRequest) -> Result<EmbeddingResponse>;

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
