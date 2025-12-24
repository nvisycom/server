//! Request types for embedding operations.
//!
//! The `Request<Req>` type is a generic wrapper that allows embedding implementations
//! to define their own request payload types while maintaining a consistent
//! interface for common metadata like request IDs and options.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{Chat, Content, Document};

/// Generic request for embedding operations.
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
/// struct MyEmbeddingPayload {
///     custom_field: String,
/// }
///
/// let request = Request::new(MyEmbeddingPayload {
///     custom_field: "value".to_string(),
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

/// Processing options for embedding requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestOptions {
    /// The number of dimensions the resulting output embeddings should have.
    pub dimensions: Option<u32>,
    /// The format to return embeddings in.
    pub encoding_format: EncodingFormat,
    /// A unique identifier representing your end-user.
    pub user: Option<String>,
}

impl Default for RequestOptions {
    fn default() -> Self {
        Self {
            dimensions: None,
            encoding_format: EncodingFormat::Float,
            user: None,
        }
    }
}

/// Format for returned embeddings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EncodingFormat {
    /// Return embeddings as floating point numbers.
    #[default]
    Float,
    /// Return embeddings as base64-encoded strings.
    Base64,
}

impl<Req> Request<Req> {
    /// Create a new embedding request with the given payload.
    pub fn new(payload: Req) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            payload,
            options: RequestOptions::default(),
        }
    }

    /// Create a new embedding request with a specific request ID.
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

    /// Set the number of dimensions for the output embeddings.
    pub fn with_dimensions(mut self, dimensions: u32) -> Self {
        self.options.dimensions = Some(dimensions);
        self
    }

    /// Set the encoding format for the embeddings.
    pub fn with_encoding_format(mut self, format: EncodingFormat) -> Self {
        self.options.encoding_format = format;
        self
    }

    /// Set the user identifier for the request.
    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.options.user = Some(user.into());
        self
    }

    /// Validate the request parameters.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(dimensions) = self.options.dimensions
            && dimensions == 0
        {
            return Err("Dimensions must be greater than 0".to_string());
        }

        Ok(())
    }
}

/// Standard embedding request using Content inputs.
///
/// This is a convenience type for embedding operations that work directly with
/// Content inputs, providing a standard interface while maintaining compatibility
/// with the generic Request type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentEmbeddingRequest {
    /// The input(s) to generate embeddings for.
    pub inputs: Vec<Content>,
}

impl ContentEmbeddingRequest {
    /// Creates a new content embedding request.
    pub fn new() -> Self {
        Self { inputs: Vec::new() }
    }

    /// Creates a new content embedding request from inputs.
    pub fn from_inputs(inputs: Vec<Content>) -> Self {
        Self { inputs }
    }

    /// Adds a single input to the request.
    pub fn with_input(mut self, input: Content) -> Self {
        self.inputs.push(input);
        self
    }

    /// Adds a text input to the request.
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.inputs.push(Content::text(text));
        self
    }

    /// Adds a document input to the request.
    pub fn with_document(mut self, document: Document) -> Self {
        self.inputs.push(Content::document(document));
        self
    }

    /// Adds a chat input to the request.
    pub fn with_chat(mut self, chat: Chat) -> Self {
        self.inputs.push(Content::chat(chat));
        self
    }

    /// Returns the total number of inputs in this request.
    pub fn input_count(&self) -> usize {
        self.inputs.len()
    }

    /// Returns true if this request contains any text inputs.
    pub fn has_text_inputs(&self) -> bool {
        self.inputs.iter().any(|input| input.is_text())
    }

    /// Returns true if this request contains any document inputs.
    pub fn has_document_inputs(&self) -> bool {
        self.inputs.iter().any(|input| input.is_document())
    }

    /// Returns true if this request contains any chat inputs.
    pub fn has_chat_inputs(&self) -> bool {
        self.inputs.iter().any(|input| input.is_chat())
    }

    /// Estimates the total size of all inputs for rate limiting.
    pub fn estimated_total_size(&self) -> usize {
        self.inputs.iter().map(|input| input.estimated_size()).sum()
    }

    /// Validates the request parameters.
    pub fn validate(&self) -> Result<(), String> {
        if self.inputs.is_empty() {
            return Err("Request must contain at least one input".to_string());
        }

        if self.inputs.len() > 2048 {
            return Err("Too many inputs in batch request".to_string());
        }

        for (i, input) in self.inputs.iter().enumerate() {
            if let Err(err) = self.validate_input(input) {
                return Err(format!("Input {}: {}", i, err));
            }
        }

        Ok(())
    }

    fn validate_input(&self, input: &Content) -> Result<(), String> {
        match input {
            Content::Text { text } => {
                if text.is_empty() {
                    return Err("Text input cannot be empty".to_string());
                }
                if text.len() > 1_000_000 {
                    return Err("Text input too long".to_string());
                }
            }
            Content::Document { document } => {
                if document.is_empty() {
                    return Err("Document cannot be empty".to_string());
                }
                if document.size() > 10_000_000 {
                    return Err("Document too large".to_string());
                }
                if let Err(err) = document.validate() {
                    return Err(format!("Document validation failed: {}", err));
                }
            }
            Content::Chat { chat } => {
                if chat.message_count() == 0 {
                    return Err("Chat cannot be empty".to_string());
                }
                if chat.estimated_size() > 10_000_000 {
                    return Err("Chat too large".to_string());
                }
                if let Err(err) = chat.validate() {
                    return Err(format!("Chat validation failed: {}", err));
                }
            }
        }
        Ok(())
    }
}

impl Default for ContentEmbeddingRequest {
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
        let request = Request::new(());

        assert!(!request.request_id.is_nil());
        assert_eq!(request.options.dimensions, None);
        assert_eq!(request.options.encoding_format, EncodingFormat::Float);
    }

    #[test]
    fn test_request_with_options() {
        let request = Request::new(())
            .with_dimensions(512)
            .with_encoding_format(EncodingFormat::Base64)
            .with_user("test-user");

        assert_eq!(request.options.dimensions, Some(512));
        assert_eq!(request.options.encoding_format, EncodingFormat::Base64);
        assert_eq!(request.options.user, Some("test-user".to_string()));
    }

    #[test]
    fn test_content_embedding_request() {
        let document = Document::new(Bytes::from("Hello, world!")).with_content_type("text/plain");

        let payload = ContentEmbeddingRequest::new()
            .with_text("Test text")
            .with_document(document);

        assert_eq!(payload.input_count(), 2);
        assert!(payload.has_text_inputs());
        assert!(payload.has_document_inputs());
    }

    #[test]
    fn test_content_embedding_request_validation() {
        let empty_request = ContentEmbeddingRequest::new();
        assert!(empty_request.validate().is_err());

        let valid_request = ContentEmbeddingRequest::new().with_text("Hello");
        assert!(valid_request.validate().is_ok());
    }

    #[test]
    fn test_empty_text_validation() {
        let request = ContentEmbeddingRequest::new().with_text("");
        assert!(request.validate().is_err());
    }

    #[test]
    fn test_empty_document_validation() {
        let empty_document = Document::new(Bytes::new());
        let request = ContentEmbeddingRequest::new().with_document(empty_document);
        assert!(request.validate().is_err());
    }
}
