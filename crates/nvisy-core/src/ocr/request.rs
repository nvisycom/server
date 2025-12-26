//! Request types for OCR operations.
//!
//! The `Request<Req>` type is a generic wrapper that allows OCR implementations
//! to define their own request payload types while maintaining a consistent
//! interface for common metadata like request IDs and options.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::Document;

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
/// ```rust
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

    /// Create a new OCR request with a specific request ID.
    pub fn with_request_id(request_id: Uuid, payload: Req) -> Self {
        Self {
            request_id,
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

/// Standard OCR request using Document input.
///
/// This is a convenience type for OCR operations that work directly with Document inputs,
/// providing a standard interface while maintaining compatibility with the generic Request type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentRequest {
    /// The document to process.
    pub document: Document,
    /// Optional region of interest within the document.
    pub region: Option<BoundingBox>,
    /// Language hint for OCR processing (ISO 639-1 code).
    pub language: Option<String>,
}

/// Bounding box for specifying regions within images.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    /// X coordinate of the top-left corner.
    pub x: f32,
    /// Y coordinate of the top-left corner.
    pub y: f32,
    /// Width of the bounding box.
    pub width: f32,
    /// Height of the bounding box.
    pub height: f32,
}

impl BoundingBox {
    /// Create a new bounding box.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Check if this bounding box is valid (non-negative dimensions).
    pub fn is_valid(&self) -> bool {
        self.width >= 0.0 && self.height >= 0.0
    }

    /// Get the area of this bounding box.
    pub fn area(&self) -> f32 {
        self.width * self.height
    }
}

impl DocumentRequest {
    /// Create a new document OCR request.
    pub fn new(document: Document) -> Self {
        Self {
            document,
            region: None,
            language: None,
        }
    }

    /// Set the region of interest for processing.
    pub fn with_region(mut self, region: BoundingBox) -> Self {
        self.region = Some(region);
        self
    }

    /// Set the language hint for OCR processing.
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Check if this request has a specific region of interest.
    pub fn has_region(&self) -> bool {
        self.region.is_some()
    }

    /// Get the content type of the document.
    pub fn content_type(&self) -> Option<String> {
        self.document.content_type().map(|s| s.to_string())
    }

    /// Get the document size in bytes.
    pub fn document_size(&self) -> usize {
        self.document.size()
    }

    /// Check if the document is empty.
    pub fn is_empty(&self) -> bool {
        self.document.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;
    use crate::types::Document;

    #[test]
    fn test_document_ocr_request_creation() {
        let document = Document::new(Bytes::from("test content")).with_content_type("image/png");

        let request = DocumentRequest::new(document.clone());

        assert_eq!(request.document.size(), 12);
        assert!(!request.has_region());
        assert_eq!(request.language, None);
        assert_eq!(request.content_type(), Some("image/png".to_string()));
    }

    #[test]
    fn test_document_ocr_request_with_region() {
        let document =
            Document::new(Bytes::from("test image data")).with_content_type("image/jpeg");

        let bbox = BoundingBox::new(10.0, 20.0, 100.0, 50.0);
        let request = DocumentRequest::new(document)
            .with_region(bbox)
            .with_language("en");

        assert!(request.has_region());
        assert_eq!(request.language, Some("en".to_string()));
        assert_eq!(request.region.unwrap().area(), 5000.0);
    }

    #[test]
    fn test_bounding_box_validation() {
        let valid_bbox = BoundingBox::new(0.0, 0.0, 100.0, 50.0);
        assert!(valid_bbox.is_valid());
        assert_eq!(valid_bbox.area(), 5000.0);

        let invalid_bbox = BoundingBox::new(0.0, 0.0, -10.0, 50.0);
        assert!(!invalid_bbox.is_valid());
    }

    #[test]
    fn test_request_is_empty() {
        let empty_document = Document::new(Bytes::new()).with_content_type("image/png");
        let request = DocumentRequest::new(empty_document);
        assert!(request.is_empty());

        let document =
            Document::new(Bytes::from(vec![0x89, 0x50, 0x4E, 0x47])).with_content_type("image/png");
        let request = DocumentRequest::new(document);
        assert!(!request.is_empty());
    }

    #[test]
    fn test_request_options() {
        let request = Request::new(())
            .with_confidence_threshold(0.8)
            .with_dpi(300)
            .with_layout_preservation(false);

        assert_eq!(request.options.confidence_threshold, Some(0.8));
        assert_eq!(request.options.dpi, Some(300));
        assert!(!request.options.preserve_layout);
    }
}
