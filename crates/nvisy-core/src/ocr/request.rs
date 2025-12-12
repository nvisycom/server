//! Request types for OCR operations.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::error::{Error, Result};

/// Request for OCR operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Unique identifier for this request.
    pub request_id: uuid::Uuid,
    /// Image data to process.
    pub image_data: Vec<u8>,
    /// MIME type of the image.
    pub mime_type: String,
    /// Processing options.
    pub options: RequestOptions,
}

/// Processing options for OCR requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestOptions {
    /// Whether to detect tables in the document.
    pub detect_tables: bool,
    /// Whether to preserve layout information.
    pub preserve_layout: bool,
    /// Minimum confidence threshold for text extraction.
    pub confidence_threshold: Option<f32>,
    /// DPI setting for image processing.
    pub dpi: Option<u32>,
    /// Whether to preprocess images for better OCR results.
    pub preprocess_image: bool,
    /// Custom parameters for specific OCR engines.
    pub custom_parameters: HashMap<String, serde_json::Value>,
}

impl Default for RequestOptions {
    fn default() -> Self {
        Self {
            detect_tables: false,
            preserve_layout: true,
            confidence_threshold: Some(0.5),
            dpi: Some(300),
            preprocess_image: true,
            custom_parameters: HashMap::new(),
        }
    }
}

impl Request {
    /// Create a new OCR request.
    pub fn new(image_data: Vec<u8>, mime_type: String) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4(),
            image_data,
            mime_type,
            options: RequestOptions::default(),
        }
    }

    /// Create a new request with custom options.
    pub fn with_options(image_data: Vec<u8>, mime_type: String, options: RequestOptions) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4(),
            image_data,
            mime_type,
            options,
        }
    }

    /// Set whether to detect tables.
    pub fn with_table_detection(mut self, detect_tables: bool) -> Self {
        self.options.detect_tables = detect_tables;
        self
    }

    /// Set whether to preserve layout.
    pub fn with_layout_preservation(mut self, preserve_layout: bool) -> Self {
        self.options.preserve_layout = preserve_layout;
        self
    }

    /// Set confidence threshold.
    pub fn with_confidence_threshold(mut self, threshold: f32) -> Self {
        self.options.confidence_threshold = Some(threshold);
        self
    }

    /// Set DPI for processing.
    pub fn with_dpi(mut self, dpi: u32) -> Self {
        self.options.dpi = Some(dpi);
        self
    }

    /// Enable or disable image preprocessing.
    pub fn with_preprocessing(mut self, preprocess: bool) -> Self {
        self.options.preprocess_image = preprocess;
        self
    }

    /// Add a custom parameter.
    pub fn with_custom_parameter(mut self, key: String, value: serde_json::Value) -> Self {
        self.options.custom_parameters.insert(key, value);
        self
    }

    /// Validate the request.
    pub fn validate(&self) -> Result<()> {
        if self.image_data.is_empty() {
            return Err(Error::invalid_input());
        }

        if self.mime_type.is_empty() {
            return Err(Error::invalid_input());
        }

        // Check for supported image formats
        let supported_formats = [
            "image/jpeg",
            "image/jpg",
            "image/png",
            "image/tiff",
            "image/bmp",
            "image/webp",
            "application/pdf",
        ];

        if !supported_formats.contains(&self.mime_type.as_str()) {
            return Err(Error::unsupported_format());
        }

        // Check image size (max 10MB)
        if self.image_data.len() > 10 * 1024 * 1024 {
            return Err(Error::image_too_large());
        }

        // Check confidence threshold
        if let Some(threshold) = self.options.confidence_threshold {
            if threshold < 0.0 || threshold > 1.0 {
                return Err(Error::invalid_input());
            }
        }

        Ok(())
    }

    /// Get the size of the image data in bytes.
    pub fn image_size(&self) -> usize {
        self.image_data.len()
    }

    /// Check if this is a PDF document.
    pub fn is_pdf(&self) -> bool {
        self.mime_type == "application/pdf"
    }

    /// Check if this is an image file.
    pub fn is_image(&self) -> bool {
        self.mime_type.starts_with("image/")
    }
}
