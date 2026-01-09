//! VLM request types.

use std::collections::HashMap;

use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::response::VlmResponse;
use crate::types::{Document, Message};

/// Request for a single VLM operation.
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(
    name = "VlmRequestBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(private, name = "build_inner", error = "VlmRequestError")
)]
pub struct VlmRequest {
    /// Unique identifier for this request.
    #[builder(default = "Uuid::now_v7()")]
    pub request_id: Uuid,
    /// Text prompt for the VLM.
    pub prompt: String,
    /// Documents to analyze (images, PDFs, etc.).
    #[builder(default)]
    pub documents: Vec<Document>,
    /// Optional conversation history.
    #[builder(default)]
    pub messages: Vec<Message>,
    /// Maximum number of tokens to generate.
    #[builder(default = "Some(1024)")]
    pub max_tokens: Option<u32>,
    /// Temperature for response generation (0.0 to 1.0).
    #[builder(default = "Some(0.7)")]
    pub temperature: Option<f32>,
    /// Custom parameters for specific VLM engines.
    #[builder(default)]
    pub custom_parameters: HashMap<String, serde_json::Value>,
}

/// Error type for VlmRequest builder.
pub type VlmRequestError = derive_builder::UninitializedFieldError;

impl VlmRequestBuilder {
    /// Build the request.
    pub fn build(self) -> Result<VlmRequest, VlmRequestError> {
        self.build_inner()
    }

    /// Add a document to analyze.
    pub fn add_document(mut self, document: Document) -> Self {
        self.documents.get_or_insert_with(Vec::new).push(document);
        self
    }

    /// Add a message to the conversation history.
    pub fn add_message(mut self, message: Message) -> Self {
        self.messages.get_or_insert_with(Vec::new).push(message);
        self
    }

    /// Add a custom parameter.
    pub fn add_custom_parameter(
        mut self,
        key: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        self.custom_parameters
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value);
        self
    }
}

impl VlmRequest {
    /// Create a new VLM request with the given prompt.
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            request_id: Uuid::now_v7(),
            prompt: prompt.into(),
            documents: Vec::new(),
            messages: Vec::new(),
            max_tokens: Some(1024),
            temperature: Some(0.7),
            custom_parameters: HashMap::new(),
        }
    }

    /// Create a new VLM request with prompt and document.
    pub fn with_document(prompt: impl Into<String>, document: Document) -> Self {
        Self {
            request_id: Uuid::now_v7(),
            prompt: prompt.into(),
            documents: vec![document],
            messages: Vec::new(),
            max_tokens: Some(1024),
            temperature: Some(0.7),
            custom_parameters: HashMap::new(),
        }
    }

    /// Create a builder for this request.
    pub fn builder() -> VlmRequestBuilder {
        VlmRequestBuilder::default()
    }

    /// Check if this request has documents.
    pub fn has_documents(&self) -> bool {
        !self.documents.is_empty()
    }

    /// Get the number of documents.
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Check if this request has messages.
    pub fn has_messages(&self) -> bool {
        !self.messages.is_empty()
    }

    /// Get the number of messages.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Check if this is a text-only request.
    pub fn is_text_only(&self) -> bool {
        self.documents.is_empty()
    }

    /// Get the total size of all documents.
    pub fn total_document_size(&self) -> usize {
        self.documents.iter().map(|doc| doc.size()).sum()
    }

    /// Get the prompt length in characters.
    pub fn prompt_length(&self) -> usize {
        self.prompt.chars().count()
    }

    /// Create a response for this request with the given content.
    pub fn reply(&self, content: impl Into<String>) -> VlmResponse {
        VlmResponse::new(self.request_id, content)
    }
}

/// Batch request for multiple VLM operations.
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(
    name = "VlmBatchRequestBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(private, name = "build_inner", error = "VlmBatchRequestError")
)]
pub struct VlmBatchRequest {
    /// Unique identifier for this batch request.
    #[builder(default = "Uuid::now_v7()")]
    pub batch_id: Uuid,
    /// Individual requests in the batch.
    #[builder(default)]
    requests: Vec<VlmRequest>,
}

/// Error type for VlmBatchRequest builder.
pub type VlmBatchRequestError = derive_builder::UninitializedFieldError;

impl VlmBatchRequestBuilder {
    /// Build the request.
    pub fn build(self) -> Result<VlmBatchRequest, VlmBatchRequestError> {
        self.build_inner()
    }

    /// Add a request to the batch.
    pub fn add_request(mut self, request: VlmRequest) -> Self {
        self.requests.get_or_insert_with(Vec::new).push(request);
        self
    }

    /// Add a simple prompt request to the batch.
    pub fn add_prompt(self, prompt: impl Into<String>) -> Self {
        self.add_request(VlmRequest::new(prompt))
    }

    /// Add a request with prompt and document to the batch.
    pub fn add_prompt_and_document(self, prompt: impl Into<String>, document: Document) -> Self {
        self.add_request(VlmRequest::with_document(prompt, document))
    }
}

impl VlmBatchRequest {
    /// Create a new batch request.
    pub fn new() -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            requests: Vec::new(),
        }
    }

    /// Create a new batch request from requests.
    pub fn from_requests(requests: Vec<VlmRequest>) -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            requests,
        }
    }

    /// Create a builder for this request.
    pub fn builder() -> VlmBatchRequestBuilder {
        VlmBatchRequestBuilder::default()
    }

    /// Returns the number of requests in this batch.
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    /// Returns true if this batch has no requests.
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    /// Convert this batch request into individual requests.
    pub fn into_requests(self) -> Vec<VlmRequest> {
        self.requests
    }

    /// Get a reference to the requests.
    pub fn iter_requests(&self) -> &[VlmRequest] {
        &self.requests
    }

    /// Get the total number of documents across all requests.
    pub fn total_documents(&self) -> usize {
        self.requests.iter().map(|r| r.document_count()).sum()
    }

    /// Get the total size of all documents across all requests.
    pub fn total_document_size(&self) -> usize {
        self.requests.iter().map(|r| r.total_document_size()).sum()
    }
}

impl Default for VlmBatchRequest {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;
    use crate::types::MessageRole;

    #[test]
    fn test_vlm_request_creation() {
        let request = VlmRequest::new("Describe this image");
        assert!(!request.request_id.is_nil());
        assert_eq!(request.prompt, "Describe this image");
        assert!(request.is_text_only());
    }

    #[test]
    fn test_vlm_request_builder() {
        let document = Document::new(Bytes::from("image data")).with_content_type("image/png");
        let request = VlmRequest::builder()
            .with_prompt("Describe this")
            .add_document(document)
            .with_max_tokens(2048u32)
            .build()
            .unwrap();
        assert!(request.has_documents());
        assert_eq!(request.max_tokens, Some(2048));
    }

    #[test]
    fn test_vlm_request_with_document() {
        let document = Document::new(Bytes::from("image data")).with_content_type("image/png");
        let request = VlmRequest::with_document("Describe this", document);
        assert!(request.has_documents());
        assert_eq!(request.document_count(), 1);
        assert!(!request.is_text_only());
    }

    #[test]
    fn test_vlm_request_with_messages() {
        let message1 = Message::new(MessageRole::User, "Previous question");
        let message2 = Message::new(MessageRole::Assistant, "Previous response");
        let request = VlmRequest::builder()
            .with_prompt("Continue")
            .add_message(message1)
            .add_message(message2)
            .build()
            .unwrap();
        assert!(request.has_messages());
        assert_eq!(request.message_count(), 2);
    }

    #[test]
    fn test_vlm_batch_request() {
        let batch = VlmBatchRequest::builder()
            .add_prompt("First prompt")
            .add_prompt("Second prompt")
            .build()
            .unwrap();
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }
}
