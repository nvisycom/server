//! Vision Language Model (VLM) abstractions.
//!
//! This module provides traits and types for working with multimodal AI models that
//! can process both images and text. It supports various VLM capabilities including
//! visual question answering, image description, visual reasoning, and multimodal
//! conversations.
//!
//! # Example
//!
//! ```rust,ignore
//! use nvisy_core::vlm::{Vlm, Request, Response};
//!
//! struct MyVlm;
//!
//! impl Vlm for MyVlm {
//!     async fn process(&self, request: &Request) -> Result<Response> {
//!         // Implementation
//!     }
//!
//!     async fn process_stream(&self, request: &Request) -> Result<BoxedStream<Response>> {
//!         // Implementation
//!     }
//!
//!     async fn list_models(&self) -> Result<Vec<ModelInfo>> {
//!         // Implementation
//!     }
//!
//!     fn service_name(&self) -> &str {
//!         "my-vlm"
//!     }
//!
//!     async fn health_check(&self) -> Result<ServiceHealth> {
//!         // Implementation
//!     }
//! }
//! ```

use std::future::Future;
use std::sync::Arc;

use futures_util::Stream;

pub mod context;
pub mod error;
pub mod request;
pub mod response;
pub mod service;

pub use context::{Context, ProcessingOptions};
pub use error::{Error, Result};
pub use request::Request;
pub use response::Response;
pub use service::Service;

use crate::health::ServiceHealth;

/// Type alias for a boxed VLM service.
pub type Boxed = Arc<dyn Vlm + Send + Sync>;

/// Type alias for boxed response stream.
pub type BoxedStream<T> = Box<dyn Stream<Item = std::result::Result<T, Error>> + Send + Unpin>;

/// Core trait for VLM (Vision Language Model) operations.
///
/// This trait defines the interface for multimodal AI services that can process
/// both images and text. Implementations should provide both streaming and
/// non-streaming variants of request processing.
pub trait Vlm {
    /// Process a vision-language request and return a complete response.
    ///
    /// # Parameters
    ///
    /// * `request` - VLM request containing images, text prompts, and configuration
    ///
    /// # Returns
    ///
    /// Returns a complete `Response` with the model's output.
    fn process(&self, request: &Request) -> impl Future<Output = Result<Response>> + Send;

    /// Process a request with streaming response.
    ///
    /// This method returns a stream of partial responses, allowing for real-time
    /// processing of long-running requests.
    ///
    /// # Parameters
    ///
    /// * `request` - VLM request containing images, text prompts, and configuration
    ///
    /// # Returns
    ///
    /// Returns a stream of `Response` chunks that can be consumed incrementally.
    fn process_stream(
        &self,
        request: &Request,
    ) -> impl Future<Output = Result<BoxedStream<Response>>> + Send;

    /// Perform a health check on the VLM service.
    ///
    /// # Returns
    ///
    /// Returns service health information including status, response time, and metrics.
    fn health_check(&self) -> impl Future<Output = Result<ServiceHealth>> + Send;
}
