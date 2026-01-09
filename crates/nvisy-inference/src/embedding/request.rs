//! Embedding request types.

use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::response::EmbeddingResponse;
use crate::types::{Chat, Content, Document};

/// Request for a single embedding operation.
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(
    name = "EmbeddingRequestBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(private, name = "build_inner", error = "EmbeddingRequestError")
)]
pub struct EmbeddingRequest {
    /// Unique identifier for this request.
    #[builder(default = "Uuid::now_v7()")]
    pub request_id: Uuid,
    /// The content to generate an embedding for.
    pub content: Content,
    /// Whether to normalize the resulting embedding to unit length.
    #[builder(default)]
    pub normalize: bool,
}

/// Error type for EmbeddingRequest builder.
pub type EmbeddingRequestError = derive_builder::UninitializedFieldError;

impl EmbeddingRequestBuilder {
    /// Build the request.
    pub fn build(self) -> Result<EmbeddingRequest, EmbeddingRequestError> {
        self.build_inner()
    }
}

impl EmbeddingRequest {
    /// Create a new embedding request with the given content.
    pub fn new(content: Content) -> Self {
        Self {
            request_id: Uuid::now_v7(),
            content,
            normalize: false,
        }
    }

    /// Create a new embedding request from text.
    pub fn from_text(text: impl Into<String>) -> Self {
        Self::new(Content::text(text))
    }

    /// Create a new embedding request from a document.
    pub fn from_document(document: Document) -> Self {
        Self::new(Content::document(document))
    }

    /// Create a new embedding request from a chat.
    pub fn from_chat(chat: Chat) -> Self {
        Self::new(Content::chat(chat))
    }

    /// Create a builder for this request.
    pub fn builder() -> EmbeddingRequestBuilder {
        EmbeddingRequestBuilder::default()
    }

    /// Get the text content if this is a text request.
    pub fn as_text(&self) -> Option<&str> {
        self.content.as_text()
    }

    /// Create a response for this request with the given embedding.
    pub fn reply(&self, embedding: Vec<f32>) -> EmbeddingResponse {
        EmbeddingResponse::new(self.request_id, embedding)
    }
}

/// Batch request for multiple embedding operations.
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(
    name = "EmbeddingBatchRequestBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(private, name = "build_inner", error = "EmbeddingBatchRequestError")
)]
pub struct EmbeddingBatchRequest {
    /// Unique identifier for this batch request.
    #[builder(default = "Uuid::now_v7()")]
    pub batch_id: Uuid,
    /// The contents to generate embeddings for.
    #[builder(default)]
    pub contents: Vec<Content>,
    /// Whether to normalize the resulting embeddings to unit length.
    #[builder(default)]
    pub normalize: bool,
}

/// Error type for EmbeddingBatchRequest builder.
pub type EmbeddingBatchRequestError = derive_builder::UninitializedFieldError;

impl EmbeddingBatchRequestBuilder {
    /// Build the request.
    pub fn build(self) -> Result<EmbeddingBatchRequest, EmbeddingBatchRequestError> {
        self.build_inner()
    }

    /// Add a content item to the batch.
    pub fn add_content(mut self, content: Content) -> Self {
        self.contents.get_or_insert_with(Vec::new).push(content);
        self
    }

    /// Add a text input to the batch.
    pub fn add_text(self, text: impl Into<String>) -> Self {
        self.add_content(Content::text(text))
    }

    /// Add a document input to the batch.
    pub fn add_document(self, document: Document) -> Self {
        self.add_content(Content::document(document))
    }

    /// Add a chat input to the batch.
    pub fn add_chat(self, chat: Chat) -> Self {
        self.add_content(Content::chat(chat))
    }
}

impl EmbeddingBatchRequest {
    /// Create a new batch request.
    pub fn new() -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            contents: Vec::new(),
            normalize: false,
        }
    }

    /// Create a new batch request from contents.
    pub fn from_contents(contents: Vec<Content>) -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            contents,
            normalize: false,
        }
    }

    /// Create a builder for this request.
    pub fn builder() -> EmbeddingBatchRequestBuilder {
        EmbeddingBatchRequestBuilder::default()
    }

    /// Returns the number of contents in this batch.
    pub fn len(&self) -> usize {
        self.contents.len()
    }

    /// Returns true if this batch has no contents.
    pub fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }

    /// Convert this batch request into individual requests.
    pub fn into_requests(self) -> Vec<EmbeddingRequest> {
        self.contents
            .into_iter()
            .map(|content| EmbeddingRequest {
                request_id: Uuid::now_v7(),
                content,
                normalize: self.normalize,
            })
            .collect()
    }

    /// Create individual requests from this batch.
    pub fn iter_requests(&self) -> Vec<EmbeddingRequest> {
        self.contents
            .iter()
            .cloned()
            .map(|content| EmbeddingRequest {
                request_id: Uuid::now_v7(),
                content,
                normalize: self.normalize,
            })
            .collect()
    }

    /// Estimates the total size of all contents for rate limiting.
    pub fn estimated_total_size(&self) -> usize {
        self.contents
            .iter()
            .map(|content| content.estimated_size())
            .sum()
    }
}

impl Default for EmbeddingBatchRequest {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;

    #[test]
    fn test_embedding_request_creation() {
        let request = EmbeddingRequest::from_text("Hello, world!");
        assert!(!request.request_id.is_nil());
        assert!(!request.normalize);
        assert_eq!(request.as_text(), Some("Hello, world!"));
    }

    #[test]
    fn test_embedding_request_builder() {
        let request = EmbeddingRequest::builder()
            .with_content(Content::text("Hello"))
            .with_normalize(true)
            .build()
            .unwrap();
        assert!(request.normalize);
        assert_eq!(request.as_text(), Some("Hello"));
    }

    #[test]
    fn test_embedding_batch_request() {
        let document = Document::new(Bytes::from("Hello, world!")).with_content_type("text/plain");
        let batch = EmbeddingBatchRequest::builder()
            .add_text("Test text")
            .add_document(document)
            .build()
            .unwrap();
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }
}
