//! Request types for VLM operations.
//!
//! This module provides types for constructing VLM requests, including text prompts,
//! image inputs, and processing options. Requests support both single and multi-image
//! scenarios for various multimodal AI tasks.

use std::collections::HashMap;

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::types::{Document, Message};
use crate::{Error, Result};

/// Request for VLM operations.
///
/// Generic over the implementation-specific request payload type `Req`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request<Req> {
    /// Unique identifier for this request.
    pub request_id: uuid::Uuid,
    /// Text prompt for the VLM.
    pub prompt: String,
    /// Optional images to analyze.
    pub images: Vec<ImageInput>,
    /// Optional documents to analyze.
    pub documents: Vec<Document>,
    /// Optional conversation context using Message types.
    pub messages: Vec<Message>,
    /// Processing options.
    pub options: RequestOptions,
    /// Implementation-specific request payload.
    pub payload: Req,
}

/// Processing options for VLM requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestOptions {
    /// Maximum number of tokens to generate.
    pub max_tokens: Option<u32>,
    /// Temperature for response generation (0.0 to 1.0).
    pub temperature: Option<f32>,
    /// Whether to enable streaming responses.
    pub streaming: bool,
    /// Custom parameters for specific VLM engines.
    pub custom_parameters: HashMap<String, serde_json::Value>,
}

impl Default for RequestOptions {
    fn default() -> Self {
        Self {
            max_tokens: Some(1024),
            temperature: Some(0.7),
            streaming: false,
            custom_parameters: HashMap::new(),
        }
    }
}

/// Image input for VLM processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInput {
    /// Unique identifier for this image.
    pub id: uuid::Uuid,
    /// Image data as base64 encoded string.
    pub data: String,
    /// MIME type of the image.
    pub mime_type: String,
    /// Optional filename or description.
    pub filename: Option<String>,
    /// Optional detail level for processing.
    pub detail_level: Option<String>,
}

/// VLM input that can handle various content types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum VlmInput {
    /// Text input.
    Text(String),
    /// Document input using the unified Document type.
    Document(Document),
    /// Image input with processing options.
    Image(ImageInput),
    /// Message for conversation context.
    Message(Message),
}

impl ImageInput {
    /// Create new image input from bytes.
    pub fn from_bytes(data: Vec<u8>, mime_type: String) -> Result<Self> {
        if data.is_empty() {
            return Err(Error::invalid_input());
        }

        let base64_data = base64::engine::general_purpose::STANDARD.encode(&data);
        Ok(Self {
            id: uuid::Uuid::new_v4(),
            data: base64_data,
            mime_type,
            filename: None,
            detail_level: None,
        })
    }

    /// Create image input from a Document.
    pub fn from_document(document: &Document) -> Result<Self> {
        if !document.is_image() {
            return Err(Error::invalid_input());
        }

        let mime_type = document
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();

        let base64_data = base64::engine::general_purpose::STANDARD.encode(document.as_bytes());

        Ok(Self {
            id: uuid::Uuid::new_v4(),
            data: base64_data,
            mime_type,
            filename: document.filename().map(|s| s.to_string()),
            detail_level: None,
        })
    }

    /// Set filename for this image.
    pub fn with_filename(mut self, filename: String) -> Self {
        self.filename = Some(filename);
        self
    }

    /// Set detail level for processing.
    pub fn with_detail_level(mut self, level: String) -> Self {
        self.detail_level = Some(level);
        self
    }

    /// Get the estimated size of the base64 data.
    pub fn estimated_size(&self) -> usize {
        // Base64 encoding increases size by ~33%
        (self.data.len() * 3) / 4
    }
}

impl<Req> Request<Req> {
    /// Create a new VLM request with text only.
    pub fn new(prompt: String, payload: Req) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4(),
            prompt,
            images: Vec::new(),
            documents: Vec::new(),
            messages: Vec::new(),
            options: RequestOptions::default(),
            payload,
        }
    }

    /// Create a new request with text and images.
    pub fn with_images(prompt: String, images: Vec<ImageInput>, payload: Req) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4(),
            prompt,
            images,
            documents: Vec::new(),
            messages: Vec::new(),
            options: RequestOptions::default(),
            payload,
        }
    }

    /// Create a new request with custom options.
    pub fn with_options(prompt: String, options: RequestOptions, payload: Req) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4(),
            prompt,
            images: Vec::new(),
            documents: Vec::new(),
            messages: Vec::new(),
            options,
            payload,
        }
    }

    /// Add an image to this request.
    pub fn add_image(mut self, image: ImageInput) -> Self {
        self.images.push(image);
        self
    }

    /// Add a document to this request.
    pub fn add_document(mut self, document: Document) -> Self {
        self.documents.push(document);
        self
    }

    /// Add a message to this request for conversation context.
    pub fn add_message(mut self, message: Message) -> Self {
        self.messages.push(message);
        self
    }

    /// Add documents to this request.
    pub fn with_documents(mut self, documents: Vec<Document>) -> Self {
        self.documents = documents;
        self
    }

    /// Add messages to this request for conversation context.
    pub fn with_messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
        self
    }

    /// Set maximum tokens to generate.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.options.max_tokens = Some(max_tokens);
        self
    }

    /// Set temperature for response generation.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.options.temperature = Some(temperature);
        self
    }

    /// Enable streaming responses.
    pub fn with_streaming(mut self, streaming: bool) -> Self {
        self.options.streaming = streaming;
        self
    }

    /// Add a custom parameter.
    pub fn with_custom_parameter(mut self, key: String, value: serde_json::Value) -> Self {
        self.options.custom_parameters.insert(key, value);
        self
    }

    /// Validate the request.
    pub fn validate(&self) -> Result<()> {
        if self.prompt.trim().is_empty() {
            return Err(Error::invalid_input());
        }

        // Check image count limits
        if self.images.len() > 10 {
            return Err(Error::invalid_input());
        }

        // Check document count limits
        if self.documents.len() > 20 {
            return Err(Error::invalid_input());
        }

        // Check message count limits
        if self.messages.len() > 100 {
            return Err(Error::invalid_input());
        }

        // Validate each image
        for image in &self.images {
            if image.data.is_empty() {
                return Err(Error::invalid_input());
            }

            if image.mime_type.is_empty() {
                return Err(Error::invalid_input());
            }

            // Check for supported image formats
            let supported_formats = [
                "image/jpeg",
                "image/jpg",
                "image/png",
                "image/webp",
                "image/gif",
            ];

            if !supported_formats.contains(&image.mime_type.as_str()) {
                return Err(Error::invalid_input().with_message("Unsupported image format"));
            }

            // Check image size (rough estimate from base64)
            if image.estimated_size() > 20 * 1024 * 1024 {
                return Err(Error::invalid_input());
            }
        }

        // Validate each document
        for document in &self.documents {
            if document.is_empty() {
                return Err(Error::invalid_input());
            }

            // Check document size
            if document.size() > 50 * 1024 * 1024 {
                return Err(Error::invalid_input());
            }
        }

        // Validate messages
        for message in &self.messages {
            if message.content.trim().is_empty() && message.content_parts.is_empty() {
                return Err(Error::invalid_input());
            }
        }

        // Check temperature range
        if let Some(temp) = self.options.temperature
            && (!(0.0..=2.0).contains(&temp))
        {
            return Err(Error::invalid_input());
        }

        // Check max tokens
        if let Some(max_tokens) = self.options.max_tokens
            && (max_tokens == 0 || max_tokens > 8192)
        {
            return Err(Error::invalid_input());
        }

        Ok(())
    }

    /// Check if this request has images.
    pub fn has_images(&self) -> bool {
        !self.images.is_empty()
    }

    /// Get the number of images in this request.
    pub fn image_count(&self) -> usize {
        self.images.len()
    }

    /// Check if this request has documents.
    pub fn has_documents(&self) -> bool {
        !self.documents.is_empty()
    }

    /// Get the number of documents in this request.
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Check if this request has messages.
    pub fn has_messages(&self) -> bool {
        !self.messages.is_empty()
    }

    /// Get the number of messages in this request.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Check if this is a text-only request.
    pub fn is_text_only(&self) -> bool {
        self.images.is_empty() && self.documents.is_empty()
    }

    /// Get the estimated total size of all images.
    pub fn total_image_size(&self) -> usize {
        self.images.iter().map(|img| img.estimated_size()).sum()
    }

    /// Get the total size of all documents.
    pub fn total_document_size(&self) -> usize {
        self.documents.iter().map(|doc| doc.size()).sum()
    }

    /// Get the estimated total content size.
    pub fn total_content_size(&self) -> usize {
        self.prompt.len()
            + self.total_image_size()
            + self.total_document_size()
            + self.messages.iter().map(|m| m.content.len()).sum::<usize>()
    }

    /// Get the prompt length in characters.
    pub fn prompt_length(&self) -> usize {
        self.prompt.chars().count()
    }

    /// Check if streaming is enabled.
    pub fn is_streaming(&self) -> bool {
        self.options.streaming
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;
    use crate::types::{Document, Message, MessageRole};

    #[test]
    fn test_vlm_request_with_documents() {
        let document1 =
            Document::new(Bytes::from("Test document 1")).with_content_type("text/plain");
        let document2 =
            Document::new(Bytes::from("Test document 2")).with_content_type("application/pdf");

        let request = Request::new("Analyze these documents".to_string(), ())
            .add_document(document1)
            .add_document(document2);

        assert!(request.has_documents());
        assert_eq!(request.document_count(), 2);
        assert!(!request.is_text_only());
        assert_eq!(request.total_document_size(), 30); // 15 + 15
    }

    #[test]
    fn test_vlm_request_with_messages() {
        let message1 = Message::new(MessageRole::User, "Previous question");

        let message2 = Message::new(MessageRole::Assistant, "Previous response");

        let request = Request::new("Continue conversation".to_string(), ())
            .add_message(message1)
            .add_message(message2);

        assert!(request.has_messages());
        assert_eq!(request.message_count(), 2);
        assert!(request.is_text_only()); // No images or visual documents
    }

    #[test]
    fn test_image_input_from_document() {
        let image_document = Document::new(Bytes::from(vec![0x89, 0x50, 0x4E, 0x47]))
            .with_content_type("image/png")
            .with_filename("test.png");

        let image_input = ImageInput::from_document(&image_document).unwrap();

        assert_eq!(image_input.mime_type, "image/png");
        assert_eq!(image_input.filename, Some("test.png".to_string()));
        assert!(!image_input.data.is_empty());
    }

    #[test]
    fn test_image_input_from_non_image_document() {
        let text_document =
            Document::new(Bytes::from("Not an image")).with_content_type("text/plain");

        let result = ImageInput::from_document(&text_document);
        assert!(result.is_err());
    }

    #[test]
    fn test_vlm_request_validation_success() {
        let document = Document::new(Bytes::from("Valid content")).with_content_type("text/plain");
        let image = ImageInput::from_bytes(vec![1, 2, 3, 4], "image/jpeg".to_string()).unwrap();

        let request = Request::new("Valid prompt".to_string(), ())
            .add_document(document)
            .add_image(image)
            .with_max_tokens(100)
            .with_temperature(0.7);

        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_vlm_request_validation_empty_prompt() {
        let request = Request::new("   ".to_string(), ());
        let result = request.validate();

        assert!(result.is_err());
    }

    #[test]
    fn test_vlm_request_validation_too_many_images() {
        let mut request = Request::new("Test prompt".to_string(), ());

        // Add more than 10 images
        for i in 0..11 {
            let image = ImageInput::from_bytes(vec![i as u8], "image/jpeg".to_string()).unwrap();
            request = request.add_image(image);
        }

        let result = request.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_vlm_request_validation_too_many_documents() {
        let mut request = Request::new("Test prompt".to_string(), ());

        // Add more than 20 documents
        for i in 0..21 {
            let document =
                Document::new(Bytes::from(format!("Doc {}", i))).with_content_type("text/plain");
            request = request.add_document(document);
        }

        let result = request.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_vlm_request_validation_large_document() {
        let large_content = "x".repeat(51 * 1024 * 1024); // 51MB
        let document = Document::new(Bytes::from(large_content)).with_content_type("text/plain");

        let request = Request::new("Test prompt".to_string(), ()).add_document(document);

        let result = request.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_vlm_request_validation_invalid_temperature() {
        let request = Request::new("Test prompt".to_string(), ()).with_temperature(3.0); // Invalid temperature > 2.0

        let result = request.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_vlm_request_validation_invalid_max_tokens() {
        let request = Request::new("Test prompt".to_string(), ()).with_max_tokens(0); // Invalid max_tokens

        let result = request.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_vlm_request_total_content_size() {
        let document =
            Document::new(Bytes::from("Document content")).with_content_type("text/plain");
        let message = Message::new(MessageRole::User, "Message content");

        let request = Request::new("Prompt content".to_string(), ())
            .add_document(document)
            .add_message(message);

        // Prompt: 14 chars, Document: 16 chars, Message: 15 chars = 45 total
        assert_eq!(request.total_content_size(), 45);
    }

    #[test]
    fn test_vlm_request_mixed_content() {
        let document = Document::new(Bytes::from("Text document")).with_content_type("text/plain");
        let image = ImageInput::from_bytes(vec![1, 2, 3, 4], "image/png".to_string()).unwrap();
        let message = Message::new(MessageRole::User, "Context message");

        let request = Request::new("Analyze all content".to_string(), ())
            .add_document(document)
            .add_image(image)
            .add_message(message);

        assert!(request.has_documents());
        assert!(request.has_images());
        assert!(request.has_messages());
        assert!(!request.is_text_only());
        assert_eq!(request.document_count(), 1);
        assert_eq!(request.image_count(), 1);
        assert_eq!(request.message_count(), 1);
    }

    #[test]
    fn test_vlm_input_variants() {
        let text_input = VlmInput::Text("Hello".to_string());
        let document_input =
            VlmInput::Document(Document::new(Bytes::from("Doc")).with_content_type("text/plain"));
        let image_input = VlmInput::Image(
            ImageInput::from_bytes(vec![1, 2, 3], "image/jpg".to_string()).unwrap(),
        );
        let message_input = VlmInput::Message(Message::new(MessageRole::User, "Test"));

        // Test that all variants are created correctly
        match text_input {
            VlmInput::Text(s) => assert_eq!(s, "Hello"),
            _ => panic!("Expected Text variant"),
        }

        match document_input {
            VlmInput::Document(doc) => assert_eq!(doc.size(), 3),
            _ => panic!("Expected Document variant"),
        }

        match image_input {
            VlmInput::Image(img) => assert_eq!(img.mime_type, "image/jpg"),
            _ => panic!("Expected Image variant"),
        }

        match message_input {
            VlmInput::Message(msg) => assert_eq!(msg.content, "Test"),
            _ => panic!("Expected Message variant"),
        }
    }
}
