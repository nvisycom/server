//! OCR response types.

use std::collections::HashMap;

use derive_builder::Builder;
use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::service::UsageStats;
use crate::types::{BoundingBox, Timing};

/// Individual text extraction with positional information.
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(
    name = "TextExtractionBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(private, name = "build_inner", error = "TextExtractionError")
)]
pub struct TextExtraction {
    /// The extracted text content.
    pub text: String,
    /// Bounding box for the text region.
    #[builder(default)]
    pub bounding_box: Option<BoundingBox>,
    /// Page number (for multi-page documents).
    #[builder(default)]
    pub page: Option<u32>,
}

/// Error type for TextExtraction builder.
pub type TextExtractionError = derive_builder::UninitializedFieldError;

impl TextExtractionBuilder {
    /// Build the extraction.
    pub fn build(self) -> Result<TextExtraction, TextExtractionError> {
        self.build_inner()
    }
}

impl TextExtraction {
    /// Create a new text extraction.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bounding_box: None,
            page: None,
        }
    }

    /// Create a builder for this extraction.
    pub fn builder() -> TextExtractionBuilder {
        TextExtractionBuilder::default()
    }

    /// Set the bounding box.
    pub fn with_bounding_box(mut self, bounding_box: BoundingBox) -> Self {
        self.bounding_box = Some(bounding_box);
        self
    }

    /// Set the page number.
    pub fn with_page(mut self, page: u32) -> Self {
        self.page = Some(page);
        self
    }
}

/// Response from a single OCR operation.
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(
    name = "OcrResponseBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(private, name = "build_inner", error = "OcrResponseError")
)]
pub struct OcrResponse {
    /// Unique identifier for this response.
    #[builder(default = "Uuid::now_v7()")]
    pub response_id: Uuid,
    /// Request ID this response corresponds to.
    pub request_id: Uuid,
    /// Extracted text from the document.
    pub text: String,
    /// Individual text extractions with positional information.
    #[builder(default)]
    pub extractions: Vec<TextExtraction>,
    /// Timing information for the operation.
    #[builder(default)]
    pub timing: Option<Timing>,
    /// Additional metadata about the processing.
    #[builder(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    /// Usage statistics for this operation.
    #[builder(default)]
    pub usage: Option<UsageStats>,
}

/// Error type for OcrResponse builder.
pub type OcrResponseError = derive_builder::UninitializedFieldError;

impl OcrResponseBuilder {
    /// Build the response.
    pub fn build(self) -> Result<OcrResponse, OcrResponseError> {
        self.build_inner()
    }

    /// Add an extraction to this response.
    pub fn add_extraction(mut self, extraction: TextExtraction) -> Self {
        self.extractions
            .get_or_insert_with(Vec::new)
            .push(extraction);
        self
    }

    /// Add metadata to this response.
    pub fn add_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value);
        self
    }
}

impl OcrResponse {
    /// Create a new OCR response with the given text.
    pub fn new(request_id: Uuid, text: impl Into<String>) -> Self {
        Self {
            response_id: Uuid::now_v7(),
            request_id,
            text: text.into(),
            extractions: Vec::new(),
            timing: None,
            metadata: HashMap::new(),
            usage: None,
        }
    }

    /// Create a new OCR response with text and extractions.
    pub fn with_extractions(
        request_id: Uuid,
        text: impl Into<String>,
        extractions: Vec<TextExtraction>,
    ) -> Self {
        Self {
            response_id: Uuid::now_v7(),
            request_id,
            text: text.into(),
            extractions,
            timing: None,
            metadata: HashMap::new(),
            usage: None,
        }
    }

    /// Create a builder for this response.
    pub fn builder() -> OcrResponseBuilder {
        OcrResponseBuilder::default()
    }

    /// Set timing information.
    pub fn with_timing(mut self, started_at: Timestamp, ended_at: Timestamp) -> Self {
        self.timing = Some(Timing::new(started_at, ended_at));
        self
    }

    /// Get the extracted text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get the processing time as a signed duration.
    pub fn processing_time(&self) -> Option<SignedDuration> {
        self.timing.map(|t| t.duration())
    }

    /// Get the number of text extractions.
    pub fn extraction_count(&self) -> usize {
        self.extractions.len()
    }

    /// Check if the OCR result is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && self.extractions.is_empty()
    }
}

/// Batch response containing multiple OCR results.
#[derive(Debug, Clone, Builder, Serialize, Deserialize)]
#[builder(
    name = "OcrBatchResponseBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(private, name = "build_inner", error = "OcrBatchResponseError")
)]
pub struct OcrBatchResponse {
    /// Unique identifier for this batch response.
    #[builder(default = "Uuid::now_v7()")]
    pub batch_id: Uuid,
    /// Individual responses in the batch.
    #[builder(default)]
    pub responses: Vec<OcrResponse>,
}

/// Error type for OcrBatchResponse builder.
pub type OcrBatchResponseError = derive_builder::UninitializedFieldError;

impl OcrBatchResponseBuilder {
    /// Build the response.
    pub fn build(self) -> Result<OcrBatchResponse, OcrBatchResponseError> {
        self.build_inner()
    }

    /// Add a response to the batch.
    pub fn add_response(mut self, response: OcrResponse) -> Self {
        self.responses.get_or_insert_with(Vec::new).push(response);
        self
    }
}

impl OcrBatchResponse {
    /// Create a new batch response.
    pub fn new(responses: Vec<OcrResponse>) -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            responses,
        }
    }

    /// Create a builder for this response.
    pub fn builder() -> OcrBatchResponseBuilder {
        OcrBatchResponseBuilder::default()
    }

    /// Get the earliest start time from all responses.
    pub fn started_at(&self) -> Option<Timestamp> {
        self.responses
            .iter()
            .filter_map(|r| r.timing.map(|t| t.started_at))
            .min()
    }

    /// Get the latest end time from all responses.
    pub fn ended_at(&self) -> Option<Timestamp> {
        self.responses
            .iter()
            .filter_map(|r| r.timing.map(|t| t.ended_at))
            .max()
    }

    /// Get the total processing time as a signed duration.
    pub fn processing_time(&self) -> Option<SignedDuration> {
        match (self.started_at(), self.ended_at()) {
            (Some(start), Some(end)) => Some(end.duration_since(start)),
            _ => None,
        }
    }

    /// Get the number of responses.
    pub fn len(&self) -> usize {
        self.responses.len()
    }

    /// Check if the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.responses.is_empty()
    }

    /// Get all extracted texts.
    pub fn texts(&self) -> Vec<&str> {
        self.responses.iter().map(|r| r.text()).collect()
    }

    /// Consume the batch and return all texts.
    pub fn into_texts(self) -> Vec<String> {
        self.responses.into_iter().map(|r| r.text).collect()
    }

    /// Get the total number of extractions across all responses.
    pub fn total_extractions(&self) -> usize {
        self.responses.iter().map(|r| r.extraction_count()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocr_response_creation() {
        let request_id = Uuid::new_v4();
        let text = "Hello, world!";
        let response = OcrResponse::new(request_id, text);
        assert_eq!(response.request_id, request_id);
        assert_eq!(response.text(), text);
        assert!(response.timing.is_none());
    }

    #[test]
    fn test_ocr_response_builder() {
        let request_id = Uuid::new_v4();
        let response = OcrResponse::builder()
            .with_request_id(request_id)
            .with_text("Hello")
            .build()
            .unwrap();
        assert_eq!(response.request_id, request_id);
        assert_eq!(response.text(), "Hello");
    }

    #[test]
    fn test_text_extraction() {
        let extraction = TextExtraction::new("Hello")
            .with_bounding_box(BoundingBox::new(10.0, 20.0, 100.0, 30.0))
            .with_page(1);
        assert_eq!(extraction.text, "Hello");
        assert!(extraction.bounding_box.is_some());
    }

    #[test]
    fn test_text_extraction_builder() {
        let extraction = TextExtraction::builder()
            .with_text("Hello")
            .with_page(1u32)
            .build()
            .unwrap();
        assert_eq!(extraction.text, "Hello");
        assert_eq!(extraction.page, Some(1));
    }
}
