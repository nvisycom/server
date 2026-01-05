//! Embedding request types.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::response::EmbeddingResponse;
use crate::{Chat, Content, Document};

/// Request for a single embedding operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    /// Unique identifier for this request.
    pub request_id: Uuid,
    /// Account identifier associated with this request.
    pub account_id: Option<Uuid>,
    /// The content to generate an embedding for.
    pub content: Content,
    /// Custom tags for categorization and filtering.
    pub tags: HashSet<String>,
    /// Whether to normalize the resulting embedding to unit length.
    pub normalize: bool,
}

impl EmbeddingRequest {
    /// Create a new embedding request with the given content.
    pub fn new(content: Content) -> Self {
        Self {
            request_id: Uuid::now_v7(),
            account_id: None,
            content,
            tags: HashSet::new(),
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

    /// Create a new embedding request with a specific request ID.
    pub fn with_request_id(mut self, request_id: Uuid) -> Self {
        self.request_id = request_id;
        self
    }

    /// Set the account ID for this request.
    pub fn with_account_id(mut self, account_id: Uuid) -> Self {
        self.account_id = Some(account_id);
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

    /// Enable normalization of the embedding to unit length.
    pub fn with_normalize(mut self, normalize: bool) -> Self {
        self.normalize = normalize;
        self
    }

    /// Check if the request has a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingBatchRequest {
    /// Unique identifier for this batch request.
    pub batch_id: Uuid,
    /// Account identifier associated with this batch.
    pub account_id: Option<Uuid>,
    /// The contents to generate embeddings for.
    pub contents: Vec<Content>,
    /// Custom tags for categorization and filtering.
    pub tags: HashSet<String>,
    /// Whether to normalize the resulting embeddings to unit length.
    pub normalize: bool,
}

impl EmbeddingBatchRequest {
    /// Create a new batch request.
    pub fn new() -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            account_id: None,
            contents: Vec::new(),
            tags: HashSet::new(),
            normalize: false,
        }
    }

    /// Create a new batch request from contents.
    pub fn from_contents(contents: Vec<Content>) -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            account_id: None,
            contents,
            tags: HashSet::new(),
            normalize: false,
        }
    }

    /// Set the account ID for this batch.
    pub fn with_account_id(mut self, account_id: Uuid) -> Self {
        self.account_id = Some(account_id);
        self
    }

    /// Add a content item to the batch.
    pub fn with_content(mut self, content: Content) -> Self {
        self.contents.push(content);
        self
    }

    /// Add a text input to the batch.
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.contents.push(Content::text(text));
        self
    }

    /// Add a document input to the batch.
    pub fn with_document(mut self, document: Document) -> Self {
        self.contents.push(Content::document(document));
        self
    }

    /// Add a chat input to the batch.
    pub fn with_chat(mut self, chat: Chat) -> Self {
        self.contents.push(Content::chat(chat));
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

    /// Enable normalization of embeddings to unit length.
    pub fn with_normalize(mut self, normalize: bool) -> Self {
        self.normalize = normalize;
        self
    }

    /// Check if the batch request has a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
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
                account_id: self.account_id,
                content,
                tags: self.tags.clone(),
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
                account_id: self.account_id,
                content,
                tags: self.tags.clone(),
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
        assert!(request.account_id.is_none());
        assert!(request.tags.is_empty());
        assert!(!request.normalize);
        assert_eq!(request.as_text(), Some("Hello, world!"));
    }

    #[test]
    fn test_embedding_batch_request() {
        let document = Document::new(Bytes::from("Hello, world!")).with_content_type("text/plain");
        let batch = EmbeddingBatchRequest::new()
            .with_text("Test text")
            .with_document(document);
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }
}
