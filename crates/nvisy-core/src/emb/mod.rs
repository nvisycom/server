//! Embeddings service abstractions.
//!
//! This module provides traits and types for generating embeddings from text,
//! documents, and other content. It supports single and batch embedding operations.
//!
//! # Example
//!
//! ```rust,ignore
//! use nvisy_core::emb::{EmbeddingProvider, EmbeddingService, Request};
//! use nvisy_core::types::SharedContext;
//!
//! // Create a service with your provider
//! let service = EmbeddingService::new(my_provider);
//!
//! // Generate an embedding
//! let request = Request::from_text("Hello, world!");
//! let response = service.generate_embedding(&request).await?;
//!
//! println!("Embedding dimensions: {}", response.dimensions());
//! ```

pub mod request;
pub mod response;
pub mod service;

pub use request::{BatchRequest, Request};
pub use response::{BatchResponse, EncodingFormat, Response};
pub use service::EmbeddingService;

use crate::Result;
use crate::types::{ServiceHealth, SharedContext};

/// Tracing target for embedding operations.
pub const TRACING_TARGET: &str = "nvisy_core::embedding";

/// Core trait for embedding operations.
///
/// Implement this trait to create custom embedding providers. The trait provides
/// a default batch implementation that processes requests concurrently.
///
/// # Example
///
/// ```rust,ignore
/// use nvisy_core::emb::{EmbeddingProvider, Request, Response};
/// use nvisy_core::types::{ServiceHealth, SharedContext};
/// use nvisy_core::Result;
///
/// struct MyProvider;
///
/// #[async_trait::async_trait]
/// impl EmbeddingProvider for MyProvider {
///     async fn generate_embedding(
///         &self,
///         context: &SharedContext,
///         request: &Request,
///     ) -> Result<Response> {
///         let embedding = vec![0.1, 0.2, 0.3]; // Your embedding logic
///         Ok(request.reply(embedding))
///     }
///
///     async fn health_check(&self) -> Result<ServiceHealth> {
///         Ok(ServiceHealth::healthy())
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Generate an embedding for the provided input.
    ///
    /// # Parameters
    ///
    /// * `context` - Shared context for tracking usage statistics
    /// * `request` - The embedding request containing content to embed
    ///
    /// # Returns
    ///
    /// Returns a `Response` containing the embedding vector and metadata.
    async fn generate_embedding(
        &self,
        context: &SharedContext,
        request: &Request,
    ) -> Result<Response>;

    /// Generate embeddings for a batch of inputs.
    ///
    /// The default implementation processes requests concurrently using `futures::join_all`.
    /// Providers can override this for optimized batch processing (e.g., single API call).
    ///
    /// # Error Handling
    ///
    /// Returns an error if any request in the batch fails. For partial failure tolerance,
    /// override this method with custom logic.
    async fn generate_embedding_batch(
        &self,
        context: &SharedContext,
        request: &BatchRequest,
    ) -> Result<BatchResponse> {
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

        Ok(BatchResponse::new(responses))
    }

    /// Perform a health check on the embedding service.
    ///
    /// # Returns
    ///
    /// Returns `ServiceHealth` indicating the current status of the provider.
    async fn health_check(&self) -> Result<ServiceHealth>;
}
