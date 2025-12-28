//! Vision Language Model (VLM) abstractions.
//!
//! This module provides traits and types for working with multimodal AI models that
//! can process both images and text. It supports visual question answering, image
//! description, and multimodal conversations.
//!
//! # Example
//!
//! ```rust,ignore
//! use nvisy_core::vlm::{VlmProvider, VlmService, Request};
//! use nvisy_core::types::SharedContext;
//!
//! // Create a service with your provider
//! let service = VlmService::new(my_provider);
//!
//! // Process a VLM request
//! let request = Request::new("Describe this image").with_document(document);
//! let response = service.process_vlm(&request).await?;
//!
//! println!("Response: {}", response.content());
//! ```

pub mod request;
pub mod response;
pub mod service;

pub use request::{BatchRequest, Request};
pub use response::{BatchResponse, Response, Usage};
pub use service::VlmService;

use crate::Result;
use crate::types::{ServiceHealth, SharedContext};

/// Tracing target for VLM operations.
pub const TRACING_TARGET: &str = "nvisy_core::vlm";

/// Core trait for VLM (Vision Language Model) operations.
///
/// Implement this trait to create custom VLM providers. The trait provides
/// a default batch implementation that processes requests concurrently.
///
/// # Example
///
/// ```rust,ignore
/// use nvisy_core::vlm::{VlmProvider, Request, Response};
/// use nvisy_core::types::{ServiceHealth, SharedContext};
/// use nvisy_core::Result;
///
/// struct MyProvider;
///
/// #[async_trait::async_trait]
/// impl VlmProvider for MyProvider {
///     async fn process_vlm(
///         &self,
///         context: &SharedContext,
///         request: &Request,
///     ) -> Result<Response> {
///         let content = "Description of the image"; // Your VLM logic
///         Ok(request.reply(content))
///     }
///
///     async fn health_check(&self) -> Result<ServiceHealth> {
///         Ok(ServiceHealth::healthy())
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait VlmProvider: Send + Sync {
    /// Process a vision-language request and return a response.
    ///
    /// # Parameters
    ///
    /// * `context` - Shared context for tracking usage statistics
    /// * `request` - The VLM request containing images and prompts
    ///
    /// # Returns
    ///
    /// Returns a `Response` containing the model's output and metadata.
    async fn process_vlm(&self, context: &SharedContext, request: &Request) -> Result<Response>;

    /// Process a batch of VLM requests.
    ///
    /// The default implementation processes requests concurrently using `futures::join_all`.
    /// Providers can override this for optimized batch processing.
    ///
    /// # Error Handling
    ///
    /// Returns an error if any request in the batch fails. For partial failure tolerance,
    /// override this method with custom logic.
    async fn process_vlm_batch(
        &self,
        context: &SharedContext,
        request: &BatchRequest,
    ) -> Result<BatchResponse> {
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

        Ok(BatchResponse::new(responses))
    }

    /// Perform a health check on the VLM service.
    ///
    /// # Returns
    ///
    /// Returns `ServiceHealth` indicating the current status of the provider.
    async fn health_check(&self) -> Result<ServiceHealth>;
}
