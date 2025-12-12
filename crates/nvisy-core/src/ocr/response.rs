//! Response types for OCR operations.

use std::collections::HashMap;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::context::{BoundingBox, TextRegion, UsageStats};

/// Response from an OCR operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Unique identifier for this response.
    pub response_id: Uuid,
    /// Request ID this response corresponds to.
    pub request_id: Uuid,
    /// Extracted text content.
    pub text: String,
    /// Text regions with positional information.
    pub regions: Vec<TextRegion>,
    /// Overall confidence score for the extraction.
    pub confidence: Option<f32>,
    /// Detected language code.
    pub detected_language: Option<String>,
    /// Number of pages processed.
    pub pages_processed: u32,
    /// Processing time in milliseconds.
    pub processing_time_ms: Option<u64>,
    /// When this response was generated.
    pub timestamp: Timestamp,
    /// Usage statistics for this operation.
    pub usage: UsageStats,
    /// Additional metadata about the processing.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Response {
    /// Create a new OCR response.
    pub fn new(request_id: Uuid, text: String) -> Self {
        Self {
            response_id: Uuid::new_v4(),
            request_id,
            text,
            regions: Vec::new(),
            confidence: None,
            detected_language: None,
            pages_processed: 1,
            processing_time_ms: None,
            timestamp: Timestamp::now(),
            usage: UsageStats::default(),
            metadata: HashMap::new(),
        }
    }

    /// Set the confidence score for this response.
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = Some(confidence);
        self
    }

    /// Set the detected language.
    pub fn with_language(mut self, language: String) -> Self {
        self.detected_language = Some(language);
        self
    }

    /// Set the text regions.
    pub fn with_regions(mut self, regions: Vec<TextRegion>) -> Self {
        self.regions = regions;
        self
    }

    /// Set the number of pages processed.
    pub fn with_pages_processed(mut self, pages: u32) -> Self {
        self.pages_processed = pages;
        self
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

    /// Get the word count of extracted text.
    pub fn word_count(&self) -> usize {
        self.text.split_whitespace().count()
    }

    /// Get the character count of extracted text.
    pub fn character_count(&self) -> usize {
        self.text.chars().count()
    }

    /// Check if any text was extracted.
    pub fn has_text(&self) -> bool {
        !self.text.trim().is_empty()
    }

    /// Check if the response has high confidence.
    pub fn is_high_confidence(&self) -> bool {
        self.confidence.map_or(false, |c| c >= 0.8)
    }

    /// Get text from a specific region type.
    pub fn text_by_region_type(&self, region_type: &super::context::TextRegionType) -> String {
        self.regions
            .iter()
            .filter(|r| {
                std::mem::discriminant(&r.region_type) == std::mem::discriminant(region_type)
            })
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Get all words with their bounding boxes.
    pub fn words_with_positions(&self) -> Vec<WordPosition> {
        self.regions
            .iter()
            .flat_map(|region| {
                region.words.iter().map(|word| WordPosition {
                    text: word.text.clone(),
                    bounding_box: word.bounding_box.clone(),
                    confidence: Some(word.confidence),
                    region_id: region.region_type.clone(),
                })
            })
            .collect()
    }

    /// Get statistics about the extraction quality.
    pub fn quality_stats(&self) -> QualityStats {
        let total_regions = self.regions.len();
        let regions_with_confidence: Vec<f32> = self.regions.iter().map(|r| r.confidence).collect();

        let avg_confidence = if regions_with_confidence.is_empty() {
            self.confidence.unwrap_or(0.0)
        } else {
            regions_with_confidence.iter().sum::<f32>() / regions_with_confidence.len() as f32
        };

        let low_confidence_regions = regions_with_confidence.iter().filter(|&&c| c < 0.5).count();

        QualityStats {
            total_regions,
            average_confidence: avg_confidence,
            low_confidence_regions,
            has_language_detection: self.detected_language.is_some(),
            processing_time_ms: self.processing_time_ms.unwrap_or(0),
        }
    }
}

/// A word with its position information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordPosition {
    /// The word text.
    pub text: String,
    /// Bounding box of the word.
    pub bounding_box: BoundingBox,
    /// Confidence score for this word.
    pub confidence: Option<f32>,
    /// Type of region this word belongs to.
    pub region_id: super::context::TextRegionType,
}

/// Quality statistics for an OCR response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityStats {
    /// Total number of text regions detected.
    pub total_regions: usize,
    /// Average confidence across all regions.
    pub average_confidence: f32,
    /// Number of regions with low confidence (< 0.5).
    pub low_confidence_regions: usize,
    /// Whether language detection was performed.
    pub has_language_detection: bool,
    /// Processing time in milliseconds.
    pub processing_time_ms: u64,
}

impl QualityStats {
    /// Check if the overall quality is good.
    pub fn is_good_quality(&self) -> bool {
        self.average_confidence >= 0.7
            && (self.low_confidence_regions as f32 / self.total_regions as f32) < 0.2
    }

    /// Get quality grade as a letter.
    pub fn quality_grade(&self) -> char {
        if self.average_confidence >= 0.9 {
            'A'
        } else if self.average_confidence >= 0.8 {
            'B'
        } else if self.average_confidence >= 0.7 {
            'C'
        } else if self.average_confidence >= 0.5 {
            'D'
        } else {
            'F'
        }
    }
}

/// Batch response containing multiple OCR results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResponse {
    /// Unique identifier for this batch response.
    pub batch_id: Uuid,
    /// Individual responses in the batch.
    pub responses: Vec<Response>,
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
