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

use std::sync::Arc;

use async_trait::async_trait;
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
pub use service::Service as VlmService;

use crate::health::ServiceHealth;

/// Type alias for a boxed VLM service with specific request and response types.
pub type Boxed<Req, Resp> = Arc<dyn Vlm<Req, Resp> + Send + Sync>;

/// Type alias for boxed response stream.
pub type BoxedStream<T> = Box<dyn Stream<Item = std::result::Result<T, Error>> + Send + Unpin>;

/// Core trait for VLM (Vision Language Model) operations.
///
/// This trait is generic over request (`Req`) and response (`Resp`) types,
/// allowing implementations to define their own specific data structures
/// while maintaining a consistent interface.
///
/// # Type Parameters
///
/// * `Req` - The request payload type specific to the VLM implementation
/// * `Resp` - The response payload type specific to the VLM implementation
#[async_trait]
pub trait Vlm<Req, Resp>: Send + Sync {
    /// Process a vision-language request and return a complete response.
    ///
    /// # Parameters
    ///
    /// * `request` - VLM request containing images, text prompts, and configuration
    ///
    /// # Returns
    ///
    /// Returns a complete `Response<Resp>` with the model's output.
    async fn process(&self, request: &Request<Req>) -> Result<Response<Resp>>;

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
    /// Returns a stream of `Response<Resp>` chunks that can be consumed incrementally.
    async fn process_stream(&self, request: &Request<Req>) -> Result<BoxedStream<Response<Resp>>>;

    /// Perform a health check on the VLM service.
    ///
    /// # Returns
    ///
    /// Returns service health information including status, response time, and metrics.
    async fn health_check(&self) -> Result<ServiceHealth>;
}
