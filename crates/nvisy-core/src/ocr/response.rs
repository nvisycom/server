//! Response types for OCR operations.
//!
//! This module provides `Response` for single OCR results
//! and `BatchResponse` for batch operation results.

use std::collections::HashMap;

use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::Timing;

/// Response from a single OCR operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
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

impl Response {
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

/// Batch response containing multiple OCR results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResponse {
    /// Unique identifier for this batch response.
    pub batch_id: Uuid,
    /// Individual responses in the batch.
    pub responses: Vec<Response>,
}

impl BatchResponse {
    /// Create a new batch response.
    pub fn new(responses: Vec<Response>) -> Self {
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
    fn test_response_creation() {
        let request_id = Uuid::new_v4();
        let text = "Hello, world!";

        let response = Response::new(request_id, text);

        assert_eq!(response.request_id, request_id);
        assert_eq!(response.text(), text);
        assert!(response.timing.is_none());
        assert!(response.confidence.is_none());
    }

    #[test]
    fn test_response_with_confidence() {
        let response = Response::new(Uuid::new_v4(), "test").with_confidence(0.95);

        assert_eq!(response.confidence, Some(0.95));
    }

    #[test]
    fn test_response_with_timing() {
        let started_at = Timestamp::now();
        let ended_at = started_at + SignedDuration::from_millis(150);

        let response = Response::new(Uuid::new_v4(), "test").with_timing(started_at, ended_at);

        assert!(response.timing.is_some());
        assert!(response.processing_time().is_some());
    }

    #[test]
    fn test_response_with_extractions() {
        let extractions = vec![
            TextExtraction::new("First").with_confidence(0.9),
            TextExtraction::new("Second").with_confidence(0.8),
        ];

        let response = Response::with_extractions(Uuid::new_v4(), "First Second", extractions);

        assert_eq!(response.extraction_count(), 2);
        assert!(!response.is_empty());
    }

    #[test]
    fn test_text_extraction_creation() {
        let extraction = TextExtraction::new("Hello")
            .with_confidence(0.95)
            .with_bounding_box(10.0, 20.0, 100.0, 30.0)
            .with_page(1)
            .with_language("en");

        assert_eq!(extraction.text, "Hello");
        assert_eq!(extraction.confidence, Some(0.95));
        assert_eq!(extraction.bounding_box, Some((10.0, 20.0, 100.0, 30.0)));
        assert_eq!(extraction.page, Some(1));
        assert_eq!(extraction.language, Some("en".to_string()));
        assert!(extraction.is_high_confidence(0.9));
    }

    #[test]
    fn test_high_confidence_extractions() {
        let extractions = vec![
            TextExtraction::new("High1").with_confidence(0.95),
            TextExtraction::new("Low").with_confidence(0.5),
            TextExtraction::new("High2").with_confidence(0.9),
        ];

        let response = Response::with_extractions(Uuid::new_v4(), "text", extractions);
        let high = response.high_confidence_extractions(0.8);

        assert_eq!(high.len(), 2);
        assert_eq!(high[0].text, "High1");
        assert_eq!(high[1].text, "High2");
    }

    #[test]
    fn test_average_confidence() {
        let extractions = vec![
            TextExtraction::new("A").with_confidence(0.8),
            TextExtraction::new("B").with_confidence(0.9),
            TextExtraction::new("C").with_confidence(1.0),
        ];

        let response = Response::with_extractions(Uuid::new_v4(), "text", extractions);

        assert!((response.average_confidence().unwrap() - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn test_batch_response() {
        let responses = vec![
            Response::new(Uuid::new_v4(), "First"),
            Response::new(Uuid::new_v4(), "Second"),
        ];

        let batch = BatchResponse::new(responses);

        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
        assert_eq!(batch.texts(), vec!["First", "Second"]);
    }

    #[test]
    fn test_batch_response_into_texts() {
        let responses = vec![
            Response::new(Uuid::new_v4(), "First"),
            Response::new(Uuid::new_v4(), "Second"),
        ];

        let batch = BatchResponse::new(responses);
        let texts = batch.into_texts();

        assert_eq!(texts, vec!["First", "Second"]);
    }

    #[test]
    fn test_batch_response_timing() {
        let base_time = Timestamp::now();

        let responses = vec![
            Response::new(Uuid::new_v4(), "First").with_timing(
                base_time + SignedDuration::from_millis(100),
                base_time + SignedDuration::from_millis(200),
            ),
            Response::new(Uuid::new_v4(), "Second").with_timing(
                base_time + SignedDuration::from_millis(50),
                base_time + SignedDuration::from_millis(300),
            ),
        ];

        let batch = BatchResponse::new(responses);

        assert_eq!(
            batch.started_at(),
            Some(base_time + SignedDuration::from_millis(50))
        );
        assert_eq!(
            batch.ended_at(),
            Some(base_time + SignedDuration::from_millis(300))
        );
        assert_eq!(batch.processing_time().map(|d| d.as_millis()), Some(250));
    }
}
