//! Request types for OCR operations.
//!
//! The `Request<Req>` type is a generic wrapper that allows OCR implementations
//! to define their own request payload types while maintaining a consistent
//! interface for common metadata like request IDs and options.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Generic request for OCR operations.
///
/// This wrapper type provides common metadata and configuration while allowing
/// implementations to define their own specific request payload type.
///
/// # Type Parameters
///
/// * `Req` - The implementation-specific request payload type
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Debug, Clone)]
/// struct MyOcrRequest {
///     image_data: Vec<u8>,
///     mime_type: String,
/// }
///
/// let request = Request::new(MyOcrRequest {
///     image_data: image_bytes,
///     mime_type: "image/jpeg".to_string(),
/// });
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request<Req> {
    /// Unique identifier for this request.
    pub request_id: Uuid,
    /// Implementation-specific request payload.
    pub payload: Req,
    /// Processing options.
    pub options: RequestOptions,
}

/// Processing options for OCR requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestOptions {
    /// Whether to preserve layout information.
    pub preserve_layout: bool,
    /// Minimum confidence threshold for text extraction.
    pub confidence_threshold: Option<f32>,
    /// DPI setting for image processing.
    pub dpi: Option<u32>,
}

impl Default for RequestOptions {
    fn default() -> Self {
        Self {
            preserve_layout: true,
            confidence_threshold: Some(0.5),
            dpi: Some(300),
        }
    }
}

impl<Req> Request<Req> {
    /// Create a new OCR request with the given payload.
    pub fn new(payload: Req) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            payload,
            options: RequestOptions::default(),
        }
    }

    /// Create a new request with custom options.
    pub fn with_options(payload: Req, options: RequestOptions) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            payload,
            options,
        }
    }

    /// Set whether to preserve layout.
    pub fn with_layout_preservation(mut self, preserve_layout: bool) -> Self {
        self.options.preserve_layout = preserve_layout;
        self
    }

    /// Set confidence threshold.
    pub fn with_confidence_threshold(mut self, threshold: f32) -> Self {
        self.options.confidence_threshold = Some(threshold);
        self
    }

    /// Set DPI for processing.
    pub fn with_dpi(mut self, dpi: u32) -> Self {
        self.options.dpi = Some(dpi);
        self
    }
}
