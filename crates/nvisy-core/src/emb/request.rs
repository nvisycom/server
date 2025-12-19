//! Request types for embedding operations.
//!
//! This module defines the request types used for embedding generation,
//! including input handling, model configuration, and request validation.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::Document;

/// Request for generating embeddings.
///
/// This struct represents a complete embedding request with all necessary
/// parameters for generating embeddings from text or image inputs.
///
/// # Examples
///
/// Creating a text embedding request:
///
/// ```rust,ignore
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
/// ```rust,ignore
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
    pub inputs: Vec<EmbeddingInput>,

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

/// Input types for embedding generation.
///
/// This enum represents the different types of input that can be used
/// to generate embeddings, including text and various image formats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EmbeddingInput {
    /// Plain text input.
    Text(String),

    /// Document input using the unified Document type.
    Document(Document),

    /// Base64-encoded image data with optional MIME type.
    Image {
        /// Base64-encoded image data.
        data: String,
        /// MIME type of the image (e.g., "image/jpeg", "image/png").
        mime_type: Option<String>,
    },

    /// Image from a URL.
    ImageUrl {
        /// URL of the image to embed.
        url: String,
    },

    /// Raw bytes with optional content type.
    Bytes {
        /// Raw byte data.
        data: Vec<u8>,
        /// Content type of the data.
        content_type: Option<String>,
    },
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

impl EmbeddingInput {
    /// Creates a text input.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    /// Creates a document input.
    pub fn document(document: Document) -> Self {
        Self::Document(document)
    }

    /// Creates an image input from base64-encoded data.
    pub fn image_base64(data: impl Into<String>, mime_type: Option<String>) -> Self {
        Self::Image {
            data: data.into(),
            mime_type,
        }
    }

    /// Creates an image input from a URL.
    pub fn image_url(url: impl Into<String>) -> Self {
        Self::ImageUrl { url: url.into() }
    }

    /// Creates a bytes input.
    pub fn bytes(data: Vec<u8>, content_type: Option<String>) -> Self {
        Self::Bytes { data, content_type }
    }

    /// Returns true if this is a text input.
    pub fn is_text(&self) -> bool {
        match self {
            Self::Text(_) => true,
            Self::Document(doc) => doc.is_text(),
            _ => false,
        }
    }

    /// Returns true if this is an image input.
    pub fn is_image(&self) -> bool {
        match self {
            Self::Image { .. } | Self::ImageUrl { .. } => true,
            Self::Document(doc) => doc.is_image(),
            _ => false,
        }
    }

    /// Returns true if this is a document input.
    pub fn is_document(&self) -> bool {
        matches!(self, Self::Document(_))
    }

    /// Returns the text content if this is a text input.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Document(doc) if doc.is_text() => doc.as_text(),
            _ => None,
        }
    }

    /// Returns the document if this is a document input.
    pub fn as_document(&self) -> Option<&Document> {
        match self {
            Self::Document(doc) => Some(doc),
            _ => None,
        }
    }

    /// Estimates the size of this input for rate limiting and validation.
    pub fn estimated_size(&self) -> usize {
        match self {
            Self::Text(text) => text.len(),
            Self::Document(doc) => doc.size(),
            Self::Image { data, .. } => data.len(),
            Self::ImageUrl { url } => url.len(),
            Self::Bytes { data, .. } => data.len(),
        }
    }
}

impl EmbeddingRequest {
    /// Creates a new request builder.
    pub fn builder() -> EmbeddingRequestBuilder {
        EmbeddingRequestBuilder::new()
    }

    /// Returns the total number of inputs in this request.
    pub fn input_count(&self) -> usize {
        self.inputs.len()
    }

    /// Returns true if this request contains any text inputs.
    pub fn has_text_inputs(&self) -> bool {
        self.inputs.iter().any(|input| input.is_text())
    }

    /// Returns true if this request contains any image inputs.
    pub fn has_image_inputs(&self) -> bool {
        self.inputs.iter().any(|input| input.is_image())
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

    fn validate_input(&self, input: &EmbeddingInput) -> Result<(), String> {
        match input {
            EmbeddingInput::Text(text) => {
                if text.is_empty() {
                    return Err("Text input cannot be empty".to_string());
                }
                if text.len() > 1_000_000 {
                    return Err("Text input too long".to_string());
                }
            }
            EmbeddingInput::Document(doc) => {
                if doc.is_empty() {
                    return Err("Document cannot be empty".to_string());
                }
                if doc.size() > 10_000_000 {
                    return Err("Document too large".to_string());
                }
                // Validate document internally
                if let Err(err) = doc.validate() {
                    return Err(format!("Document validation failed: {}", err));
                }
            }
            EmbeddingInput::Image { data, .. } => {
                if data.is_empty() {
                    return Err("Image data cannot be empty".to_string());
                }
                // Basic base64 validation
                if !data
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
                {
                    return Err("Invalid base64 image data".to_string());
                }
            }
            EmbeddingInput::ImageUrl { url } => {
                if url.is_empty() {
                    return Err("Image URL cannot be empty".to_string());
                }
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    return Err("Image URL must be HTTP or HTTPS".to_string());
                }
            }
            EmbeddingInput::Bytes { data, .. } => {
                if data.is_empty() {
                    return Err("Bytes data cannot be empty".to_string());
                }
                if data.len() > 10_000_000 {
                    return Err("Bytes data too large".to_string());
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
#[derive(Debug, Clone)]
pub struct EmbeddingRequestBuilder {
    request_id: Option<Uuid>,
    inputs: Vec<EmbeddingInput>,
    model: Option<String>,
    encoding_format: EncodingFormat,
    dimensions: Option<u32>,
    user: Option<String>,
    additional_params: HashMap<String, serde_json::Value>,
}

impl EmbeddingRequestBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self {
            request_id: None,
            inputs: Vec::new(),
            model: None,
            encoding_format: EncodingFormat::default(),
            dimensions: None,
            user: None,
            additional_params: HashMap::new(),
        }
    }

    /// Sets the request ID.
    pub fn request_id(mut self, request_id: Uuid) -> Self {
        self.request_id = Some(request_id);
        self
    }

    /// Adds a single input to the request.
    pub fn input(mut self, input: EmbeddingInput) -> Self {
        self.inputs.push(input);
        self
    }

    /// Sets all inputs for the request.
    pub fn inputs(mut self, inputs: Vec<EmbeddingInput>) -> Self {
        self.inputs = inputs;
        self
    }

    /// Adds a text input to the request.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.inputs.push(EmbeddingInput::text(text));
        self
    }

    /// Adds a document input to the request.
    pub fn document(mut self, document: Document) -> Self {
        self.inputs.push(EmbeddingInput::document(document));
        self
    }

    /// Adds an image input from base64 data.
    pub fn image_base64(mut self, data: impl Into<String>, mime_type: Option<String>) -> Self {
        self.inputs
            .push(EmbeddingInput::image_base64(data, mime_type));
        self
    }

    /// Adds an image input from a URL.
    pub fn image_url(mut self, url: impl Into<String>) -> Self {
        self.inputs.push(EmbeddingInput::image_url(url));
        self
    }

    /// Sets the model to use for embedding generation.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Sets the encoding format for the embeddings.
    pub fn encoding_format(mut self, format: EncodingFormat) -> Self {
        self.encoding_format = format;
        self
    }

    /// Sets the number of dimensions for the output embeddings.
    pub fn dimensions(mut self, dimensions: Option<u32>) -> Self {
        self.dimensions = dimensions;
        self
    }

    /// Sets the user identifier for the request.
    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// Adds an additional parameter to the request.
    pub fn additional_param(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.additional_params.insert(key.into(), value);
        self
    }

    /// Builds the embedding request.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing or if validation fails.
    pub fn build(self) -> Result<EmbeddingRequest, String> {
        let request = EmbeddingRequest {
            request_id: self.request_id.unwrap_or_else(Uuid::new_v4),
            inputs: self.inputs,
            model: self.model.ok_or("Model must be specified")?,
            encoding_format: self.encoding_format,
            dimensions: self.dimensions,
            user: self.user,
            additional_params: self.additional_params,
        };

        request.validate()?;
        Ok(request)
    }
}

impl Default for EmbeddingRequestBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;

    #[test]
    fn test_embedding_input_document() {
        let document = Document::new(Bytes::from("Hello, world!")).with_content_type("text/plain");

        let input = EmbeddingInput::document(document.clone());

        assert!(input.is_document());
        assert!(input.is_text());
        assert!(!input.is_image());
        assert_eq!(input.as_text(), Some("Hello, world!"));
        assert_eq!(input.as_document(), Some(&document));
        assert_eq!(input.estimated_size(), 13);
    }

    #[test]
    fn test_embedding_input_image_document() {
        let document =
            Document::new(Bytes::from(vec![0x89, 0x50, 0x4E, 0x47])).with_content_type("image/png");

        let input = EmbeddingInput::document(document.clone());

        assert!(input.is_document());
        assert!(input.is_image());
        assert!(!input.is_text());
        assert_eq!(input.as_text(), None);
        assert_eq!(input.as_document(), Some(&document));
    }

    #[test]
    fn test_embedding_request_with_document() {
        let document = Document::new(Bytes::from("Test content")).with_content_type("text/plain");

        let request = EmbeddingRequest::builder()
            .document(document.clone())
            .model("test-model")
            .build()
            .unwrap();

        assert_eq!(request.input_count(), 1);
        assert!(request.has_text_inputs());
        assert!(!request.has_image_inputs());
        assert_eq!(request.estimated_total_size(), 12);

        // Check that the document is correctly stored
        if let EmbeddingInput::Document(doc) = &request.inputs[0] {
            assert_eq!(doc.as_text(), Some("Test content"));
        } else {
            panic!("Expected Document input");
        }
    }

    #[test]
    fn test_embedding_request_mixed_inputs() {
        let document = Document::new(Bytes::from("Document text")).with_content_type("text/plain");

        let request = EmbeddingRequest::builder()
            .text("Plain text")
            .document(document)
            .image_base64("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==", Some("image/png".to_string()))
            .model("test-model")
            .build()
            .unwrap();

        assert_eq!(request.input_count(), 3);
        assert!(request.has_text_inputs());
        assert!(request.has_image_inputs());

        // Verify input types
        assert!(request.inputs[0].is_text());
        assert!(request.inputs[1].is_text() && request.inputs[1].is_document());
        assert!(request.inputs[2].is_image());
    }

    #[test]
    fn test_document_input_validation() {
        let empty_document = Document::new(Bytes::new());
        let input = EmbeddingInput::document(empty_document);

        let request = EmbeddingRequest::builder()
            .input(input)
            .model("test-model")
            .build();

        assert!(request.is_err());
        assert!(request.unwrap_err().contains("Document cannot be empty"));
    }

    #[test]
    fn test_large_document_validation() {
        let large_content = "x".repeat(11_000_000);
        let document = Document::new(Bytes::from(large_content));
        let input = EmbeddingInput::document(document);

        let request = EmbeddingRequest::builder()
            .input(input)
            .model("test-model")
            .build();

        assert!(request.is_err());
        assert!(request.unwrap_err().contains("Document too large"));
    }

    #[test]
    fn test_embedding_input_estimated_sizes() {
        let text_input = EmbeddingInput::text("Hello");
        assert_eq!(text_input.estimated_size(), 5);

        let document_input = EmbeddingInput::document(
            Document::new(Bytes::from("Hello")).with_content_type("text/plain"),
        );
        assert_eq!(document_input.estimated_size(), 5);

        let image_input = EmbeddingInput::image_base64("dGVzdA==", None);
        assert_eq!(image_input.estimated_size(), 8);
    }
}
