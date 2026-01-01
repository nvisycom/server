//! Response types for all inference operations.
//!
//! This module provides response types for embedding, OCR, and VLM operations,
//! supporting both single and batch results.

use std::collections::HashMap;
use std::num::NonZeroU32;

use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::Timing;

// ============================================================================
// Embedding Response Types
// ============================================================================

/// Format for returned embeddings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingFormat {
    /// Return embeddings as floating point numbers.
    #[default]
    Float,
    /// Return embeddings as base64-encoded strings.
    Base64,
}

/// Response from a single embedding operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    /// Unique identifier for this response.
    pub response_id: Uuid,
    /// Request ID this response corresponds to.
    pub request_id: Uuid,
    /// Generated embedding vector.
    pub embedding: Vec<f32>,
    /// Tokens processed for this request.
    pub tokens: Option<NonZeroU32>,
    /// Timing information for the operation.
    pub timing: Option<Timing>,
    /// Additional metadata about the processing.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl EmbeddingResponse {
    /// Create a new embedding response.
    pub fn new(request_id: Uuid, embedding: Vec<f32>) -> Self {
        Self {
            response_id: Uuid::now_v7(),
            request_id,
            embedding,
            tokens: None,
            timing: None,
            metadata: HashMap::new(),
        }
    }

    /// Set timing information.
    pub fn with_timing(mut self, started_at: Timestamp, ended_at: Timestamp) -> Self {
        self.timing = Some(Timing::new(started_at, ended_at));
        self
    }

    /// Set the token count.
    pub fn with_tokens(mut self, count: NonZeroU32) -> Self {
        self.tokens = Some(count);
        self
    }

    /// Set the token count from a u32 value (ignores zero values).
    pub fn with_tokens_u32(mut self, count: u32) -> Self {
        self.tokens = NonZeroU32::new(count);
        self
    }

    /// Add metadata to this response.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Get the processing time as a signed duration.
    pub fn processing_time(&self) -> Option<SignedDuration> {
        self.timing.map(|t| t.duration())
    }

    /// Get the dimensionality of the embedding.
    pub fn dimensions(&self) -> usize {
        self.embedding.len()
    }
}

/// Batch response containing multiple embedding results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingBatchResponse {
    /// Unique identifier for this batch response.
    pub batch_id: Uuid,
    /// Individual responses in the batch.
    pub responses: Vec<EmbeddingResponse>,
}

impl EmbeddingBatchResponse {
    /// Create a new batch response.
    pub fn new(responses: Vec<EmbeddingResponse>) -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            responses,
        }
    }

    /// Create a batch response from embeddings.
    pub fn from_embeddings(embeddings: Vec<Vec<f32>>) -> Self {
        let responses = embeddings
            .into_iter()
            .map(|embedding| EmbeddingResponse::new(Uuid::now_v7(), embedding))
            .collect();
        Self::new(responses)
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

    /// Get all embeddings as a vector.
    pub fn embeddings(&self) -> Vec<&Vec<f32>> {
        self.responses.iter().map(|r| &r.embedding).collect()
    }

    /// Consume the batch and return all embeddings.
    pub fn into_embeddings(self) -> Vec<Vec<f32>> {
        self.responses.into_iter().map(|r| r.embedding).collect()
    }

    /// Get the total tokens processed across all responses.
    pub fn total_tokens(&self) -> u32 {
        self.responses
            .iter()
            .filter_map(|r| r.tokens)
            .map(|t| t.get())
            .sum()
    }
}

// ============================================================================
// OCR Response Types
// ============================================================================

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

// ============================================================================
// VLM Response Types
// ============================================================================

/// Usage statistics for VLM operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VlmUsage {
    /// Number of tokens in the prompt.
    pub prompt_tokens: u32,
    /// Number of tokens in the completion.
    pub completion_tokens: u32,
    /// Total number of tokens used.
    pub total_tokens: u32,
}

impl VlmUsage {
    /// Create a new usage record.
    pub fn new(prompt_tokens: u32, completion_tokens: u32) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        }
    }
}

/// Response from a single VLM operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlmResponse {
    /// Unique identifier for this response.
    pub response_id: Uuid,
    /// Request ID this response corresponds to.
    pub request_id: Uuid,
    /// The generated content.
    pub content: String,
    /// Token usage information.
    pub usage: Option<VlmUsage>,
    /// Reason why generation finished.
    pub finish_reason: Option<String>,
    /// Confidence score for the response (0.0 to 1.0).
    pub confidence: Option<f64>,
    /// Timing information for the operation.
    pub timing: Option<Timing>,
    /// Additional metadata about the processing.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl VlmResponse {
    /// Create a new VLM response with the given content.
    pub fn new(request_id: Uuid, content: impl Into<String>) -> Self {
        Self {
            response_id: Uuid::now_v7(),
            request_id,
            content: content.into(),
            usage: None,
            finish_reason: None,
            confidence: None,
            timing: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a new VLM response with content and usage.
    pub fn with_usage(request_id: Uuid, content: impl Into<String>, usage: VlmUsage) -> Self {
        Self {
            response_id: Uuid::now_v7(),
            request_id,
            content: content.into(),
            usage: Some(usage),
            finish_reason: None,
            confidence: None,
            timing: None,
            metadata: HashMap::new(),
        }
    }

    /// Set timing information.
    pub fn with_timing(mut self, started_at: Timestamp, ended_at: Timestamp) -> Self {
        self.timing = Some(Timing::new(started_at, ended_at));
        self
    }

    /// Set the finish reason.
    pub fn with_finish_reason(mut self, reason: impl Into<String>) -> Self {
        self.finish_reason = Some(reason.into());
        self
    }

    /// Set the confidence score.
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }

    /// Add metadata to this response.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Get the generated content.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Get the processing time as a signed duration.
    pub fn processing_time(&self) -> Option<SignedDuration> {
        self.timing.map(|t| t.duration())
    }

    /// Get the content length in characters.
    pub fn content_length(&self) -> usize {
        self.content.chars().count()
    }

    /// Check if the response is empty.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Check if the response generation completed normally.
    pub fn is_complete(&self) -> bool {
        matches!(
            self.finish_reason.as_deref(),
            Some("complete") | Some("stop") | Some("end_turn") | None
        )
    }

    /// Check if the response was truncated due to length limits.
    pub fn is_truncated(&self) -> bool {
        matches!(
            self.finish_reason.as_deref(),
            Some("length") | Some("max_tokens")
        )
    }

    /// Check if the response was stopped due to content filtering.
    pub fn is_filtered(&self) -> bool {
        matches!(
            self.finish_reason.as_deref(),
            Some("content_filter") | Some("safety")
        )
    }

    /// Get total tokens used.
    pub fn total_tokens(&self) -> Option<u32> {
        self.usage.as_ref().map(|u| u.total_tokens)
    }
}

/// Batch response containing multiple VLM results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlmBatchResponse {
    /// Unique identifier for this batch response.
    pub batch_id: Uuid,
    /// Individual responses in the batch.
    pub responses: Vec<VlmResponse>,
}

impl VlmBatchResponse {
    /// Create a new batch response.
    pub fn new(responses: Vec<VlmResponse>) -> Self {
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

    /// Get all contents as a vector.
    pub fn contents(&self) -> Vec<&str> {
        self.responses.iter().map(|r| r.content()).collect()
    }

    /// Consume the batch and return all contents.
    pub fn into_contents(self) -> Vec<String> {
        self.responses.into_iter().map(|r| r.content).collect()
    }

    /// Get the total tokens used across all responses.
    pub fn total_tokens(&self) -> u32 {
        self.responses
            .iter()
            .filter_map(|r| r.usage.as_ref())
            .map(|u| u.total_tokens)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Embedding tests
    #[test]
    fn test_embedding_response_creation() {
        let request_id = Uuid::new_v4();
        let embedding = vec![0.1, 0.2, 0.3];
        let response = EmbeddingResponse::new(request_id, embedding);
        assert_eq!(response.request_id, request_id);
        assert_eq!(response.dimensions(), 3);
        assert!(response.timing.is_none());
    }

    #[test]
    fn test_embedding_batch_response() {
        let embeddings = vec![vec![0.1, 0.2], vec![0.3, 0.4], vec![0.5, 0.6]];
        let batch = EmbeddingBatchResponse::from_embeddings(embeddings);
        assert_eq!(batch.len(), 3);
        assert_eq!(batch.embeddings().len(), 3);
    }

    // OCR tests
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

    // VLM tests
    #[test]
    fn test_vlm_response_creation() {
        let request_id = Uuid::new_v4();
        let content = "This is the response";
        let response = VlmResponse::new(request_id, content);
        assert_eq!(response.request_id, request_id);
        assert_eq!(response.content(), content);
        assert!(response.usage.is_none());
        assert!(response.timing.is_none());
    }

    #[test]
    fn test_vlm_usage() {
        let usage = VlmUsage::new(25, 75);
        assert_eq!(usage.prompt_tokens, 25);
        assert_eq!(usage.completion_tokens, 75);
        assert_eq!(usage.total_tokens, 100);
    }

    #[test]
    fn test_vlm_response_status_methods() {
        let complete = VlmResponse::new(Uuid::new_v4(), "test").with_finish_reason("stop");
        assert!(complete.is_complete());
        assert!(!complete.is_truncated());
        assert!(!complete.is_filtered());

        let truncated = VlmResponse::new(Uuid::new_v4(), "test").with_finish_reason("length");
        assert!(!truncated.is_complete());
        assert!(truncated.is_truncated());

        let filtered =
            VlmResponse::new(Uuid::new_v4(), "test").with_finish_reason("content_filter");
        assert!(filtered.is_filtered());
    }
}
