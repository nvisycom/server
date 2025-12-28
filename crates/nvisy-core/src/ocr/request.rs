//! Request types for OCR operations.
//!
//! This module provides `Request` for single-document OCR operations
//! and `BatchRequest` for processing multiple documents in one call.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Response;
use crate::types::Document;

/// Request for a single OCR operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Unique identifier for this request.
    pub request_id: Uuid,
    /// Account identifier associated with this request.
    pub account_id: Option<Uuid>,
    /// The document to process for text extraction.
    pub document: Document,
    /// Optional custom prompt for OCR processing.
    pub prompt: Option<String>,
    /// Language hint for OCR processing (ISO 639-1 code).
    pub language: Option<String>,
    /// Custom tags for categorization and filtering.
    pub tags: HashSet<String>,
    /// Whether to preserve layout information in the output.
    pub preserve_layout: bool,
    /// Minimum confidence threshold for text extraction.
    pub confidence_threshold: Option<f32>,
}

impl Request {
    /// Create a new OCR request with the given document.
    pub fn new(document: Document) -> Self {
        Self {
            request_id: Uuid::now_v7(),
            account_id: None,
            document,
            prompt: None,
            language: None,
            tags: HashSet::new(),
            preserve_layout: true,
            confidence_threshold: None,
        }
    }

    /// Create a new OCR request from a document (alias for `new`).
    pub fn from_document(document: Document) -> Self {
        Self::new(document)
    }

    /// Create a new OCR request with a specific request ID.
    pub fn with_request_id(mut self, request_id: Uuid) -> Self {
        self.request_id = request_id;
        self
    }

    /// Set the account ID for this request.
    pub fn with_account_id(mut self, account_id: Uuid) -> Self {
        self.account_id = Some(account_id);
        self
    }

    /// Set a custom prompt for OCR processing.
    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    /// Set the language hint for OCR processing.
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Add a tag to this request.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.insert(tag.into());
        self
    }

    /// Set tags for this request.
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(|t| t.into()).collect();
        self
    }

    /// Set whether to preserve layout information.
    pub fn with_preserve_layout(mut self, preserve: bool) -> Self {
        self.preserve_layout = preserve;
        self
    }

    /// Set the confidence threshold for text extraction.
    pub fn with_confidence_threshold(mut self, threshold: f32) -> Self {
        self.confidence_threshold = Some(threshold);
        self
    }

    /// Check if the request has a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
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
    pub fn reply(&self, text: impl Into<String>) -> Response {
        Response::new(self.request_id, text)
    }
}

/// Batch request for multiple OCR operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRequest {
    /// Unique identifier for this batch request.
    pub batch_id: Uuid,
    /// Account identifier associated with this batch.
    pub account_id: Option<Uuid>,
    /// The documents to process.
    pub documents: Vec<Document>,
    /// Optional custom prompt for OCR processing.
    pub prompt: Option<String>,
    /// Language hint for OCR processing (ISO 639-1 code).
    pub language: Option<String>,
    /// Custom tags for categorization and filtering.
    pub tags: HashSet<String>,
    /// Whether to preserve layout information.
    pub preserve_layout: bool,
    /// Minimum confidence threshold for text extraction.
    pub confidence_threshold: Option<f32>,
}

impl BatchRequest {
    /// Create a new batch request.
    pub fn new() -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            account_id: None,
            documents: Vec::new(),
            prompt: None,
            language: None,
            tags: HashSet::new(),
            preserve_layout: true,
            confidence_threshold: None,
        }
    }

    /// Create a new batch request from documents.
    pub fn from_documents(documents: Vec<Document>) -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            account_id: None,
            documents,
            prompt: None,
            language: None,
            tags: HashSet::new(),
            preserve_layout: true,
            confidence_threshold: None,
        }
    }

    /// Set the account ID for this batch.
    pub fn with_account_id(mut self, account_id: Uuid) -> Self {
        self.account_id = Some(account_id);
        self
    }

    /// Add a document to the batch.
    pub fn with_document(mut self, document: Document) -> Self {
        self.documents.push(document);
        self
    }

    /// Set a custom prompt for OCR processing.
    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    /// Set the language hint for OCR processing.
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Add a tag to this batch request.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.insert(tag.into());
        self
    }

    /// Set tags for this batch request.
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(|t| t.into()).collect();
        self
    }

    /// Set whether to preserve layout information.
    pub fn with_preserve_layout(mut self, preserve: bool) -> Self {
        self.preserve_layout = preserve;
        self
    }

    /// Set the confidence threshold for text extraction.
    pub fn with_confidence_threshold(mut self, threshold: f32) -> Self {
        self.confidence_threshold = Some(threshold);
        self
    }

    /// Check if the batch request has a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
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
    pub fn into_requests(self) -> Vec<Request> {
        self.documents
            .into_iter()
            .map(|document| Request {
                request_id: Uuid::now_v7(),
                account_id: self.account_id,
                document,
                prompt: self.prompt.clone(),
                language: self.language.clone(),
                tags: self.tags.clone(),
                preserve_layout: self.preserve_layout,
                confidence_threshold: self.confidence_threshold,
            })
            .collect()
    }

    /// Create individual requests from this batch.
    pub fn iter_requests(&self) -> Vec<Request> {
        self.documents
            .iter()
            .cloned()
            .map(|document| Request {
                request_id: Uuid::now_v7(),
                account_id: self.account_id,
                document,
                prompt: self.prompt.clone(),
                language: self.language.clone(),
                tags: self.tags.clone(),
                preserve_layout: self.preserve_layout,
                confidence_threshold: self.confidence_threshold,
            })
            .collect()
    }

    /// Estimates the total size of all documents.
    pub fn estimated_total_size(&self) -> usize {
        self.documents.iter().map(|doc| doc.size()).sum()
    }
}

impl Default for BatchRequest {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;

    #[test]
    fn test_request_creation() {
        let document = Document::new(Bytes::from("test image data")).with_content_type("image/png");
        let request = Request::from_document(document);

        assert!(!request.request_id.is_nil());
        assert!(request.account_id.is_none());
        assert!(request.tags.is_empty());
        assert!(request.preserve_layout);
        assert_eq!(request.content_type(), Some("image/png"));
    }

    #[test]
    fn test_request_with_account_id() {
        let document = Document::new(Bytes::from("test")).with_content_type("image/png");
        let account_id = Uuid::new_v4();
        let request = Request::from_document(document).with_account_id(account_id);

        assert_eq!(request.account_id, Some(account_id));
    }

    #[test]
    fn test_request_with_tags() {
        let document = Document::new(Bytes::from("test")).with_content_type("image/png");
        let request = Request::from_document(document)
            .with_tag("category:test")
            .with_tag("priority:high");

        assert_eq!(request.tags.len(), 2);
        assert!(request.has_tag("category:test"));
        assert!(request.has_tag("priority:high"));
        assert!(!request.has_tag("unknown"));
    }

    #[test]
    fn test_request_with_options() {
        let document = Document::new(Bytes::from("test")).with_content_type("image/png");
        let request = Request::from_document(document)
            .with_prompt("Extract all text")
            .with_language("en")
            .with_preserve_layout(false)
            .with_confidence_threshold(0.8);

        assert_eq!(request.prompt, Some("Extract all text".to_string()));
        assert_eq!(request.language, Some("en".to_string()));
        assert!(!request.preserve_layout);
        assert_eq!(request.confidence_threshold, Some(0.8));
    }

    #[test]
    fn test_request_reply() {
        let document = Document::new(Bytes::from("test")).with_content_type("image/png");
        let request = Request::from_document(document);
        let text = "Extracted text";

        let response = request.reply(text);

        assert_eq!(response.request_id, request.request_id);
        assert_eq!(response.text(), text);
    }

    #[test]
    fn test_batch_request() {
        let doc1 = Document::new(Bytes::from("doc1")).with_content_type("image/png");
        let doc2 = Document::new(Bytes::from("doc2")).with_content_type("image/jpeg");

        let batch = BatchRequest::new().with_document(doc1).with_document(doc2);

        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_batch_request_with_account_id() {
        let doc1 = Document::new(Bytes::from("doc1")).with_content_type("image/png");
        let doc2 = Document::new(Bytes::from("doc2")).with_content_type("image/jpeg");
        let account_id = Uuid::new_v4();

        let batch = BatchRequest::new()
            .with_account_id(account_id)
            .with_document(doc1)
            .with_document(doc2);

        let requests = batch.into_requests();

        assert_eq!(requests[0].account_id, Some(account_id));
        assert_eq!(requests[1].account_id, Some(account_id));
    }

    #[test]
    fn test_batch_request_into_requests() {
        let doc1 = Document::new(Bytes::from("doc1")).with_content_type("image/png");
        let doc2 = Document::new(Bytes::from("doc2")).with_content_type("image/jpeg");

        let batch = BatchRequest::new()
            .with_document(doc1)
            .with_document(doc2)
            .with_tag("batch")
            .with_language("en");

        let requests = batch.into_requests();

        assert_eq!(requests.len(), 2);
        assert!(requests[0].has_tag("batch"));
        assert_eq!(requests[0].language, Some("en".to_string()));
        assert!(requests[1].has_tag("batch"));
        assert_eq!(requests[1].language, Some("en".to_string()));
    }
}
