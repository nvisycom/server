//! OCR response types.

use std::collections::HashMap;

use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Timing;

/// Individual text extraction with positional information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextExtraction {
    /// The extracted text content.
    pub text: String,
    /// Confidence score for this extraction (0.0 to 1.0).
    pub confidence: Option<f32>,
    /// Bounding box for the text region (x, y, width, height).
    pub bounding_box: Option<(f32, f32, f32, f32)>,
    /// Page number (for multi-page documents).
    pub page: Option<u32>,
    /// Language detected for this text (ISO 639-1 code).
    pub language: Option<String>,
}

impl TextExtraction {
    /// Create a new text extraction.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            confidence: None,
            bounding_box: None,
            page: None,
            language: None,
        }
    }

    /// Set the confidence score.
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }

    /// Set the bounding box.
    pub fn with_bounding_box(mut self, x: f32, y: f32, width: f32, height: f32) -> Self {
        self.bounding_box = Some((x, y, width, height));
        self
    }

    /// Set the page number.
    pub fn with_page(mut self, page: u32) -> Self {
        self.page = Some(page);
        self
    }

    /// Set the language.
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Check if this extraction has high confidence.
    pub fn is_high_confidence(&self, threshold: f32) -> bool {
        self.confidence.unwrap_or(0.0) >= threshold
    }
}

/// Response from a single OCR operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResponse {
    /// Unique identifier for this response.
    pub response_id: Uuid,
    /// Request ID this response corresponds to.
    pub request_id: Uuid,
    /// Extracted text from the document.
    pub text: String,
    /// Individual text extractions with positional information.
    pub extractions: Vec<TextExtraction>,
    /// Overall confidence score for the extraction (0.0 to 1.0).
    pub confidence: Option<f32>,
    /// Detected language of the text (ISO 639-1 code).
    pub language: Option<String>,
    /// Timing information for the operation.
    pub timing: Option<Timing>,
    /// Additional metadata about the processing.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl OcrResponse {
    /// Create a new OCR response with the given text.
    pub fn new(request_id: Uuid, text: impl Into<String>) -> Self {
        Self {
            response_id: Uuid::now_v7(),
            request_id,
            text: text.into(),
            extractions: Vec::new(),
            confidence: None,
            language: None,
            timing: None,
            metadata: HashMap::new(),
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
            confidence: None,
            language: None,
            timing: None,
            metadata: HashMap::new(),
        }
    }

    /// Set timing information.
    pub fn with_timing(mut self, started_at: Timestamp, ended_at: Timestamp) -> Self {
        self.timing = Some(Timing::new(started_at, ended_at));
        self
    }

    /// Set the confidence score.
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }

    /// Set the detected language.
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Add metadata to this response.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
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

    /// Get extractions with confidence above a threshold.
    pub fn high_confidence_extractions(&self, threshold: f32) -> Vec<&TextExtraction> {
        self.extractions
            .iter()
            .filter(|e| e.confidence.unwrap_or(0.0) >= threshold)
            .collect()
    }

    /// Get the average confidence across all extractions.
    pub fn average_confidence(&self) -> Option<f32> {
        let confidences: Vec<f32> = self
            .extractions
            .iter()
            .filter_map(|e| e.confidence)
            .collect();

        if confidences.is_empty() {
            self.confidence
        } else {
            Some(confidences.iter().sum::<f32>() / confidences.len() as f32)
        }
    }
}

/// Batch response containing multiple OCR results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrBatchResponse {
    /// Unique identifier for this batch response.
    pub batch_id: Uuid,
    /// Individual responses in the batch.
    pub responses: Vec<OcrResponse>,
}

impl OcrBatchResponse {
    /// Create a new batch response.
    pub fn new(responses: Vec<OcrResponse>) -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            responses,
        }
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
        assert!(response.confidence.is_none());
    }

    #[test]
    fn test_text_extraction() {
        let extraction = TextExtraction::new("Hello")
            .with_confidence(0.95)
            .with_bounding_box(10.0, 20.0, 100.0, 30.0)
            .with_page(1)
            .with_language("en");
        assert_eq!(extraction.text, "Hello");
        assert_eq!(extraction.confidence, Some(0.95));
        assert!(extraction.is_high_confidence(0.9));
    }
}
