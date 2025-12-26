//! Vision Language Model (VLM) abstractions.
//!
//! This module provides traits and types for working with multimodal AI models that
//! can process both images and text. It supports various VLM capabilities including
//! visual question answering, image description, visual reasoning, and multimodal
//! conversations.

use futures_util::Stream;

pub mod request;
pub mod response;
pub mod service;

pub use request::{ImageInput, Request, RequestOptions, VlmInput};
pub use response::{
    ColorInfo, DetectedObject, EmotionalAnalysis, FontProperties, ImageProperties, Response,
    ResponseMetadata, SceneCategory, TextRegion, Usage, VisualAnalysis, VlmResponseChunk,
};
pub use service::VlmService;

use crate::types::{ServiceHealth, SharedContext};
use crate::{Error, Result};

/// Type alias for a boxed VLM service with specific request and response types.
pub type BoxedVlmProvider<Req, Resp> = Box<dyn VlmProvider<Req, Resp> + Send + Sync>;

/// Type alias for boxed response stream.
pub type BoxedStream<T> = Box<dyn Stream<Item = std::result::Result<T, Error>> + Send + Unpin>;

/// Tracing target for VLM operations.
pub const TRACING_TARGET: &str = "nvisy_core::vlm";

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
#[async_trait::async_trait]
pub trait VlmProvider<Req, Resp>: Send + Sync {
    /// Process a vision-language request and return a complete response.
    ///
    /// # Parameters
    ///
    /// * `context` - VLM shared context with session information and usage tracking
    /// * `request` - VLM request containing images, text prompts, and configuration
    ///
    /// # Returns
    ///
    /// Returns a complete `Response<Resp>` with the model's output.
    async fn process_vlm(
        &self,
        context: &SharedContext,
        request: &Request<Req>,
    ) -> Result<Response<Resp>>;

    /// Process a request with streaming response.
    ///
    /// This method returns a stream of partial responses, allowing for real-time
    /// processing of long-running requests.
    ///
    /// # Parameters
    ///
    /// * `context` - VLM shared context with session information and usage tracking
    /// * `request` - VLM request containing images, text prompts, and configuration
    ///
    /// # Returns
    ///
    /// Returns a stream of `Response<Resp>` chunks that can be consumed incrementally.
    async fn process_vlm_stream(
        &self,
        context: &SharedContext,
        request: &Request<Req>,
    ) -> Result<BoxedStream<Response<Resp>>>;

    /// Perform a health check on the VLM service.
    ///
    /// # Returns
    ///
    /// Returns service health information including status, response time, and metrics.
    async fn health_check(&self) -> Result<ServiceHealth>;
}
