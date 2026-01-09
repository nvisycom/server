//! OCR request types.

use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::response::OcrResponse;
use crate::types::Document;

/// Request for a single OCR operation.
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(
    name = "OcrRequestBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(private, name = "build_inner", error = "OcrRequestError")
)]
pub struct OcrRequest {
    /// Unique identifier for this request.
    #[builder(default = "Uuid::now_v7()")]
    pub request_id: Uuid,
    /// The document to process for text extraction.
    pub document: Document,
    /// Whether to preserve layout information in the output.
    #[builder(default = "true")]
    pub preserve_layout: bool,
}

/// Error type for OcrRequest builder.
pub type OcrRequestError = derive_builder::UninitializedFieldError;

impl OcrRequestBuilder {
    /// Build the request.
    pub fn build(self) -> Result<OcrRequest, OcrRequestError> {
        self.build_inner()
    }
}

impl OcrRequest {
    /// Create a new OCR request with the given document.
    pub fn new(document: Document) -> Self {
        Self {
            request_id: Uuid::now_v7(),
            document,
            preserve_layout: true,
        }
    }

    /// Create a new OCR request from a document (alias for `new`).
    pub fn from_document(document: Document) -> Self {
        Self::new(document)
    }

    /// Create a builder for this request.
    pub fn builder() -> OcrRequestBuilder {
        OcrRequestBuilder::default()
    }

    /// Get the document's content type.
    pub fn content_type(&self) -> Option<&str> {
        self.document.content_type()
    }

    /// Get the document size in bytes.
    pub fn document_size(&self) -> usize {
        self.document.size()
    }

    /// Check if the document is empty.
    pub fn is_empty(&self) -> bool {
        self.document.is_empty()
    }

    /// Get the document bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.document.as_bytes()
    }

    /// Create a response for this request with the given text.
    pub fn reply(&self, text: impl Into<String>) -> OcrResponse {
        OcrResponse::new(self.request_id, text)
    }
}

/// Batch request for multiple OCR operations.
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(
    name = "OcrBatchRequestBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(private, name = "build_inner", error = "OcrBatchRequestError")
)]
pub struct OcrBatchRequest {
    /// Unique identifier for this batch request.
    #[builder(default = "Uuid::now_v7()")]
    pub batch_id: Uuid,
    /// The documents to process.
    #[builder(default)]
    pub documents: Vec<Document>,
    /// Whether to preserve layout information.
    #[builder(default = "true")]
    pub preserve_layout: bool,
}

/// Error type for OcrBatchRequest builder.
pub type OcrBatchRequestError = derive_builder::UninitializedFieldError;

impl OcrBatchRequestBuilder {
    /// Build the request.
    pub fn build(self) -> Result<OcrBatchRequest, OcrBatchRequestError> {
        self.build_inner()
    }

    /// Add a document to the batch.
    pub fn add_document(mut self, document: Document) -> Self {
        self.documents.get_or_insert_with(Vec::new).push(document);
        self
    }
}

impl OcrBatchRequest {
    /// Create a new batch request.
    pub fn new() -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            documents: Vec::new(),
            preserve_layout: true,
        }
    }

    /// Create a new batch request from documents.
    pub fn from_documents(documents: Vec<Document>) -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            documents,
            preserve_layout: true,
        }
    }

    /// Create a builder for this request.
    pub fn builder() -> OcrBatchRequestBuilder {
        OcrBatchRequestBuilder::default()
    }

    /// Returns the number of documents in this batch.
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    /// Returns true if this batch has no documents.
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Convert this batch request into individual requests.
    pub fn into_requests(self) -> Vec<OcrRequest> {
        self.documents
            .into_iter()
            .map(|document| OcrRequest {
                request_id: Uuid::now_v7(),
                document,
                preserve_layout: self.preserve_layout,
            })
            .collect()
    }

    /// Create individual requests from this batch.
    pub fn iter_requests(&self) -> Vec<OcrRequest> {
        self.documents
            .iter()
            .cloned()
            .map(|document| OcrRequest {
                request_id: Uuid::now_v7(),
                document,
                preserve_layout: self.preserve_layout,
            })
            .collect()
    }

    /// Estimates the total size of all documents.
    pub fn estimated_total_size(&self) -> usize {
        self.documents.iter().map(|doc| doc.size()).sum()
    }
}

impl Default for OcrBatchRequest {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;

    #[test]
    fn test_ocr_request_creation() {
        let document = Document::new(Bytes::from("test image data")).with_content_type("image/png");
        let request = OcrRequest::from_document(document);
        assert!(!request.request_id.is_nil());
        assert!(request.preserve_layout);
        assert_eq!(request.content_type(), Some("image/png"));
    }

    #[test]
    fn test_ocr_request_builder() {
        let document = Document::new(Bytes::from("test")).with_content_type("image/png");
        let request = OcrRequest::builder()
            .with_document(document)
            .with_preserve_layout(false)
            .build()
            .unwrap();
        assert!(!request.preserve_layout);
    }

    #[test]
    fn test_ocr_batch_request() {
        let doc1 = Document::new(Bytes::from("doc1")).with_content_type("image/png");
        let doc2 = Document::new(Bytes::from("doc2")).with_content_type("image/jpeg");
        let batch = OcrBatchRequest::builder()
            .add_document(doc1)
            .add_document(doc2)
            .build()
            .unwrap();
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }
}
