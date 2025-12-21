//! Optical Character Recognition (OCR) abstractions.
//!
//! This module provides traits and types for extracting structured text from images
//! and documents. It supports various OCR capabilities including image text extraction,
//! document processing, and structured output.

use std::sync::Arc;

use futures_util::Stream;

pub mod context;

pub mod request;
pub mod response;
pub mod service;

pub use context::{Context, ProcessingOptions};
pub use request::{BoundingBox, DocumentOcrRequest, Request, RequestOptions};
pub use response::{BatchResponse, BatchStats, OcrResult, Response, TextExtraction};
pub use service::OcrService;

use crate::types::ServiceHealth;
pub use crate::{Error, ErrorKind, Result};

/// Type alias for a boxed OCR service with specific request and response types.
pub type BoxedOcrProvider<Req, Resp> = Arc<dyn OcrProvider<Req, Resp> + Send + Sync>;

/// Type alias for boxed response stream.
pub type BoxedStream<T> = Box<dyn Stream<Item = std::result::Result<T, Error>> + Send + Unpin>;

/// Tracing target for OCR operations.
pub const TRACING_TARGET: &str = "nvisy_core::ocr";

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
#[async_trait::async_trait]
pub trait OcrProvider<Req, Resp>: Send + Sync {
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
    async fn process_ocr(&self, request: Request<Req>) -> Result<Response<Resp>>;

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
    async fn process_ocr_stream(
        &self,
        request: Request<Req>,
    ) -> Result<BoxedStream<Response<Resp>>>;

    /// Perform a health check on the OCR service.
    ///
    /// # Returns
    ///
    /// Returns service health information including status, response time, and metrics.
    async fn health_check(&self) -> Result<ServiceHealth>;
}
