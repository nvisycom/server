//! Annotation payload types and utilities.
//!
//! This module provides payload structures and utilities for annotation collections,
//! including point definitions, coordinate systems, and annotation types.

use serde::{Deserialize, Serialize};
#[cfg(feature = "schema")]
use schemars::JsonSchema;

use crate::SearchResult;
use crate::error::{Error, Result};
use crate::types::{Payload, Point, PointId, Vector};

/// Create a payload with standard metadata fields
fn create_metadata_payload() -> Payload {
    let now = jiff::Timestamp::now().to_string();
    Payload::new()
        .with("created_at", now.clone())
        .with("updated_at", now)
        .with("version", 1)
}

/// Types of annotations supported by the system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum AnnotationType {
    /// Text-based annotation
    Text,
    /// Image region annotation
    ImageRegion,
    /// Audio segment annotation
    AudioSegment,
    /// Video frame annotation
    VideoFrame,
    /// Document section annotation
    DocumentSection,
    /// Code annotation
    Code,
    /// Data annotation
    Data,
    /// Custom annotation type
    Custom(String),
}

impl AnnotationType {
    /// Get the string representation of the annotation type
    pub fn as_str(&self) -> &str {
        match self {
            AnnotationType::Text => "text",
            AnnotationType::ImageRegion => "image_region",
            AnnotationType::AudioSegment => "audio_segment",
            AnnotationType::VideoFrame => "video_frame",
            AnnotationType::DocumentSection => "document_section",
            AnnotationType::Code => "code",
            AnnotationType::Data => "data",
            AnnotationType::Custom(name) => name,
        }
    }
}

impl std::fmt::Display for AnnotationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Spatial coordinates for annotations that have positional information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct AnnotationCoordinates {
    /// X coordinate or left position
    pub x: f64,
    /// Y coordinate or top position
    pub y: f64,
    /// Width (for rectangular regions)
    pub width: Option<f64>,
    /// Height (for rectangular regions)
    pub height: Option<f64>,
    /// Additional points for complex shapes (polygons)
    pub points: Option<Vec<(f64, f64)>>,
}

impl AnnotationCoordinates {
    /// Create a point coordinate
    pub fn point(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            width: None,
            height: None,
            points: None,
        }
    }

    /// Create a rectangular region
    pub fn rectangle(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width: Some(width),
            height: Some(height),
            points: None,
        }
    }

    /// Create a polygon region
    pub fn polygon(points: Vec<(f64, f64)>) -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: None,
            height: None,
            points: Some(points),
        }
    }
}

/// Payload structure for updating annotation data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct AnnotationPayload {
    /// The annotation text or description
    pub content: Option<String>,

    /// Type of annotation
    pub annotation_type: Option<AnnotationType>,

    /// Spatial coordinates (for image/video annotations)
    pub coordinates: Option<AnnotationCoordinates>,

    /// Additional custom metadata
    pub metadata: Option<Payload>,
}

impl AnnotationPayload {
    /// Create a new empty annotation payload
    pub fn new() -> Self {
        Self {
            content: None,
            annotation_type: None,
            coordinates: None,
            metadata: None,
        }
    }

    /// Set the content
    pub fn with_content(mut self, content: String) -> Self {
        self.content = Some(content);
        self
    }

    /// Set the annotation type
    pub fn with_annotation_type(mut self, annotation_type: AnnotationType) -> Self {
        self.annotation_type = Some(annotation_type);
        self
    }

    /// Set the coordinates
    pub fn with_coordinates(mut self, coordinates: AnnotationCoordinates) -> Self {
        self.coordinates = Some(coordinates);
        self
    }

    /// Set custom metadata
    pub fn with_metadata(mut self, metadata: Payload) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Convert to a Payload for database operations
    pub fn to_payload(self) -> Payload {
        let mut payload = Payload::new();

        if let Some(content) = self.content {
            payload = payload.with("content", content);
        }

        if let Some(annotation_type) = self.annotation_type {
            payload = payload.with("annotation_type", annotation_type.as_str());
        }

        if let Some(coords) = self.coordinates {
            payload = payload.with("x", coords.x).with("y", coords.y);

            if let Some(width) = coords.width {
                payload = payload.with("width", width);
            }
            if let Some(height) = coords.height {
                payload = payload.with("height", height);
            }
            if let Some(points) = coords.points {
                let serializable_points: Vec<[f64; 2]> =
                    points.into_iter().map(|(x, y)| [x, y]).collect();
                payload = payload.with("points", serializable_points);
            }
        }

        if let Some(metadata) = self.metadata {
            payload.merge(&metadata);
        }

        payload
    }
}

impl Default for AnnotationPayload {
    fn default() -> Self {
        Self::new()
    }
}

/// A point representing an annotation in the vector database.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct AnnotationPoint {
    /// Unique identifier for the annotation
    pub id: PointId,

    /// Vector embedding of the annotation content
    pub embedding: Vector,

    /// Type of annotation
    pub annotation_type: AnnotationType,

    /// The annotation text or description
    pub content: String,

    /// ID of the source document/media
    pub source_id: String,

    /// User who created the annotation
    pub user_id: String,

    /// Spatial coordinates (for image/video annotations)
    pub coordinates: Option<AnnotationCoordinates>,

    /// Additional metadata
    pub metadata: Payload,
}

impl AnnotationPoint {
    /// Create a new annotation point
    pub fn new(
        id: impl Into<PointId>,
        embedding: Vector,
        annotation_type: AnnotationType,
        content: String,
        source_id: String,
        user_id: String,
    ) -> Self {
        Self {
            id: id.into(),
            embedding,
            annotation_type,
            content,
            source_id,
            user_id,
            coordinates: None,
            metadata: create_metadata_payload(),
        }
    }

    /// Create a text annotation
    pub fn text_annotation(
        id: impl Into<PointId>,
        embedding: Vector,
        content: String,
        source_id: String,
        user_id: String,
    ) -> Self {
        Self::new(
            id,
            embedding,
            AnnotationType::Text,
            content,
            source_id,
            user_id,
        )
    }

    /// Create an image annotation with coordinates
    pub fn image_annotation(
        id: impl Into<PointId>,
        embedding: Vector,
        content: String,
        source_id: String,
        user_id: String,
        coordinates: AnnotationCoordinates,
    ) -> Self {
        let mut annotation = Self::new(
            id,
            embedding,
            AnnotationType::ImageRegion,
            content,
            source_id,
            user_id,
        );
        annotation.coordinates = Some(coordinates);
        annotation
    }

    /// Add spatial coordinates to the annotation
    pub fn with_coordinates(mut self, coordinates: AnnotationCoordinates) -> Self {
        self.coordinates = Some(coordinates);
        self
    }

    /// Add additional metadata
    pub fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Convert to a generic Point for storage
    pub fn to_point(self) -> Point {
        let mut payload = Payload::new()
            .with("annotation_type", self.annotation_type.as_str())
            .with("content", self.content)
            .with("source_id", self.source_id)
            .with("user_id", self.user_id);

        if let Some(coords) = self.coordinates {
            payload = payload.with("x", coords.x).with("y", coords.y);

            if let Some(width) = coords.width {
                payload = payload.with("width", width);
            }
            if let Some(height) = coords.height {
                payload = payload.with("height", height);
            }
            if let Some(points) = coords.points {
                let serializable_points: Vec<[f64; 2]> =
                    points.into_iter().map(|(x, y)| [x, y]).collect();
                payload = payload.with("points", serializable_points);
            }
        }

        // Merge additional metadata
        payload.merge(&self.metadata);

        Point::new(self.id, self.embedding, payload)
    }

    /// Create from a search result
    pub fn from_search_result(result: SearchResult) -> Result<Self> {
        let id = result.id.clone();
        let embedding = result.vector().unwrap_or_default();
        let payload = result.payload;

        let annotation_type = match payload.get_string("annotation_type") {
            Some(type_str) => match type_str {
                "text" => AnnotationType::Text,
                "image_region" => AnnotationType::ImageRegion,
                "audio_segment" => AnnotationType::AudioSegment,
                "video_frame" => AnnotationType::VideoFrame,
                "document_section" => AnnotationType::DocumentSection,
                "code" => AnnotationType::Code,
                "data" => AnnotationType::Data,
                custom => AnnotationType::Custom(custom.to_string()),
            },
            None => {
                return Err(Error::invalid_input().with_message("Missing annotation_type"));
            }
        };

        let content = payload
            .get_string("content")
            .ok_or_else(|| Error::invalid_input().with_message("Missing content"))?
            .to_string();

        let source_id = payload
            .get_string("source_id")
            .ok_or_else(|| Error::invalid_input().with_message("Missing source_id"))?
            .to_string();

        let user_id = payload
            .get_string("user_id")
            .ok_or_else(|| Error::invalid_input().with_message("Missing user_id"))?
            .to_string();

        let coordinates = if payload.contains_key("x") && payload.contains_key("y") {
            let x = payload.get_f64("x").unwrap_or(0.0);
            let y = payload.get_f64("y").unwrap_or(0.0);
            let width = payload.get_f64("width");
            let height = payload.get_f64("height");

            // Try to get points array
            let points = payload.get("points").and_then(|v| {
                if let serde_json::Value::Array(arr) = v {
                    let mut coords = Vec::new();
                    for item in arr {
                        if let serde_json::Value::Array(pair) = item
                            && pair.len() == 2
                                && let (Some(px), Some(py)) = (pair[0].as_f64(), pair[1].as_f64()) {
                                    coords.push((px, py));
                                }
                    }
                    if coords.is_empty() {
                        None
                    } else {
                        Some(coords)
                    }
                } else {
                    None
                }
            });

            Some(AnnotationCoordinates {
                x,
                y,
                width,
                height,
                points,
            })
        } else {
            None
        };

        Ok(Self {
            id,
            embedding,
            annotation_type,
            content,
            source_id,
            user_id,
            coordinates,
            metadata: payload,
        })
    }
}

impl From<AnnotationPoint> for Point {
    fn from(annotation: AnnotationPoint) -> Self {
        annotation.to_point()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_annotation_type_conversion() {
        assert_eq!(AnnotationType::Text.as_str(), "text");
        assert_eq!(AnnotationType::ImageRegion.as_str(), "image_region");
        assert_eq!(AnnotationType::Custom("test".to_string()).as_str(), "test");
    }

    #[test]
    fn test_annotation_coordinates() {
        let point = AnnotationCoordinates::point(10.0, 20.0);
        assert_eq!(point.x, 10.0);
        assert_eq!(point.y, 20.0);
        assert!(point.width.is_none());

        let rect = AnnotationCoordinates::rectangle(0.0, 0.0, 100.0, 50.0);
        assert_eq!(rect.width, Some(100.0));
        assert_eq!(rect.height, Some(50.0));

        let poly = AnnotationCoordinates::polygon(vec![(0.0, 0.0), (10.0, 0.0), (5.0, 10.0)]);
        assert!(poly.points.is_some());
        assert_eq!(poly.points.unwrap().len(), 3);
    }

    #[test]
    fn test_annotation_point_creation() {
        let vector = Vector::new(vec![1.0, 2.0, 3.0]);
        let point = AnnotationPoint::text_annotation(
            "test-id",
            vector,
            "Test content".to_string(),
            "doc-123".to_string(),
            "user-456".to_string(),
        );

        assert_eq!(point.annotation_type, AnnotationType::Text);
        assert_eq!(point.content, "Test content");
        assert_eq!(point.source_id, "doc-123");
        assert_eq!(point.user_id, "user-456");
    }

    #[test]
    fn test_annotation_point_to_point_conversion() {
        let vector = Vector::new(vec![1.0, 2.0, 3.0]);
        let coords = AnnotationCoordinates::rectangle(10.0, 20.0, 100.0, 200.0);

        let annotation_point = AnnotationPoint::image_annotation(
            "test-id",
            vector,
            "Test image annotation".to_string(),
            "img-123".to_string(),
            "user-456".to_string(),
            coords,
        );

        let point = annotation_point.to_point();

        assert_eq!(
            point.payload.get_string("annotation_type"),
            Some("image_region")
        );
        assert_eq!(
            point.payload.get_string("content"),
            Some("Test image annotation")
        );
        assert_eq!(point.payload.get_f64("x"), Some(10.0));
        assert_eq!(point.payload.get_f64("width"), Some(100.0));
    }
}
