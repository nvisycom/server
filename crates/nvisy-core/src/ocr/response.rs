//! Response types for OCR operations.
//!
//! The `Response<Resp>` type is a generic wrapper that allows OCR implementations
//! to define their own response payload types while maintaining a consistent
//! interface for common metadata like response IDs, timestamps, and usage statistics.

use std::collections::HashMap;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::context::UsageStats;
use crate::types::{Annotation, AnnotationType, BoundingBox, Document, TextSpan};

/// Generic response from an OCR operation.
///
/// This wrapper type provides common metadata and statistics while allowing
/// implementations to define their own specific response payload type.
///
/// # Type Parameters
///
/// * `Resp` - The implementation-specific response payload type
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Debug, Clone)]
/// struct MyOcrResponse {
///     text: String,
///     confidence: f32,
/// }
///
/// let response = Response::new(
///     request_id,
///     MyOcrResponse {
///         text: "extracted text".to_string(),
///         confidence: 0.95,
///     }
/// );
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response<Resp> {
    /// Unique identifier for this response.
    pub response_id: Uuid,
    /// Request ID this response corresponds to.
    pub request_id: Uuid,
    /// Implementation-specific response payload.
    pub payload: Resp,
    /// Processing time in milliseconds.
    pub processing_time_ms: Option<u64>,
    /// When this response was generated.
    pub timestamp: Timestamp,
    /// Usage statistics for this operation.
    pub usage: UsageStats,
    /// Additional metadata about the processing.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl<Resp> Response<Resp> {
    /// Create a new OCR response with the given payload.
    pub fn new(request_id: Uuid, payload: Resp) -> Self {
        Self {
            response_id: Uuid::new_v4(),
            request_id,
            payload,
            processing_time_ms: None,
            timestamp: Timestamp::now(),
            usage: UsageStats::default(),
            metadata: HashMap::new(),
        }
    }

    /// Set the processing time.
    pub fn with_processing_time(mut self, ms: u64) -> Self {
        self.processing_time_ms = Some(ms);
        self
    }

    /// Set usage statistics.
    pub fn with_usage(mut self, usage: UsageStats) -> Self {
        self.usage = usage;
        self
    }

    /// Add metadata to this response.
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
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
    /// Overall processing statistics.
    pub batch_stats: BatchStats,
    /// When the batch was processed.
    pub timestamp: Timestamp,
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
    /// Processing metadata and statistics.
    pub processing_info: ProcessingInfo,
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

/// Processing information and metadata for OCR operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingInfo {
    /// Model or engine used for OCR processing.
    pub model: Option<String>,
    /// Processing time in milliseconds.
    pub processing_time_ms: u64,
    /// DPI used for processing.
    pub dpi: Option<u32>,
    /// Languages detected in the document.
    pub detected_languages: Vec<String>,
    /// Number of pages processed (for multi-page documents).
    pub pages_processed: u32,
    /// Additional metadata about the processing.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl OcrResult {
    /// Create a new OCR result.
    pub fn new(document: Document, text_extractions: Vec<TextExtraction>, confidence: f32) -> Self {
        Self {
            document,
            text_extractions,
            confidence,
            processing_info: ProcessingInfo::default(),
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
            .enumerate()
            .map(|(_index, extraction)| {
                let mut annotation = Annotation::new(AnnotationType::Text, "ocr_text")
                    .with_confidence(extraction.confidence)
                    .with_content(extraction.text.clone())
                    .with_source("ocr");

                if let Some(text_span) = &extraction.text_span {
                    annotation = annotation.with_text_span(text_span.clone());
                }

                if let Some(bounding_box) = &extraction.bounding_box {
                    annotation = annotation.with_bounding_box(bounding_box.clone());
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

impl Default for ProcessingInfo {
    fn default() -> Self {
        Self {
            model: None,
            processing_time_ms: 0,
            dpi: Some(300),
            detected_languages: Vec::new(),
            pages_processed: 1,
            metadata: HashMap::new(),
        }
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

        let mut result = OcrResult::new(document, vec![extraction], 0.95);
        result.processing_info.model = Some("test-ocr-model".to_string());

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
        assert_eq!(annotation.model, Some("test-ocr-model".to_string()));
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
    fn test_processing_info_default() {
        let info = ProcessingInfo::default();

        assert_eq!(info.model, None);
        assert_eq!(info.processing_time_ms, 0);
        assert_eq!(info.dpi, Some(300));
        assert!(info.detected_languages.is_empty());
        assert_eq!(info.pages_processed, 1);
        assert!(info.metadata.is_empty());
    }

    #[test]
    fn test_text_extraction_with_annotation() {
        let annotation = Annotation::builder()
            .annotation_type(AnnotationType::Entity)
            .label("PERSON")
            .confidence(0.9)
            .content("John Doe")
            .source("ner")
            .build()
            .unwrap();

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

        let mut processing_info = ProcessingInfo::default();
        processing_info.model = Some("test-model".to_string());
        processing_info.processing_time_ms = 1500;
        processing_info.detected_languages = vec!["en".to_string()];
        processing_info.pages_processed = 2;

        let mut result = OcrResult::new(document, vec![extraction1, extraction2], 0.875);
        result.processing_info = processing_info;

        assert_eq!(result.extraction_count(), 2);
        assert_eq!(result.confidence, 0.875);
        assert_eq!(result.processing_info.model, Some("test-model".to_string()));
        assert_eq!(result.processing_info.processing_time_ms, 1500);
        assert_eq!(result.processing_info.detected_languages, vec!["en"]);
        assert_eq!(result.processing_info.pages_processed, 2);
        assert_eq!(result.full_text(), "First line Second line");
    }
}
