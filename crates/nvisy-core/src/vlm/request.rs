//! Request types for VLM operations.
//!
//! This module provides `Request` for single VLM operations
//! and `BatchRequest` for processing multiple requests in one call.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Response;
use crate::types::{Document, Message};

/// Request for a single VLM operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
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

impl Request {
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
    pub fn reply(&self, content: impl Into<String>) -> Response {
        Response::new(self.request_id, content)
    }
}

/// Batch request for multiple VLM operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRequest {
    /// Unique identifier for this batch request.
    pub batch_id: Uuid,
    /// Account identifier associated with this batch.
    pub account_id: Option<Uuid>,
    /// Individual requests in the batch.
    requests: Vec<Request>,
    /// Custom tags for the entire batch.
    pub tags: HashSet<String>,
}

impl BatchRequest {
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
    pub fn from_requests(requests: Vec<Request>) -> Self {
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
    pub fn with_request(mut self, request: Request) -> Self {
        self.requests.push(request);
        self
    }

    /// Add a simple prompt request to the batch.
    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.requests.push(Request::new(prompt));
        self
    }

    /// Add a request with prompt and document to the batch.
    pub fn with_prompt_and_document(
        mut self,
        prompt: impl Into<String>,
        document: Document,
    ) -> Self {
        self.requests.push(Request::with_document(prompt, document));
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
    pub fn into_requests(self) -> Vec<Request> {
        self.requests
    }

    /// Get a reference to the requests.
    pub fn iter_requests(&self) -> &[Request] {
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

impl Default for BatchRequest {
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
    fn test_request_creation() {
        let request = Request::new("Describe this image");

        assert!(!request.request_id.is_nil());
        assert!(request.account_id.is_none());
        assert!(request.tags.is_empty());
        assert_eq!(request.prompt, "Describe this image");
        assert!(request.is_text_only());
    }

    #[test]
    fn test_request_with_document() {
        let document = Document::new(Bytes::from("image data")).with_content_type("image/png");
        let request = Request::with_document("Describe this", document);

        assert!(request.has_documents());
        assert_eq!(request.document_count(), 1);
        assert!(!request.is_text_only());
    }

    #[test]
    fn test_request_with_account_id() {
        let account_id = Uuid::new_v4();
        let request = Request::new("test").with_account_id(account_id);

        assert_eq!(request.account_id, Some(account_id));
    }

    #[test]
    fn test_request_with_tags() {
        let request = Request::new("test")
            .with_tag("category:test")
            .with_tag("priority:high");

        assert_eq!(request.tags.len(), 2);
        assert!(request.has_tag("category:test"));
        assert!(request.has_tag("priority:high"));
        assert!(!request.has_tag("unknown"));
    }

    #[test]
    fn test_request_with_options() {
        let request = Request::new("test")
            .with_max_tokens(500)
            .with_temperature(0.5);

        assert_eq!(request.max_tokens, Some(500));
        assert_eq!(request.temperature, Some(0.5));
    }

    #[test]
    fn test_request_with_messages() {
        let message1 = Message::new(MessageRole::User, "Previous question");
        let message2 = Message::new(MessageRole::Assistant, "Previous response");

        let request = Request::new("Continue")
            .add_message(message1)
            .add_message(message2);

        assert!(request.has_messages());
        assert_eq!(request.message_count(), 2);
    }

    #[test]
    fn test_request_reply() {
        let request = Request::new("test");
        let content = "This is the response";

        let response = request.reply(content);

        assert_eq!(response.request_id, request.request_id);
        assert_eq!(response.content(), content);
    }

    #[test]
    fn test_batch_request() {
        let batch = BatchRequest::new()
            .with_prompt("First prompt")
            .with_prompt("Second prompt");

        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_batch_request_with_documents() {
        let doc1 = Document::new(Bytes::from("doc1")).with_content_type("image/png");
        let doc2 = Document::new(Bytes::from("doc2")).with_content_type("image/jpeg");

        let batch = BatchRequest::new()
            .with_prompt_and_document("Describe", doc1)
            .with_prompt_and_document("Analyze", doc2);

        assert_eq!(batch.len(), 2);
        assert_eq!(batch.total_documents(), 2);
    }

    #[test]
    fn test_batch_request_with_account_id() {
        let account_id = Uuid::new_v4();

        let batch = BatchRequest::new()
            .with_account_id(account_id)
            .with_prompt("First")
            .with_prompt("Second");

        assert_eq!(batch.account_id, Some(account_id));
    }

    #[test]
    fn test_batch_request_into_requests() {
        let batch = BatchRequest::new()
            .with_prompt("First")
            .with_prompt("Second")
            .with_tag("batch");

        let requests = batch.into_requests();

        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].prompt, "First");
        assert_eq!(requests[1].prompt, "Second");
    }
}
