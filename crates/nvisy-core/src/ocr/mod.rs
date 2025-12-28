//! Optical Character Recognition (OCR) abstractions.
//!
//! This module provides traits and types for extracting text from images
//! and documents. It supports single and batch OCR operations.
//!
//! # Example
//!
//! ```rust,ignore
//! use nvisy_core::ocr::{OcrProvider, OcrService, Request};
//! use nvisy_core::types::SharedContext;
//!
//! // Create a service with your provider
//! let service = OcrService::new(my_provider);
//!
//! // Process an OCR request
//! let request = Request::from_document(document);
//! let response = service.process_ocr(&request).await?;
//!
//! println!("Extracted text: {}", response.text());
//! ```

pub mod request;
pub mod response;
pub mod service;

pub use request::{BatchRequest, Request};
pub use response::{BatchResponse, Response, TextExtraction};
pub use service::OcrService;

use crate::Result;
use crate::types::{ServiceHealth, SharedContext};

/// Tracing target for OCR operations.
pub const TRACING_TARGET: &str = "nvisy_core::ocr";

/// Core trait for OCR operations.
///
/// Implement this trait to create custom OCR providers. The trait provides
/// a default batch implementation that processes requests concurrently.
///
/// # Example
///
/// ```rust,ignore
/// use nvisy_core::ocr::{OcrProvider, Request, Response};
/// use nvisy_core::types::{ServiceHealth, SharedContext};
/// use nvisy_core::Result;
///
/// struct MyProvider;
///
/// #[async_trait::async_trait]
/// impl OcrProvider for MyProvider {
///     async fn process_ocr(
///         &self,
///         context: &SharedContext,
///         request: &Request,
///     ) -> Result<Response> {
///         let text = "Extracted text"; // Your OCR logic
///         Ok(request.reply(text))
///     }
///
///     async fn health_check(&self) -> Result<ServiceHealth> {
///         Ok(ServiceHealth::healthy())
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait OcrProvider: Send + Sync {
    /// Process an image or document with OCR to extract text.
    ///
    /// # Parameters
    ///
    /// * `context` - Shared context for tracking usage statistics
    /// * `request` - The OCR request containing the document to process
    ///
    /// # Returns
    ///
    /// Returns a `Response` containing the extracted text and metadata.
    async fn process_ocr(&self, context: &SharedContext, request: &Request) -> Result<Response>;

    /// Process a batch of OCR requests.
    ///
    /// The default implementation processes requests concurrently using `futures::join_all`.
    /// Providers can override this for optimized batch processing.
    ///
    /// # Error Handling
    ///
    /// Returns an error if any request in the batch fails. For partial failure tolerance,
    /// override this method with custom logic.
    async fn process_ocr_batch(
        &self,
        context: &SharedContext,
        request: &BatchRequest,
    ) -> Result<BatchResponse> {
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

        Ok(BatchResponse::new(responses))
    }

    /// Perform a health check on the OCR service.
    ///
    /// # Returns
    ///
    /// Returns `ServiceHealth` indicating the current status of the provider.
    async fn health_check(&self) -> Result<ServiceHealth>;
}
