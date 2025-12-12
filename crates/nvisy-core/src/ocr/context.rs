//! Context management for OCR operations.

use std::collections::HashMap;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Context information for OCR operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    /// Unique identifier for this context session.
    pub session_id: Uuid,
    /// User identifier associated with this context.
    pub user_id: String,
    /// Document identifier for tracking related extractions.
    pub document_id: String,
    /// Information about the document being processed.
    pub document: Option<Document>,
    /// Processing options and configuration.
    pub processing_options: ProcessingOptions,
    /// Extracted content from previous operations.
    pub extracted_content: Vec<ExtractedContent>,
    /// Language detection results (automatic).
    pub language_detection: Option<LanguageDetectionResult>,
    /// Processing quality metrics.
    pub quality_metrics: QualityMetrics,
    /// Usage statistics for this context.
    pub usage: UsageStats,
    /// Metadata about the context and processing.
    pub metadata: ContextMetadata,
}

impl Context {
    /// Create a new OCR context.
    pub fn new<U: Into<String>, D: Into<String>>(user_id: U, document_id: D) -> Self {
        Self {
            session_id: Uuid::new_v4(),
            user_id: user_id.into(),
            document_id: document_id.into(),
            document: None,
            processing_options: ProcessingOptions::default(),
            extracted_content: Vec::new(),
            language_detection: None,
            quality_metrics: QualityMetrics::default(),
            usage: UsageStats::default(),
            metadata: ContextMetadata::default(),
        }
    }

    /// Set document information.
    pub fn set_document(&mut self, document: Document) {
        self.document = Some(document);
    }

    /// Add extracted content to the context.
    pub fn add_extracted_content(&mut self, mut content: ExtractedContent) {
        content.extraction_id = Uuid::new_v4();
        content.processed_at = Timestamp::now();

        // Update usage statistics
        self.usage.total_characters += content.character_count();
        self.usage.total_words += content.word_count();
        self.usage.total_regions += content.regions.len() as u32;
        self.usage.successful_extractions += 1;

        // Update quality metrics
        self.quality_metrics.update_confidence(content.confidence);

        self.extracted_content.push(content);
        self.metadata.last_updated = Timestamp::now();
    }

    /// Set language detection result.
    pub fn set_language_detection(&mut self, result: LanguageDetectionResult) {
        self.language_detection = Some(result);
    }

    /// Get combined text from all extracted content.
    pub fn get_combined_text(&self) -> String {
        self.extracted_content
            .iter()
            .map(|content| content.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Get content for a specific page.
    pub fn get_page_content(&self, page_number: u32) -> Vec<&ExtractedContent> {
        self.extracted_content
            .iter()
            .filter(|content| content.page_number == Some(page_number))
            .collect()
    }

    /// Get number of pages processed.
    pub fn pages_processed(&self) -> u32 {
        self.usage.pages_processed
    }

    /// Get number of regions extracted.
    pub fn regions_extracted(&self) -> u32 {
        self.usage.total_regions
    }

    /// Get average confidence score.
    pub fn average_confidence(&self) -> Option<f32> {
        self.quality_metrics.average_confidence()
    }

    /// Check if context has any extracted content.
    pub fn has_content(&self) -> bool {
        !self.extracted_content.is_empty()
    }

    /// Clear all extracted content.
    pub fn clear_content(&mut self) {
        self.extracted_content.clear();
        self.usage = UsageStats::default();
        self.quality_metrics = QualityMetrics::default();
    }
}

/// Document information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Original filename, if available.
    pub filename: Option<String>,
    /// MIME type of the document.
    pub mime_type: String,
    /// Number of pages in the document.
    pub page_count: Option<u32>,
    /// Document size in bytes.
    pub size_bytes: u64,
    /// Document creation timestamp.
    pub created_at: Option<Timestamp>,
}

/// Processing options for OCR operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingOptions {
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

impl Default for ProcessingOptions {
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

/// Extracted content from OCR operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedContent {
    /// Unique identifier for this extraction.
    pub extraction_id: Uuid,
    /// Extracted text content.
    pub text: String,
    /// Page number (if applicable).
    pub page_number: Option<u32>,
    /// Text regions with positioning information.
    pub regions: Vec<TextRegion>,
    /// Overall confidence score for this extraction.
    pub confidence: f32,
    /// Detected language (automatic detection).
    pub detected_language: Option<String>,
    /// Timestamp when this content was processed.
    pub processed_at: Timestamp,
    /// Processing time in milliseconds.
    pub processing_time_ms: u32,
    /// Additional metadata for this extraction.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ExtractedContent {
    /// Create new extracted content.
    pub fn new(text: String) -> Self {
        Self {
            extraction_id: Uuid::new_v4(),
            text,
            page_number: None,
            regions: Vec::new(),
            confidence: 1.0,
            detected_language: None,
            processed_at: Timestamp::now(),
            processing_time_ms: 0,
            metadata: HashMap::new(),
        }
    }

    /// Set confidence score.
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence;
        self
    }

    /// Set detected language.
    pub fn with_language(mut self, language: String) -> Self {
        self.detected_language = Some(language);
        self
    }

    /// Set processing time.
    pub fn with_processing_time(mut self, ms: u32) -> Self {
        self.processing_time_ms = ms;
        self
    }

    /// Add metadata entry.
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Get word count.
    pub fn word_count(&self) -> u32 {
        self.text.split_whitespace().count() as u32
    }

    /// Get character count.
    pub fn character_count(&self) -> u32 {
        self.text.chars().count() as u32
    }

    /// Check if content is empty.
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}

/// Text region with positioning information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRegion {
    /// Text content of this region.
    pub text: String,
    /// Bounding box coordinates.
    pub bounding_box: BoundingBox,
    /// Confidence score for this region.
    pub confidence: f32,
    /// Type of text region.
    pub region_type: TextRegionType,
    /// Individual words in this region.
    pub words: Vec<Word>,
}

/// Bounding box coordinates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BoundingBox {
    /// Left coordinate.
    pub left: f32,
    /// Top coordinate.
    pub top: f32,
    /// Width of the box.
    pub width: f32,
    /// Height of the box.
    pub height: f32,
}

impl BoundingBox {
    /// Create a new bounding box.
    pub fn new(left: f32, top: f32, width: f32, height: f32) -> Self {
        Self {
            left,
            top,
            width,
            height,
        }
    }

    /// Get right coordinate.
    pub fn right(&self) -> f32 {
        self.left + self.width
    }

    /// Get bottom coordinate.
    pub fn bottom(&self) -> f32 {
        self.top + self.height
    }

    /// Calculate area.
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    /// Check if point is contained within this box.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.left && x <= self.right() && y >= self.top && y <= self.bottom()
    }
}

/// Type of text region.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TextRegionType {
    /// Regular paragraph text.
    Paragraph,
    /// Heading text.
    Heading,
    /// List item.
    ListItem,
    /// Table cell.
    TableCell,
    /// Caption text.
    Caption,
    /// Footer text.
    Footer,
    /// Header text.
    Header,
    /// Other/unknown type.
    Other,
}

/// Individual word with positioning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Word {
    /// Word text.
    pub text: String,
    /// Word bounding box.
    pub bounding_box: BoundingBox,
    /// Word confidence score.
    pub confidence: f32,
}

/// Language detection result (automatic detection).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageDetectionResult {
    /// Primary detected language code.
    pub primary_language: String,
    /// Alternative language candidates.
    pub candidates: Vec<LanguageCandidate>,
    /// Overall detection confidence.
    pub detection_confidence: f32,
    /// Whether multiple languages were detected.
    pub multilingual: bool,
}

/// Language candidate with confidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageCandidate {
    /// Language code (ISO 639-1).
    pub language_code: String,
    /// Human-readable language name.
    pub language_name: String,
    /// Detection confidence for this language.
    pub confidence: f32,
    /// Percentage of text in this language.
    pub text_percentage: f32,
}

/// Quality metrics for OCR processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    /// Confidence scores from all extractions.
    confidence_scores: Vec<f32>,
    /// Number of regions with low confidence.
    pub low_confidence_regions: u32,
    /// Number of failed extractions.
    pub failed_extractions: u32,
    /// Overall quality score.
    pub quality_score: f32,
    /// Image quality assessment.
    pub image_quality: Option<ImageQuality>,
}

impl Default for QualityMetrics {
    fn default() -> Self {
        Self {
            confidence_scores: Vec::new(),
            low_confidence_regions: 0,
            failed_extractions: 0,
            quality_score: 1.0,
            image_quality: None,
        }
    }
}

impl QualityMetrics {
    /// Update confidence metrics with a new score.
    pub fn update_confidence(&mut self, confidence: f32) {
        self.confidence_scores.push(confidence);
        if confidence < 0.7 {
            self.low_confidence_regions += 1;
        }
    }

    /// Get average confidence score.
    pub fn average_confidence(&self) -> Option<f32> {
        if self.confidence_scores.is_empty() {
            None
        } else {
            Some(self.confidence_scores.iter().sum::<f32>() / self.confidence_scores.len() as f32)
        }
    }

    /// Get minimum confidence score.
    pub fn min_confidence(&self) -> Option<f32> {
        self.confidence_scores
            .iter()
            .copied()
            .fold(None, |acc, x| Some(acc.map_or(x, |y| x.min(y))))
    }

    /// Get maximum confidence score.
    pub fn max_confidence(&self) -> Option<f32> {
        self.confidence_scores
            .iter()
            .copied()
            .fold(None, |acc, x| Some(acc.map_or(x, |y| x.max(y))))
    }
}

/// Image quality assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageQuality {
    /// Overall quality score (0.0-1.0).
    pub overall_score: f32,
    /// Sharpness score.
    pub sharpness: f32,
    /// Contrast score.
    pub contrast: f32,
    /// Brightness score.
    pub brightness: f32,
    /// Detected quality issues.
    pub issues: Vec<String>,
    /// Recommendations for improvement.
    pub recommendations: Vec<String>,
}

/// Usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    /// Total characters extracted.
    pub total_characters: u32,
    /// Total words extracted.
    pub total_words: u32,
    /// Total regions processed.
    pub total_regions: u32,
    /// Number of pages processed.
    pub pages_processed: u32,
    /// Total processing time in milliseconds.
    pub total_processing_time_ms: u32,
    /// Number of successful extractions.
    pub successful_extractions: u32,
    /// Number of failed extractions.
    pub failed_extractions: u32,
    /// Estimated cost for processing.
    pub estimated_cost: Option<f64>,
}

impl Default for UsageStats {
    fn default() -> Self {
        Self {
            total_characters: 0,
            total_words: 0,
            total_regions: 0,
            pages_processed: 0,
            total_processing_time_ms: 0,
            successful_extractions: 0,
            failed_extractions: 0,
            estimated_cost: None,
        }
    }
}

impl UsageStats {
    /// Get total number of extractions (successful + failed).
    pub fn total_extractions(&self) -> u32 {
        self.successful_extractions + self.failed_extractions
    }

    /// Calculate success rate as a percentage.
    pub fn success_rate(&self) -> f32 {
        let total = self.total_extractions();
        if total == 0 {
            0.0
        } else {
            (self.successful_extractions as f32 / total as f32) * 100.0
        }
    }

    /// Calculate average processing time per page.
    pub fn average_processing_time_per_page(&self) -> Option<f32> {
        if self.pages_processed == 0 {
            None
        } else {
            Some(self.total_processing_time_ms as f32 / self.pages_processed as f32)
        }
    }

    /// Check if there's any usage data.
    pub fn has_usage(&self) -> bool {
        self.total_extractions() > 0
    }
}

/// Context metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMetadata {
    /// Context creation timestamp.
    pub created_at: Timestamp,
    /// Last update timestamp.
    pub last_updated: Timestamp,
    /// OCR engine used.
    pub ocr_engine: Option<String>,
    /// Engine version.
    pub engine_version: Option<String>,
    /// Processing mode used.
    pub processing_mode: Option<String>,
    /// Custom tags for categorization.
    pub tags: Vec<String>,
    /// Custom metadata fields.
    pub custom: HashMap<String, serde_json::Value>,
}

impl Default for ContextMetadata {
    fn default() -> Self {
        let now = Timestamp::now();
        Self {
            created_at: now,
            last_updated: now,
            ocr_engine: None,
            engine_version: None,
            processing_mode: None,
            tags: Vec::new(),
            custom: HashMap::new(),
        }
    }
}
