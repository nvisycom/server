//! Request types for VLM operations.
//!
//! This module provides types for constructing VLM requests, including text prompts,
//! image inputs, and processing options. Requests support both single and multi-image
//! scenarios for various multimodal AI tasks.

use std::collections::HashMap;

use base64::Engine;
use serde::{Deserialize, Serialize};

use super::error::{Error, Result};

/// Request for VLM operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Unique identifier for this request.
    pub request_id: uuid::Uuid,
    /// Text prompt for the VLM.
    pub prompt: String,
    /// Optional images to analyze.
    pub images: Vec<ImageInput>,
    /// Processing options.
    pub options: RequestOptions,
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

impl Request {
    /// Create a new VLM request with text only.
    pub fn new(prompt: String) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4(),
            prompt,
            images: Vec::new(),
            options: RequestOptions::default(),
        }
    }

    /// Create a new request with text and images.
    pub fn with_images(prompt: String, images: Vec<ImageInput>) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4(),
            prompt,
            images,
            options: RequestOptions::default(),
        }
    }

    /// Create a new request with custom options.
    pub fn with_options(prompt: String, options: RequestOptions) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4(),
            prompt,
            images: Vec::new(),
            options,
        }
    }

    /// Add an image to this request.
    pub fn add_image(mut self, image: ImageInput) -> Self {
        self.images.push(image);
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
            return Err(Error::invalid_prompt());
        }

        // Check image count limits
        if self.images.len() > 10 {
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
                return Err(Error::unsupported_format());
            }

            // Check image size (rough estimate from base64)
            if image.estimated_size() > 20 * 1024 * 1024 {
                return Err(Error::image_too_large());
            }
        }

        // Check temperature range
        if let Some(temp) = self.options.temperature {
            if temp < 0.0 || temp > 2.0 {
                return Err(Error::invalid_input());
            }
        }

        // Check max tokens
        if let Some(max_tokens) = self.options.max_tokens {
            if max_tokens == 0 || max_tokens > 8192 {
                return Err(Error::invalid_input());
            }
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

    /// Check if this is a text-only request.
    pub fn is_text_only(&self) -> bool {
        self.images.is_empty()
    }

    /// Get the estimated total size of all images.
    pub fn total_image_size(&self) -> usize {
        self.images.iter().map(|img| img.estimated_size()).sum()
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
