//! Response types for Vision Language Model operations.
//!
//! This module provides response structures for VLM operations, including
//! comprehensive visual analysis results, confidence scores, detected objects,
//! and streaming support for real-time interaction.

use std::collections::HashMap;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// Usage statistics for VLM operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    /// Number of tokens in the prompt
    pub prompt_tokens: u64,
    /// Number of tokens in the completion
    pub completion_tokens: u64,
    /// Total number of tokens used
    pub total_tokens: u64,
    /// Number of images processed
    pub images_processed: Option<u64>,
}

impl Usage {
    /// Create a new usage record.
    pub fn new(prompt_tokens: u64, completion_tokens: u64) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
            images_processed: None,
        }
    }

    /// Set the number of images processed.
    pub fn with_images(mut self, count: u64) -> Self {
        self.images_processed = Some(count);
        self
    }
}

/// Response from Vision Language Model operations.
///
/// This structure represents the complete response from a VLM service,
/// including the generated analysis, visual metadata, confidence scores,
/// and usage statistics.
///
/// # Examples
///
/// ```rust
/// use nvisy_core::vlm::response::{VlmResponse, VisualAnalysis};
/// use std::time::SystemTime;
///
/// let response = VlmResponse {
///     content: "This image shows a beautiful sunset over the ocean.".to_string(),
///     model: "gpt-4-vision".to_string(),
///     usage: None,
///     finish_reason: Some("complete".to_string()),
///     created: SystemTime::now(),
///     confidence: Some(0.95),
///     visual_analysis: None,
///     metadata: ResponseMetadata::default(),
/// };
///
/// assert_eq!(response.content, "This image shows a beautiful sunset over the ocean.");
/// assert!(response.is_complete());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// The generated text content describing or analyzing the visual input.
    pub content: String,
    /// The model that generated this response.
    pub model: String,
    /// Token usage information, if available.
    pub usage: Option<Usage>,
    /// Reason why generation finished.
    pub finish_reason: Option<String>,
    /// Timestamp when the response was created.
    pub created: SystemTime,
    /// Overall confidence score for the analysis (0.0 to 1.0).
    pub confidence: Option<f64>,
    /// Detailed visual analysis results.
    pub visual_analysis: Option<VisualAnalysis>,
    /// Additional response metadata.
    pub metadata: ResponseMetadata,
}

impl Response {
    /// Create a new VLM response.
    ///
    /// # Arguments
    ///
    /// * `content` - The generated analysis text
    /// * `model` - The model identifier
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_core::vlm::response::VlmResponse;
    ///
    /// let response = VlmResponse::new(
    ///     "The image contains three cats sitting on a windowsill",
    ///     "claude-3-vision"
    /// );
    /// ```
    pub fn new<C: Into<String>, M: Into<String>>(content: C, model: M) -> Self {
        Self {
            content: content.into(),
            model: model.into(),
            usage: None,
            finish_reason: None,
            created: SystemTime::now(),
            confidence: None,
            visual_analysis: None,
            metadata: ResponseMetadata::default(),
        }
    }

    /// Create a new response with usage information.
    pub fn with_usage<C: Into<String>, M: Into<String>>(
        content: C,
        model: M,
        usage: Usage,
    ) -> Self {
        Self {
            content: content.into(),
            model: model.into(),
            usage: Some(usage),
            finish_reason: None,
            created: SystemTime::now(),
            confidence: None,
            visual_analysis: None,
            metadata: ResponseMetadata::default(),
        }
    }

    /// Set the finish reason.
    pub fn with_finish_reason<S: Into<String>>(mut self, reason: S) -> Self {
        self.finish_reason = Some(reason.into());
        self
    }

    /// Set the confidence score.
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }

    /// Add visual analysis results.
    pub fn with_visual_analysis(mut self, analysis: VisualAnalysis) -> Self {
        self.visual_analysis = Some(analysis);
        self
    }

    /// Add metadata to the response.
    pub fn with_metadata(mut self, metadata: ResponseMetadata) -> Self {
        self.metadata = metadata;
        self
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

    /// Get detected objects if available.
    pub fn detected_objects(&self) -> Option<&Vec<DetectedObject>> {
        self.visual_analysis.as_ref()?.detected_objects.as_ref()
    }

    /// Get text extraction results if available.
    pub fn extracted_text(&self) -> Option<&Vec<TextRegion>> {
        self.visual_analysis.as_ref()?.text_regions.as_ref()
    }

    /// Get the dominant colors if available.
    pub fn dominant_colors(&self) -> Option<&Vec<ColorInfo>> {
        self.visual_analysis.as_ref()?.dominant_colors.as_ref()
    }

    /// Get image properties if available.
    pub fn image_properties(&self) -> Option<&ImageProperties> {
        self.visual_analysis.as_ref()?.image_properties.as_ref()
    }

    /// Convert to a streaming chunk (for compatibility).
    pub fn to_chunk(&self) -> VlmResponseChunk {
        VlmResponseChunk {
            content: self.content.clone(),
            finish_reason: self.finish_reason.clone(),
            usage: self.usage.clone(),
            model: self.model.clone(),
            created: self.created,
            confidence: self.confidence,
        }
    }
}

/// Streaming chunk from VLM operations.
///
/// This structure represents a single chunk in a streaming VLM response,
/// containing partial content and metadata about the streaming progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlmResponseChunk {
    /// Partial content in this chunk.
    pub content: String,
    /// Finish reason, if this is the final chunk.
    pub finish_reason: Option<String>,
    /// Usage information, typically only in the final chunk.
    pub usage: Option<Usage>,
    /// The model generating this chunk.
    pub model: String,
    /// Timestamp when the chunk was created.
    pub created: SystemTime,
    /// Confidence score for this chunk.
    pub confidence: Option<f64>,
}

impl VlmResponseChunk {
    /// Create a new response chunk.
    pub fn new<C: Into<String>, M: Into<String>>(content: C, model: M) -> Self {
        Self {
            content: content.into(),
            model: model.into(),
            finish_reason: None,
            usage: None,
            created: SystemTime::now(),
            confidence: None,
        }
    }

    /// Create a final chunk with finish reason.
    pub fn final_chunk<C: Into<String>, M: Into<String>, R: Into<String>>(
        content: C,
        model: M,
        finish_reason: R,
    ) -> Self {
        Self {
            content: content.into(),
            model: model.into(),
            finish_reason: Some(finish_reason.into()),
            usage: None,
            created: SystemTime::now(),
            confidence: None,
        }
    }

    /// Create a final chunk with usage and confidence.
    pub fn with_completion<C: Into<String>, M: Into<String>, R: Into<String>>(
        content: C,
        model: M,
        finish_reason: R,
        usage: Usage,
        confidence: Option<f64>,
    ) -> Self {
        Self {
            content: content.into(),
            model: model.into(),
            finish_reason: Some(finish_reason.into()),
            usage: Some(usage),
            created: SystemTime::now(),
            confidence,
        }
    }

    /// Check if this is the final chunk in the stream.
    pub fn is_final(&self) -> bool {
        self.finish_reason.is_some()
    }

    /// Check if this chunk represents a complete response.
    pub fn is_complete(&self) -> bool {
        matches!(
            self.finish_reason.as_deref(),
            Some("complete") | Some("stop") | Some("end_turn")
        )
    }

    /// Convert to a full response (for final chunks).
    pub fn to_response(&self) -> Response {
        Response {
            content: self.content.clone(),
            model: self.model.clone(),
            usage: self.usage.clone(),
            finish_reason: self.finish_reason.clone(),
            created: self.created,
            confidence: self.confidence,
            visual_analysis: None,
            metadata: ResponseMetadata::default(),
        }
    }
}

/// Detailed visual analysis results.
///
/// Contains structured information extracted from visual analysis,
/// including detected objects, text regions, colors, and image properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualAnalysis {
    /// Objects detected in the image.
    pub detected_objects: Option<Vec<DetectedObject>>,
    /// Text regions found in the image.
    pub text_regions: Option<Vec<TextRegion>>,
    /// Dominant colors in the image.
    pub dominant_colors: Option<Vec<ColorInfo>>,
    /// Image technical properties.
    pub image_properties: Option<ImageProperties>,
    /// Scene classification results.
    pub scene_classification: Option<Vec<SceneCategory>>,
    /// Estimated emotional tone or mood.
    pub emotional_analysis: Option<EmotionalAnalysis>,
    /// Quality assessment of the image.
    pub quality_assessment: Option<QualityAssessment>,
}

impl VisualAnalysis {
    /// Create a new empty visual analysis.
    pub fn new() -> Self {
        Self {
            detected_objects: None,
            text_regions: None,
            dominant_colors: None,
            image_properties: None,
            scene_classification: None,
            emotional_analysis: None,
            quality_assessment: None,
        }
    }

    /// Check if any analysis results are present.
    pub fn has_results(&self) -> bool {
        self.detected_objects
            .as_ref()
            .map(|v| !v.is_empty())
            .unwrap_or(false)
            || self
                .text_regions
                .as_ref()
                .map(|v| !v.is_empty())
                .unwrap_or(false)
            || self
                .dominant_colors
                .as_ref()
                .map(|v| !v.is_empty())
                .unwrap_or(false)
            || self
                .scene_classification
                .as_ref()
                .map(|v| !v.is_empty())
                .unwrap_or(false)
            || self.image_properties.is_some()
            || self.emotional_analysis.is_some()
            || self.quality_assessment.is_some()
    }

    /// Get the total number of detected objects.
    pub fn object_count(&self) -> usize {
        self.detected_objects.as_ref().map(|v| v.len()).unwrap_or(0)
    }

    /// Get objects with confidence above a threshold.
    pub fn high_confidence_objects(&self, threshold: f64) -> Vec<&DetectedObject> {
        self.detected_objects
            .as_ref()
            .map(|objects| {
                objects
                    .iter()
                    .filter(|obj| obj.confidence >= threshold)
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for VisualAnalysis {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about a detected object in the image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedObject {
    /// Object class or category name.
    pub class: String,
    /// Confidence score for this detection (0.0 to 1.0).
    pub confidence: f64,
    /// Bounding box coordinates.
    pub bounding_box: Option<BoundingBox>,
    /// Additional attributes or properties.
    pub attributes: HashMap<String, String>,
}

/// Bounding box coordinates for detected objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    /// X coordinate of the top-left corner.
    pub x: f64,
    /// Y coordinate of the top-left corner.
    pub y: f64,
    /// Width of the bounding box.
    pub width: f64,
    /// Height of the bounding box.
    pub height: f64,
}

impl BoundingBox {
    /// Calculate the area of the bounding box.
    pub fn area(&self) -> f64 {
        self.width * self.height
    }

    /// Get the center point of the bounding box.
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

/// Text region extracted from the image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRegion {
    /// Extracted text content.
    pub text: String,
    /// Confidence score for this text extraction (0.0 to 1.0).
    pub confidence: f64,
    /// Bounding box for the text region.
    pub bounding_box: Option<BoundingBox>,
    /// Detected language of the text.
    pub language: Option<String>,
    /// Font properties if detectable.
    pub font_properties: Option<FontProperties>,
}

/// Font properties for extracted text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontProperties {
    /// Estimated font size.
    pub size: Option<f64>,
    /// Font style (bold, italic, etc.).
    pub style: Option<String>,
    /// Font color information.
    pub color: Option<ColorInfo>,
}

/// Color information in the image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorInfo {
    /// Color name or description.
    pub name: Option<String>,
    /// RGB color values (0-255).
    pub rgb: (u8, u8, u8),
    /// HSV color values (hue: 0-360, saturation/value: 0-100).
    pub hsv: Option<(f64, f64, f64)>,
    /// Hex color code.
    pub hex: Option<String>,
    /// Percentage of image covered by this color.
    pub percentage: Option<f64>,
}

impl ColorInfo {
    /// Create color info from RGB values.
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            name: None,
            rgb: (r, g, b),
            hsv: None,
            hex: Some(format!("#{:02x}{:02x}{:02x}", r, g, b)),
            percentage: None,
        }
    }
}

/// Technical properties of the image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageProperties {
    /// Image width in pixels.
    pub width: Option<u32>,
    /// Image height in pixels.
    pub height: Option<u32>,
    /// Image format (JPEG, PNG, etc.).
    pub format: Option<String>,
    /// File size in bytes.
    pub file_size: Option<usize>,
    /// Color depth (bits per pixel).
    pub color_depth: Option<u32>,
    /// Whether the image has transparency.
    pub has_transparency: Option<bool>,
    /// EXIF data if available.
    pub exif_data: Option<HashMap<String, String>>,
}

/// Scene classification category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneCategory {
    /// Category name.
    pub category: String,
    /// Confidence score for this classification.
    pub confidence: f64,
    /// Subcategory if applicable.
    pub subcategory: Option<String>,
}

/// Emotional analysis of the image content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalAnalysis {
    /// Primary emotion detected.
    pub primary_emotion: String,
    /// Confidence score for the primary emotion.
    pub confidence: f64,
    /// Additional emotions with scores.
    pub emotions: HashMap<String, f64>,
    /// Overall emotional valence (-1.0 to 1.0, negative to positive).
    pub valence: Option<f64>,
    /// Emotional arousal level (0.0 to 1.0, calm to excited).
    pub arousal: Option<f64>,
}

/// Quality assessment of the image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityAssessment {
    /// Overall quality score (0.0 to 1.0).
    pub overall_score: f64,
    /// Sharpness assessment.
    pub sharpness: Option<f64>,
    /// Brightness level.
    pub brightness: Option<f64>,
    /// Contrast level.
    pub contrast: Option<f64>,
    /// Color saturation.
    pub saturation: Option<f64>,
    /// Noise level assessment.
    pub noise_level: Option<f64>,
    /// Specific quality issues detected.
    pub issues: Vec<String>,
}

/// Additional metadata about the VLM response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResponseMetadata {
    /// Processing time for visual analysis in milliseconds.
    pub processing_time_ms: Option<u64>,
    /// Model-specific confidence scores.
    pub model_confidence: Option<HashMap<String, f64>>,
    /// Features used in the analysis.
    pub features_used: Vec<String>,
    /// Any warnings or notes about the analysis.
    pub warnings: Vec<String>,
    /// Additional service-specific metadata.
    pub extra: HashMap<String, serde_json::Value>,
}

/// Builder for creating comprehensive VLM responses.
pub struct VlmResponseBuilder {
    content: String,
    model: String,
    usage: Option<Usage>,
    finish_reason: Option<String>,
    confidence: Option<f64>,
    visual_analysis: Option<VisualAnalysis>,
    metadata: ResponseMetadata,
}

impl VlmResponseBuilder {
    /// Create a new response builder.
    pub fn new<C: Into<String>, M: Into<String>>(content: C, model: M) -> Self {
        Self {
            content: content.into(),
            model: model.into(),
            usage: None,
            finish_reason: None,
            confidence: None,
            visual_analysis: None,
            metadata: ResponseMetadata::default(),
        }
    }

    /// Set usage information.
    pub fn usage(mut self, usage: Usage) -> Self {
        self.usage = Some(usage);
        self
    }

    /// Set finish reason.
    pub fn finish_reason<S: Into<String>>(mut self, reason: S) -> Self {
        self.finish_reason = Some(reason.into());
        self
    }

    /// Set confidence score.
    pub fn confidence(mut self, confidence: f64) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }

    /// Add visual analysis results.
    pub fn visual_analysis(mut self, analysis: VisualAnalysis) -> Self {
        self.visual_analysis = Some(analysis);
        self
    }

    /// Add detected objects.
    pub fn detected_objects(mut self, objects: Vec<DetectedObject>) -> Self {
        let mut analysis = self.visual_analysis.unwrap_or_default();
        analysis.detected_objects = Some(objects);
        self.visual_analysis = Some(analysis);
        self
    }

    /// Add processing time metadata.
    pub fn processing_time(mut self, time_ms: u64) -> Self {
        self.metadata.processing_time_ms = Some(time_ms);
        self
    }

    /// Add a warning.
    pub fn warning<S: Into<String>>(mut self, warning: S) -> Self {
        self.metadata.warnings.push(warning.into());
        self
    }

    /// Build the final response.
    pub fn build(self) -> Response {
        Response {
            content: self.content,
            model: self.model,
            usage: self.usage,
            finish_reason: self.finish_reason,
            created: SystemTime::now(),
            confidence: self.confidence,
            visual_analysis: self.visual_analysis,
            metadata: self.metadata,
        }
    }
}
