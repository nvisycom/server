//! Response types for OCR operations.
//!
//! The `Response<Resp>` type is a generic wrapper that allows OCR implementations
//! to define their own response payload types while maintaining a consistent
//! interface for common metadata like response IDs, timestamps, and usage statistics.

use std::collections::HashMap;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{Annotation, AnnotationType, BoundingBox, Document, TextSpan, Timing};

/// Generic response from an OCR operation.
///
/// This wrapper type provides common metadata and statistics while allowing
/// implementations to define their own specific response payload type.
///
/// # Type Parameters
///
/// * `Resp` - The implementation-specific response payload type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response<Resp> {
    /// Unique identifier for this response.
    pub response_id: Uuid,
    /// Request ID this response corresponds to.
    pub request_id: Uuid,
    /// Implementation-specific response payload.
    pub payload: Resp,
    /// Timing information for the operation.
    pub timing: Option<Timing>,
    /// Additional metadata about the processing.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl<Resp> Response<Resp> {
    /// Create a new OCR response with the given payload.
    pub fn new(request_id: Uuid, payload: Resp) -> Self {
        Self {
            response_id: Uuid::now_v7(),
            request_id,
            payload,
            timing: None,
            metadata: HashMap::new(),
        }
    }

    /// Set timing information.
    pub fn with_timing(mut self, started_at: Timestamp, ended_at: Timestamp) -> Self {
        self.timing = Some(Timing::new(started_at, ended_at));
        self
    }

    /// Add metadata to this response.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Get the processing time as a signed duration.
    pub fn processing_time(&self) -> Option<jiff::SignedDuration> {
        self.timing.map(|t| t.duration())
    }
}

/// Batch response containing multiple OCR results.
///
/// # Type Parameters
///
/// * `Resp` - The implementation-specific response payload type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResponse<Resp> {
    /// Unique identifier for this batch response.
    pub batch_id: Uuid,
    /// Individual responses in the batch.
    pub responses: Vec<Response<Resp>>,
}

impl<Resp> BatchResponse<Resp> {
    /// Create a new batch response.
    pub fn new(responses: Vec<Response<Resp>>) -> Self {
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

    /// Get the number of responses.
    pub fn len(&self) -> usize {
        self.responses.len()
    }

    /// Check if the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.responses.is_empty()
    }
}

/// Statistics for a batch OCR operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchStats {
    /// Total number of items processed.
    pub total_processed: usize,
    /// Number of successful extractions.
    pub successful: usize,
    /// Number of failed extractions.
    pub failed: usize,
    /// Total processing time for the batch.
    pub total_processing_time_ms: u64,
    /// Average confidence across all successful extractions.
    pub average_confidence: f32,
}

impl BatchStats {
    /// Calculate success rate as a percentage.
    pub fn success_rate(&self) -> f32 {
        if self.total_processed == 0 {
            0.0
        } else {
            (self.successful as f32 / self.total_processed as f32) * 100.0
        }
    }

    /// Get average processing time per item.
    pub fn average_processing_time(&self) -> f32 {
        if self.total_processed == 0 {
            0.0
        } else {
            self.total_processing_time_ms as f32 / self.total_processed as f32
        }
    }
}

/// Standard OCR result using Annotation types.
///
/// This provides a standardized way to represent OCR results using the unified
/// annotation system, making it easier to integrate with downstream processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    /// The original document that was processed.
    pub document: Document,
    /// Extracted text with positional information.
    pub text_extractions: Vec<TextExtraction>,
    /// Overall confidence score for the extraction.
    pub confidence: f32,
}

/// Text extraction with annotation-based positional information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextExtraction {
    /// The extracted text content.
    pub text: String,
    /// Text span information for the extracted text.
    pub text_span: Option<TextSpan>,
    /// Bounding box information for spatial location.
    pub bounding_box: Option<BoundingBox>,
    /// Confidence score for this specific extraction.
    pub confidence: f32,
    /// Language detected for this text (ISO 639-1 code).
    pub language: Option<String>,
    /// Additional annotations for this text region.
    pub annotations: Vec<Annotation>,
}

impl OcrResult {
    /// Create a new OCR result.
    pub fn new(document: Document, text_extractions: Vec<TextExtraction>, confidence: f32) -> Self {
        Self {
            document,
            text_extractions,
            confidence,
        }
    }

    /// Get all extracted text concatenated together.
    pub fn full_text(&self) -> String {
        self.text_extractions
            .iter()
            .map(|extraction| extraction.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Get extractions with confidence above a threshold.
    pub fn high_confidence_extractions(&self, threshold: f32) -> Vec<&TextExtraction> {
        self.text_extractions
            .iter()
            .filter(|extraction| extraction.confidence >= threshold)
            .collect()
    }

    /// Get the number of text extractions.
    pub fn extraction_count(&self) -> usize {
        self.text_extractions.len()
    }

    /// Check if the OCR result is empty.
    pub fn is_empty(&self) -> bool {
        self.text_extractions.is_empty()
    }

    /// Get the average confidence across all extractions.
    pub fn average_confidence(&self) -> f32 {
        if self.text_extractions.is_empty() {
            0.0
        } else {
            let total: f32 = self.text_extractions.iter().map(|e| e.confidence).sum();
            total / self.text_extractions.len() as f32
        }
    }

    /// Convert to annotations for further processing.
    pub fn to_annotations(&self) -> Vec<Annotation> {
        self.text_extractions
            .iter()
            .map(|extraction| {
                let mut annotation = Annotation::new(AnnotationType::Text, "ocr_text")
                    .with_confidence(extraction.confidence)
                    .with_content(extraction.text.clone())
                    .with_source("ocr");

                if let Some(text_span) = &extraction.text_span {
                    annotation = annotation.with_text_span(*text_span);
                }

                if let Some(bounding_box) = &extraction.bounding_box {
                    annotation = annotation.with_bounding_box(*bounding_box);
                }

                annotation
            })
            .collect()
    }
}

impl TextExtraction {
    /// Create a new text extraction.
    pub fn new(text: String, confidence: f32) -> Self {
        Self {
            text,
            text_span: None,
            bounding_box: None,
            confidence,
            language: None,
            annotations: Vec::new(),
        }
    }

    /// Set the text span for this extraction.
    pub fn with_text_span(mut self, span: TextSpan) -> Self {
        self.text_span = Some(span);
        self
    }

    /// Set the bounding box for this extraction.
    pub fn with_bounding_box(mut self, bbox: BoundingBox) -> Self {
        self.bounding_box = Some(bbox);
        self
    }

    /// Set the language for this extraction.
    pub fn with_language(mut self, language: String) -> Self {
        self.language = Some(language);
        self
    }

    /// Add an annotation to this extraction.
    pub fn with_annotation(mut self, annotation: Annotation) -> Self {
        self.annotations.push(annotation);
        self
    }

    /// Check if this extraction has high confidence.
    pub fn is_high_confidence(&self, threshold: f32) -> bool {
        self.confidence >= threshold
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;
    use crate::types::{Document, TextSpan};

    #[test]
    fn test_ocr_result_creation() {
        let document = Document::new(Bytes::from("test image data")).with_content_type("image/png");

        let extraction = TextExtraction::new("Hello World".to_string(), 0.95);
        let extractions = vec![extraction];

        let result = OcrResult::new(document, extractions, 0.90);

        assert_eq!(result.confidence, 0.90);
        assert_eq!(result.extraction_count(), 1);
        assert!(!result.is_empty());
        assert_eq!(result.full_text(), "Hello World");
    }

    #[test]
    fn test_text_extraction_with_spans() {
        let text_span = TextSpan::new(0, 11);
        let bbox = BoundingBox::new(10.0, 20.0, 100.0, 30.0);

        let extraction = TextExtraction::new("Hello World".to_string(), 0.85)
            .with_text_span(text_span.clone())
            .with_bounding_box(bbox.clone())
            .with_language("en".to_string());

        assert_eq!(extraction.text, "Hello World");
        assert_eq!(extraction.confidence, 0.85);
        assert_eq!(extraction.text_span, Some(text_span));
        assert_eq!(extraction.bounding_box, Some(bbox));
        assert_eq!(extraction.language, Some("en".to_string()));
        assert!(extraction.is_high_confidence(0.8));
        assert!(!extraction.is_high_confidence(0.9));
    }

    #[test]
    fn test_ocr_result_full_text() {
        let document = Document::new(Bytes::from("test")).with_content_type("image/png");

        let extractions = vec![
            TextExtraction::new("Hello".to_string(), 0.9),
            TextExtraction::new("World".to_string(), 0.8),
            TextExtraction::new("Test".to_string(), 0.95),
        ];

        let result = OcrResult::new(document, extractions, 0.88);

        assert_eq!(result.full_text(), "Hello World Test");
        assert_eq!(result.extraction_count(), 3);
    }

    #[test]
    fn test_ocr_result_high_confidence_extractions() {
        let document = Document::new(Bytes::from("test")).with_content_type("image/png");

        let extractions = vec![
            TextExtraction::new("High1".to_string(), 0.95),
            TextExtraction::new("Low".to_string(), 0.5),
            TextExtraction::new("High2".to_string(), 0.9),
        ];

        let result = OcrResult::new(document, extractions, 0.78);

        let high_confidence = result.high_confidence_extractions(0.8);
        assert_eq!(high_confidence.len(), 2);
        assert_eq!(high_confidence[0].text, "High1");
        assert_eq!(high_confidence[1].text, "High2");
    }

    #[test]
    fn test_ocr_result_average_confidence() {
        let document = Document::new(Bytes::from("test")).with_content_type("image/png");

        let extractions = vec![
            TextExtraction::new("Text1".to_string(), 0.8),
            TextExtraction::new("Text2".to_string(), 0.9),
            TextExtraction::new("Text3".to_string(), 1.0),
        ];

        let result = OcrResult::new(document, extractions, 0.9);

        assert!((result.average_confidence() - 0.9).abs() < f32::EPSILON); // (0.8 + 0.9 + 1.0) / 3
    }

    #[test]
    fn test_ocr_result_empty() {
        let document = Document::new(Bytes::from("test")).with_content_type("image/png");
        let result = OcrResult::new(document, vec![], 0.0);

        assert!(result.is_empty());
        assert_eq!(result.extraction_count(), 0);
        assert_eq!(result.full_text(), "");
        assert_eq!(result.average_confidence(), 0.0);
    }

    #[test]
    fn test_ocr_result_to_annotations() {
        let document = Document::new(Bytes::from("test")).with_content_type("image/png");

        let text_span = TextSpan::new(0, 5);
        let bbox = BoundingBox::new(0.0, 0.0, 50.0, 20.0);

        let extraction = TextExtraction::new("Hello".to_string(), 0.95)
            .with_text_span(text_span.clone())
            .with_bounding_box(bbox.clone());

        let result = OcrResult::new(document, vec![extraction], 0.95);
        let annotations = result.to_annotations();

        assert_eq!(annotations.len(), 1);
        let annotation = &annotations[0];

        assert_eq!(annotation.annotation_type, AnnotationType::Text);
        assert_eq!(annotation.label, "ocr_text");
        assert_eq!(annotation.confidence, Some(0.95));
        assert_eq!(annotation.content, Some("Hello".to_string()));
        assert_eq!(annotation.text_span, Some(text_span));
        assert_eq!(annotation.bounding_box, Some(bbox));
        assert_eq!(annotation.source, Some("ocr".to_string()));
        assert_eq!(annotation.model, None);
    }

    #[test]
    fn test_batch_stats() {
        let mut stats = BatchStats {
            total_processed: 10,
            successful: 8,
            failed: 2,
            total_processing_time_ms: 5000,
            average_confidence: 0.85,
        };

        assert_eq!(stats.success_rate(), 80.0);
        assert_eq!(stats.average_processing_time(), 500.0);

        // Test edge case with zero processed
        stats.total_processed = 0;
        assert_eq!(stats.success_rate(), 0.0);
        assert_eq!(stats.average_processing_time(), 0.0);
    }

    #[test]
    fn test_text_extraction_with_annotation() {
        let annotation = Annotation::new(AnnotationType::Entity, "PERSON").with_confidence(0.9);

        let extraction =
            TextExtraction::new("John Doe".to_string(), 0.95).with_annotation(annotation.clone());

        assert_eq!(extraction.annotations.len(), 1);
        assert_eq!(extraction.annotations[0].label, "PERSON");
    }

    #[test]
    fn test_response_builder_pattern() {
        let document = Document::new(Bytes::from("test document")).with_content_type("image/pdf");

        let extraction1 =
            TextExtraction::new("First line".to_string(), 0.9).with_language("en".to_string());
        let extraction2 =
            TextExtraction::new("Second line".to_string(), 0.85).with_language("en".to_string());

        let result = OcrResult::new(document, vec![extraction1, extraction2], 0.875);
        assert_eq!(result.extraction_count(), 2);
        assert_eq!(result.confidence, 0.875);
        assert_eq!(result.full_text(), "First line Second line");
    }
}
