//! VLM request types.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::response::VlmResponse;
use crate::{Document, Message};

/// Request for a single VLM operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlmRequest {
    /// Unique identifier for this request.
    pub request_id: Uuid,
    /// Account identifier associated with this request.
    pub account_id: Option<Uuid>,
    /// Text prompt for the VLM.
    pub prompt: String,
    /// Documents to analyze (images, PDFs, etc.).
    pub documents: Vec<Document>,
    /// Optional conversation history.
    pub messages: Vec<Message>,
    /// Custom tags for categorization and filtering.
    pub tags: HashSet<String>,
    /// Maximum number of tokens to generate.
    pub max_tokens: Option<u32>,
    /// Temperature for response generation (0.0 to 1.0).
    pub temperature: Option<f32>,
    /// Custom parameters for specific VLM engines.
    pub custom_parameters: HashMap<String, serde_json::Value>,
}

impl VlmRequest {
    /// Create a new VLM request with the given prompt.
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            request_id: Uuid::now_v7(),
            account_id: None,
            prompt: prompt.into(),
            documents: Vec::new(),
            messages: Vec::new(),
            tags: HashSet::new(),
            max_tokens: Some(1024),
            temperature: Some(0.7),
            custom_parameters: HashMap::new(),
        }
    }

    /// Create a new VLM request with prompt and document.
    pub fn with_document(prompt: impl Into<String>, document: Document) -> Self {
        Self {
            request_id: Uuid::now_v7(),
            account_id: None,
            prompt: prompt.into(),
            documents: vec![document],
            messages: Vec::new(),
            tags: HashSet::new(),
            max_tokens: Some(1024),
            temperature: Some(0.7),
            custom_parameters: HashMap::new(),
        }
    }

    /// Create a new VLM request with a specific request ID.
    pub fn with_request_id(mut self, request_id: Uuid) -> Self {
        self.request_id = request_id;
        self
    }

    /// Set the account ID for this request.
    pub fn with_account_id(mut self, account_id: Uuid) -> Self {
        self.account_id = Some(account_id);
        self
    }

    /// Add a document to this request.
    pub fn add_document(mut self, document: Document) -> Self {
        self.documents.push(document);
        self
    }

    /// Add multiple documents to this request.
    pub fn with_documents(mut self, documents: Vec<Document>) -> Self {
        self.documents = documents;
        self
    }

    /// Add a message to the conversation history.
    pub fn add_message(mut self, message: Message) -> Self {
        self.messages.push(message);
        self
    }

    /// Set the conversation history.
    pub fn with_messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
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

    /// Set maximum tokens to generate.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set temperature for response generation.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature.clamp(0.0, 1.0));
        self
    }

    /// Add a custom parameter.
    pub fn with_custom_parameter(
        mut self,
        key: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        self.custom_parameters.insert(key.into(), value);
        self
    }

    /// Check if the request has a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlmBatchRequest {
    /// Unique identifier for this batch request.
    pub batch_id: Uuid,
    /// Account identifier associated with this batch.
    pub account_id: Option<Uuid>,
    /// Individual requests in the batch.
    requests: Vec<VlmRequest>,
    /// Custom tags for the entire batch.
    pub tags: HashSet<String>,
}

impl VlmBatchRequest {
    /// Create a new batch request.
    pub fn new() -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            account_id: None,
            requests: Vec::new(),
            tags: HashSet::new(),
        }
    }

    /// Create a new batch request from requests.
    pub fn from_requests(requests: Vec<VlmRequest>) -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            account_id: None,
            requests,
            tags: HashSet::new(),
        }
    }

    /// Set the account ID for this batch.
    pub fn with_account_id(mut self, account_id: Uuid) -> Self {
        self.account_id = Some(account_id);
        self
    }

    /// Add a request to the batch.
    pub fn with_request(mut self, request: VlmRequest) -> Self {
        self.requests.push(request);
        self
    }

    /// Add a simple prompt request to the batch.
    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.requests.push(VlmRequest::new(prompt));
        self
    }

    /// Add a request with prompt and document to the batch.
    pub fn with_prompt_and_document(
        mut self,
        prompt: impl Into<String>,
        document: Document,
    ) -> Self {
        self.requests
            .push(VlmRequest::with_document(prompt, document));
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

    /// Check if the batch request has a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
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
    use crate::MessageRole;

    #[test]
    fn test_vlm_request_creation() {
        let request = VlmRequest::new("Describe this image");
        assert!(!request.request_id.is_nil());
        assert!(request.account_id.is_none());
        assert!(request.tags.is_empty());
        assert_eq!(request.prompt, "Describe this image");
        assert!(request.is_text_only());
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
        let request = VlmRequest::new("Continue")
            .add_message(message1)
            .add_message(message2);
        assert!(request.has_messages());
        assert_eq!(request.message_count(), 2);
    }

    #[test]
    fn test_vlm_batch_request() {
        let batch = VlmBatchRequest::new()
            .with_prompt("First prompt")
            .with_prompt("Second prompt");
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }
}
