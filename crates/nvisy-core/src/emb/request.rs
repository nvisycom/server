//! Request types for embedding operations.
//!
//! This module defines the request types used for embedding generation,
//! including input handling, model configuration, and request validation.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{Chat, Content, Document};

/// Request for generating embeddings.
///
/// This struct represents a complete embedding request with all necessary
/// parameters for generating embeddings from text or image inputs.
///
/// # Examples
///
/// Creating a text embedding request:
///
/// ```rust
/// use nvisy_core::emb::{EmbeddingRequest, EmbeddingInput};
///
/// let request = EmbeddingRequest::builder()
///     .input(EmbeddingInput::text("Hello, world!"))
///     .model("text-embedding-ada-002")
///     .build()?;
/// ```
///
/// Creating a batch embedding request:
///
/// ```rust
/// let inputs = vec![
///     EmbeddingInput::text("First text"),
///     EmbeddingInput::text("Second text"),
/// ];
///
/// let request = EmbeddingRequest::builder()
///     .inputs(inputs)
///     .model("text-embedding-ada-002")
///     .dimensions(Some(512))
///     .build()?;
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    /// Unique identifier for this request.
    pub request_id: Uuid,

    /// The input(s) to generate embeddings for.
    pub inputs: Vec<Content>,

    /// The model to use for embedding generation.
    pub model: String,

    /// The format to return embeddings in.
    #[serde(default)]
    pub encoding_format: EncodingFormat,

    /// The number of dimensions the resulting output embeddings should have.
    /// Only supported in some models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,

    /// A unique identifier representing your end-user.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// Additional parameters specific to the embedding provider.
    #[serde(flatten)]
    pub additional_params: HashMap<String, serde_json::Value>,
}

/// Format for returned embeddings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EncodingFormat {
    /// Return embeddings as floating point numbers.
    Float,
    /// Return embeddings as base64-encoded strings.
    Base64,
}

impl Default for EncodingFormat {
    fn default() -> Self {
        Self::Float
    }
}

impl EmbeddingRequest {
    /// Creates a new embedding request with the specified model.
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            inputs: Vec::new(),
            model: model.into(),
            encoding_format: EncodingFormat::default(),
            dimensions: None,
            user: None,
            additional_params: HashMap::new(),
        }
    }

    /// Sets the request ID.
    pub fn with_request_id(mut self, request_id: Uuid) -> Self {
        self.request_id = request_id;
        self
    }

    /// Adds a single input to the request.
    pub fn with_input(mut self, input: Content) -> Self {
        self.inputs.push(input);
        self
    }

    /// Sets all inputs for the request.
    pub fn with_inputs(mut self, inputs: Vec<Content>) -> Self {
        self.inputs = inputs;
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

    /// Sets the encoding format for the embeddings.
    pub fn with_encoding_format(mut self, format: EncodingFormat) -> Self {
        self.encoding_format = format;
        self
    }

    /// Sets the number of dimensions for the output embeddings.
    pub fn with_dimensions(mut self, dimensions: u32) -> Self {
        self.dimensions = Some(dimensions);
        self
    }

    /// Sets the user identifier for the request.
    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// Adds an additional parameter to the request.
    pub fn with_additional_param(
        mut self,
        key: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        self.additional_params.insert(key.into(), value);
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

        if self.model.is_empty() {
            return Err("Model must be specified".to_string());
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
                // Validate document internally
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
                // Validate chat internally
                if let Err(err) = chat.validate() {
                    return Err(format!("Chat validation failed: {}", err));
                }
            }
        }
        Ok(())
    }
}

/// Builder for creating embedding requests.
///
/// This builder provides a fluent interface for constructing embedding requests
/// with proper validation and defaults.

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;

    #[test]
    fn test_embedding_input_document() {
        let document = Document::new(Bytes::from("Hello, world!")).with_content_type("text/plain");

        let input = Content::document(document.clone());

        assert!(input.is_document());
        assert_eq!(input.as_document(), Some(&document));
        assert_eq!(input.estimated_size(), 123);
    }

    #[test]
    fn test_embedding_input_image_document() {
        let document =
            Document::new(Bytes::from(vec![0x89, 0x50, 0x4E, 0x47])).with_content_type("image/png");

        let input = Content::document(document.clone());

        assert!(input.is_document());
        assert_eq!(input.as_document(), Some(&document));
    }

    #[test]
    fn test_embedding_request_with_document() {
        let document = Document::new(Bytes::from("Test content")).with_content_type("text/plain");

        let request = EmbeddingRequest::new("test-model").with_document(document.clone());

        assert_eq!(request.input_count(), 1);
        assert!(request.has_document_inputs());
        assert_eq!(request.estimated_total_size(), 122);

        // Check that the document is correctly stored
        if let Content::Document { document: doc } = &request.inputs[0] {
            assert_eq!(doc.as_text(), Some("Test content"));
        } else {
            panic!("Expected Document input");
        }
    }

    #[test]
    fn test_embedding_request_mixed_inputs() {
        let document = Document::new(Bytes::from("Document text")).with_content_type("text/plain");

        let request = EmbeddingRequest::new("test-model")
            .with_text("Hello")
            .with_document(document);

        assert_eq!(request.input_count(), 2);
        assert!(request.has_text_inputs());
        assert!(request.has_document_inputs());

        // Verify input types
        assert!(request.inputs[0].is_text());
        assert!(request.inputs[1].is_document());
    }

    #[test]
    fn test_document_input_validation() {
        let empty_document = Document::new(Bytes::new());
        let input = Content::document(empty_document);

        let request = EmbeddingRequest::new("test-model").with_input(input);

        assert!(request.validate().is_err());
    }

    #[test]
    fn test_large_document_validation() {
        let large_content = "x".repeat(11_000_000);
        let document = Document::new(Bytes::from(large_content));
        let input = Content::document(document);

        let request = EmbeddingRequest::new("test-model").with_input(input);

        assert!(request.validate().is_err());
    }

    #[test]
    fn test_embedding_input_estimated_sizes() {
        let text_input = Content::text("Hello");
        assert_eq!(text_input.estimated_size(), 5);

        let document_input = Content::document(
            Document::new(Bytes::from("Test document")).with_content_type("text/plain"),
        );
        assert_eq!(document_input.estimated_size(), 123);

        let chat_input = Content::chat(crate::types::Chat::new());
        assert_eq!(chat_input.estimated_size(), 0); // Empty chat has no size
    }
}
