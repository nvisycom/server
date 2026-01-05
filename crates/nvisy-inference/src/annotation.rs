//! Annotation types for marking up content with AI-generated insights.
//!
//! This module provides comprehensive annotation structures for labeling and marking
//! various types of content including text, images, and documents. Annotations can
//! represent entity recognition, sentiment analysis, object detection, and other
//! AI-powered content analysis results.

use std::collections::HashMap;
use std::fmt;

use bytes::Bytes;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, Result};

/// A comprehensive annotation that can mark up various types of content.
///
/// Annotations provide a way to attach AI-generated insights, labels, and metadata
/// to specific regions or elements within content. They support both text-based
/// annotations (spans) and spatial annotations (bounding boxes for images).
///
/// # Examples
///
/// Text entity annotation:
/// ```rust
/// use nvisy_inference::{Annotation, AnnotationType, TextSpan};
///
/// let annotation = Annotation::new(AnnotationType::Entity, "PERSON")
///     .with_text_span(TextSpan::new(0, 12))
///     .with_confidence(0.95);
/// ```
///
/// Image object detection:
/// ```rust
/// use nvisy_inference::{Annotation, AnnotationType, BoundingBox};
///
/// let annotation = Annotation::new(AnnotationType::Object, "car")
///     .with_bounding_box(BoundingBox::new(100.0, 150.0, 200.0, 300.0))
///     .with_confidence(0.87);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    /// Unique identifier for this annotation.
    pub id: Uuid,

    /// The type of annotation this represents.
    pub annotation_type: AnnotationType,

    /// Primary label or classification for this annotation.
    pub label: String,

    /// Confidence score for this annotation (0.0 to 1.0).
    pub confidence: Option<f32>,

    /// Text span information for text-based annotations.
    pub text_span: Option<TextSpan>,

    /// Bounding box information for spatial annotations.
    pub bounding_box: Option<BoundingBox>,

    /// Free-form text description or extracted content.
    pub content: Option<String>,

    /// Binary data associated with this annotation (e.g., cropped image).
    pub data: Option<Bytes>,

    /// Normalized value or score for this annotation.
    pub normalized_value: Option<f64>,

    /// Source that generated this annotation.
    pub source: Option<String>,

    /// Model or service that created this annotation.
    pub model: Option<String>,

    /// Timestamp when this annotation was created.
    pub created_at: jiff::Timestamp,

    /// Additional structured metadata.
    pub metadata: HashMap<String, serde_json::Value>,

    /// Relationships to other annotations.
    pub relations: Vec<AnnotationRelation>,

    /// Tags for categorization and filtering.
    pub tags: Vec<String>,
}

/// Types of annotations that can be applied to content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationType {
    /// Named entity recognition (person, organization, location, etc.).
    Entity,

    /// Sentiment analysis result.
    Sentiment,

    /// Intent classification.
    Intent,

    /// Topic or category classification.
    Category,

    /// Language detection.
    Language,

    /// Keyword or key phrase extraction.
    Keyword,

    /// Object detection in images.
    Object,

    /// Text detection and recognition in images.
    Text,

    /// Face detection and recognition.
    Face,

    /// Scene or activity recognition.
    Scene,

    /// Document structure elements (title, paragraph, table, etc.).
    Structure,

    /// Regions of interest or highlights.
    Region,

    /// Quality assessment or content moderation.
    Quality,

    /// Relationship or connection between elements.
    Relation,

    /// Custom or application-specific annotation.
    Custom,
}

/// Represents a span of text with start and end positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextSpan {
    /// Start position (inclusive, character offset).
    pub start: usize,

    /// End position (exclusive, character offset).
    pub end: usize,
}

/// Represents a rectangular bounding box for spatial annotations.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

/// Represents a relationship between two annotations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnnotationRelation {
    /// Type of relationship.
    pub relation_type: RelationType,

    /// ID of the target annotation.
    pub target_id: Uuid,

    /// Confidence score for this relationship.
    pub confidence: Option<f32>,

    /// Additional metadata about the relationship.
    pub metadata: HashMap<String, String>,
}

/// Types of relationships between annotations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    /// One annotation contains or encompasses another.
    Contains,

    /// Annotations refer to the same entity or concept.
    CoreferenceOf,

    /// One annotation modifies or describes another.
    ModifierOf,

    /// Annotations are part of the same group or cluster.
    GroupedWith,

    /// One annotation follows another in sequence.
    FollowedBy,

    /// One annotation is a child of another in a hierarchy.
    ChildOf,

    /// Annotations are similar or related.
    RelatedTo,

    /// One annotation contradicts another.
    Contradicts,

    /// Custom relationship type.
    Custom,
}

/// A collection of annotations with grouping and querying capabilities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnnotationSet {
    /// Unique identifier for this annotation set.
    pub id: Uuid,

    /// All annotations in this set.
    pub annotations: Vec<Annotation>,

    /// Source document or content ID.
    pub source_id: Option<Uuid>,

    /// Timestamp when this set was created.
    pub created_at: jiff::Timestamp,

    /// Set-level metadata.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl TextSpan {
    /// Creates a new text span.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Returns the length of the span.
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Returns true if the span is empty.
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// Returns true if this span contains the given position.
    pub fn contains(&self, pos: usize) -> bool {
        pos >= self.start && pos < self.end
    }

    /// Returns true if this span overlaps with another span.
    pub fn overlaps_with(&self, other: &TextSpan) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// Extracts the text content from the given string.
    pub fn extract<'a>(&self, text: &'a str) -> Option<&'a str> {
        text.get(self.start..self.end)
    }
}

impl BoundingBox {
    /// Creates a new bounding box.
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Creates a bounding box from coordinates (x1, y1, x2, y2).
    pub fn from_coordinates(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self {
            x: x1.min(x2),
            y: y1.min(y2),
            width: (x2 - x1).abs(),
            height: (y2 - y1).abs(),
        }
    }

    /// Returns the right edge x-coordinate.
    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    /// Returns the bottom edge y-coordinate.
    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    /// Returns the area of the bounding box.
    pub fn area(&self) -> f64 {
        self.width * self.height
    }

    /// Returns the center point of the bounding box.
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Returns true if this bounding box contains the given point.
    pub fn contains_point(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }

    /// Returns true if this bounding box overlaps with another.
    pub fn overlaps_with(&self, other: &BoundingBox) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    /// Returns the intersection of this bounding box with another.
    pub fn intersection(&self, other: &BoundingBox) -> Option<BoundingBox> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());

        if x < right && y < bottom {
            Some(BoundingBox::new(x, y, right - x, bottom - y))
        } else {
            None
        }
    }
}

impl Annotation {
    /// Creates a new annotation with the given type and label.
    pub fn new(annotation_type: AnnotationType, label: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            annotation_type,
            label: label.into(),
            confidence: None,
            text_span: None,
            bounding_box: None,
            content: None,
            data: None,
            normalized_value: None,
            source: None,
            model: None,
            relations: Vec::new(),
            tags: Vec::new(),
            metadata: HashMap::new(),
            created_at: Timestamp::now(),
        }
    }

    /// Sets the annotation ID.
    pub fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    /// Sets the confidence score.
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = Some(confidence);
        self
    }

    /// Sets the text span for text annotations.
    pub fn with_text_span(mut self, text_span: TextSpan) -> Self {
        self.text_span = Some(text_span);
        self
    }

    /// Sets the bounding box for spatial annotations.
    pub fn with_bounding_box(mut self, bounding_box: BoundingBox) -> Self {
        self.bounding_box = Some(bounding_box);
        self
    }

    /// Sets the content.
    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Sets the binary data.
    pub fn with_data(mut self, data: Bytes) -> Self {
        self.data = Some(data);
        self
    }

    /// Sets the normalized value.
    pub fn with_normalized_value(mut self, value: f64) -> Self {
        self.normalized_value = Some(value);
        self
    }

    /// Sets the source.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Adds a tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Adds multiple tags.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags.extend(tags);
        self
    }

    /// Adds metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Adds a relation.
    pub fn with_relation(mut self, relation: AnnotationRelation) -> Self {
        self.relations.push(relation);
        self
    }

    /// Returns true if this is a text-based annotation.
    pub fn is_text_annotation(&self) -> bool {
        self.text_span.is_some()
    }

    /// Returns true if this is a spatial annotation.
    pub fn is_spatial_annotation(&self) -> bool {
        self.bounding_box.is_some()
    }

    /// Adds a relationship to another annotation.
    pub fn add_relation(&mut self, relation: AnnotationRelation) {
        self.relations.push(relation);
    }

    /// Gets all relations of a specific type.
    pub fn get_relations(&self, relation_type: RelationType) -> Vec<&AnnotationRelation> {
        self.relations
            .iter()
            .filter(|r| r.relation_type == relation_type)
            .collect()
    }

    /// Validates the annotation data.
    pub fn validate(&self) -> Result<()> {
        if self.label.is_empty() {
            return Err(Error::invalid_input().with_message("Annotation label cannot be empty"));
        }

        if let Some(confidence) = self.confidence
            && !(0.0..=1.0).contains(&confidence)
        {
            return Err(
                Error::invalid_input().with_message("Confidence must be between 0.0 and 1.0")
            );
        }

        if let Some(span) = &self.text_span
            && span.is_empty()
        {
            return Err(Error::invalid_input().with_message("Text span cannot be empty"));
        }

        if let Some(bbox) = &self.bounding_box
            && (bbox.width <= 0.0 || bbox.height <= 0.0)
        {
            return Err(
                Error::invalid_input().with_message("Bounding box dimensions must be positive")
            );
        }

        Ok(())
    }
}

impl AnnotationSet {
    /// Creates a new empty annotation set.
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            annotations: Vec::new(),
            source_id: None,
            created_at: jiff::Timestamp::now(),
            metadata: HashMap::new(),
        }
    }

    /// Adds an annotation to this set.
    pub fn add_annotation(&mut self, annotation: Annotation) {
        self.annotations.push(annotation);
    }

    /// Gets annotations by type.
    pub fn get_by_type(&self, annotation_type: AnnotationType) -> Vec<&Annotation> {
        self.annotations
            .iter()
            .filter(|a| a.annotation_type == annotation_type)
            .collect()
    }

    /// Gets annotations by label.
    pub fn get_by_label(&self, label: &str) -> Vec<&Annotation> {
        self.annotations
            .iter()
            .filter(|a| a.label == label)
            .collect()
    }

    /// Gets annotations above a confidence threshold.
    pub fn get_by_confidence(&self, min_confidence: f32) -> Vec<&Annotation> {
        self.annotations
            .iter()
            .filter(|a| a.confidence.unwrap_or(0.0) >= min_confidence)
            .collect()
    }

    /// Gets annotations that overlap with the given text span.
    pub fn get_overlapping_text(&self, span: &TextSpan) -> Vec<&Annotation> {
        self.annotations
            .iter()
            .filter(|a| {
                a.text_span
                    .as_ref()
                    .map(|s| s.overlaps_with(span))
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Gets annotations that overlap with the given bounding box.
    pub fn get_overlapping_spatial(&self, bbox: &BoundingBox) -> Vec<&Annotation> {
        self.annotations
            .iter()
            .filter(|a| {
                a.bounding_box
                    .as_ref()
                    .map(|b| b.overlaps_with(bbox))
                    .unwrap_or(false)
            })
            .collect()
    }
}

impl Default for AnnotationSet {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AnnotationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Entity => write!(f, "entity"),
            Self::Sentiment => write!(f, "sentiment"),
            Self::Intent => write!(f, "intent"),
            Self::Category => write!(f, "category"),
            Self::Language => write!(f, "language"),
            Self::Keyword => write!(f, "keyword"),
            Self::Object => write!(f, "object"),
            Self::Text => write!(f, "text"),
            Self::Face => write!(f, "face"),
            Self::Scene => write!(f, "scene"),
            Self::Structure => write!(f, "structure"),
            Self::Region => write!(f, "region"),
            Self::Quality => write!(f, "quality"),
            Self::Relation => write!(f, "relation"),
            Self::Custom => write!(f, "custom"),
        }
    }
}
