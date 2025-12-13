//! Optical Character Recognition (OCR) abstractions.
//!
//! This module provides traits and types for extracting structured text from images
//! and documents. It supports various OCR capabilities including image text extraction,
//! document processing, and structured output.
//!
//! # Generic Design
//!
//! The OCR trait is generic over request and response types, allowing different
//! implementations to use their own specific formats while maintaining a consistent
//! interface:
//!
//! ```rust,ignore
//! use nvisy_core::ocr::{Ocr, Request, Response};
//!
//! // Generic OCR implementation
//! struct MyOcr;
//!
//! impl Ocr<MyRequest, MyResponse> for MyOcr {
//!     async fn process_with_ocr(&self, request: Request<MyRequest>) -> Result<Response<MyResponse>> {
//!         // Implementation
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

pub use context::{Context, Document, ProcessingOptions};
pub use error::{Error, Result};
pub use request::Request;
pub use response::Response;
pub use service::OcrService;

use crate::health::ServiceHealth;

/// Type alias for a boxed OCR service with specific request and response types.
pub type BoxedOcr<Req, Resp> = Arc<dyn Ocr<Req, Resp> + Send + Sync>;

/// Type alias for boxed response stream.
pub type BoxedStream<T> = Box<dyn Stream<Item = std::result::Result<T, Error>> + Send + Unpin>;

/// Core trait for OCR operations.
///
/// This trait is generic over request (`Req`) and response (`Resp`) types,
/// allowing implementations to define their own specific data structures
/// while maintaining a consistent interface.
///
/// # Type Parameters
///
/// * `Req` - The request payload type specific to the OCR implementation
/// * `Resp` - The response payload type specific to the OCR implementation
pub trait Ocr<Req, Resp> {
    /// Process an image or document with OCR to extract text and structured data.
    ///
    /// This method takes ownership of the request to allow efficient processing
    /// without unnecessary cloning.
    ///
    /// # Parameters
    ///
    /// * `request` - OCR request containing the image/document and processing options
    ///
    /// # Returns
    ///
    /// Returns a `Response<Resp>` containing the extracted text, regions, and metadata.
    fn process_with_ocr(
        &self,
        request: Request<Req>,
    ) -> impl Future<Output = Result<Response<Resp>>> + Send;

    /// Process an image or document with OCR using streaming responses.
    ///
    /// This method returns a stream of partial responses, allowing for real-time
    /// processing of long-running OCR operations or large documents.
    ///
    /// # Parameters
    ///
    /// * `request` - OCR request containing the image/document and processing options
    ///
    /// # Returns
    ///
    /// Returns a stream of `Response<Resp>` chunks that can be consumed incrementally.
    fn process_stream(
        &self,
        request: Request<Req>,
    ) -> impl Future<Output = Result<BoxedStream<Response<Resp>>>> + Send;

    /// Perform a health check on the OCR service.
    ///
    /// # Returns
    ///
    /// Returns service health information including status, response time, and metrics.
    fn health_check(&self) -> impl Future<Output = Result<ServiceHealth>> + Send;
}
