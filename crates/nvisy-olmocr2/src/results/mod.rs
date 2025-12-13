//! Results module for OCR processing results
//!
//! This module provides types and functionality for representing OCR results,
//! including extracted text, confidence scores, bounding boxes, and metadata.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::ImageFormat;

/// Complete OCR result for a processed document
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrResult {
    /// Unique identifier for this OCR result
    pub id: Uuid,
    /// Processing task identifier
    pub task_id: Option<Uuid>,
    /// Overall confidence score (0.0 to 1.0)
    pub confidence: ConfidenceScore,
    /// Extracted text blocks
    pub text_blocks: Vec<TextBlock>,
    /// Document metadata
    pub metadata: DocumentMetadata,
    /// Processing statistics
    pub processing_stats: ProcessingStats,
    /// Raw extracted text (concatenated from all blocks)
    pub raw_text: String,
    /// Formatted text with structure preservation
    pub formatted_text: Option<String>,
    /// Processing errors or warnings
    pub issues: Vec<ProcessingIssue>,
}

/// Individual text block extracted from the document
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextBlock {
    /// Unique identifier for this text block
    pub id: Uuid,
    /// Extracted text content
    pub text: String,
    /// Confidence score for this block
    pub confidence: ConfidenceScore,
    /// Bounding box coordinates
    pub bounding_box: BoundingBox,
    /// Text properties
    pub properties: TextProperties,
    /// Block type (paragraph, heading, table, etc.)
    pub block_type: BlockType,
    /// Language detected for this block
    pub language: Option<String>,
    /// Reading order index
    pub reading_order: u32,
}

/// Bounding box coordinates for text elements
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    /// X coordinate of top-left corner
    pub x: f64,
    /// Y coordinate of top-left corner
    pub y: f64,
    /// Width of the bounding box
    pub width: f64,
    /// Height of the bounding box
    pub height: f64,
    /// Coordinate system used
    pub coordinate_system: CoordinateSystem,
}

/// Confidence scoring for OCR results
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceScore {
    /// Overall confidence (0.0 to 1.0)
    pub overall: f64,
    /// Character-level confidence
    pub character_level: Option<Vec<f64>>,
    /// Word-level confidence
    pub word_level: Option<Vec<f64>>,
    /// Line-level confidence
    pub line_level: Option<Vec<f64>>,
    /// Confidence calculation method
    pub method: ConfidenceMethod,
}

/// Method used to calculate confidence scores
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfidenceMethod {
    /// Model-native confidence scoring
    ModelNative,
    /// Statistical analysis of character probabilities
    Statistical,
    /// Ensemble of multiple confidence methods
    Ensemble,
    /// Custom confidence calculation
    Custom(String),
}

/// Text properties for styling and formatting
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextProperties {
    /// Font family (if detected)
    pub font_family: Option<String>,
    /// Font size in points
    pub font_size: Option<f64>,
    /// Text styling flags
    pub style: TextStyle,
    /// Text color (RGB)
    pub color: Option<(u8, u8, u8)>,
    /// Background color (RGB)
    pub background_color: Option<(u8, u8, u8)>,
    /// Text alignment
    pub alignment: TextAlignment,
    /// Line spacing
    pub line_spacing: Option<f64>,
}

/// Text styling flags
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextStyle {
    /// Bold text
    pub bold: bool,
    /// Italic text
    pub italic: bool,
    /// Underlined text
    pub underlined: bool,
    /// Strikethrough text
    pub strikethrough: bool,
    /// Subscript text
    pub subscript: bool,
    /// Superscript text
    pub superscript: bool,
}

/// Text alignment options
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlignment {
    /// Left-aligned text
    Left,
    /// Center-aligned text
    Center,
    /// Right-aligned text
    Right,
    /// Justified text
    Justified,
}

/// Type of text block
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockType {
    /// Regular paragraph text
    Paragraph,
    /// Heading text (H1-H6)
    Heading(u8),
    /// List item
    ListItem,
    /// Table cell
    TableCell,
    /// Image caption
    Caption,
    /// Footnote
    Footnote,
    /// Header/footer
    HeaderFooter,
    /// Mathematical formula
    Formula,
    /// Code block
    Code,
    /// Unknown or unclassified block
    Unknown,
}

/// Coordinate system for bounding boxes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordinateSystem {
    /// Top-left origin, Y increases downward (standard image coordinates)
    ImageCoordinates,
    /// Bottom-left origin, Y increases upward (PDF coordinates)
    PdfCoordinates,
    /// Normalized coordinates (0.0 to 1.0)
    Normalized,
    /// Custom coordinate system
    Custom(String),
}

/// Document metadata extracted during OCR
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentMetadata {
    /// Original document filename
    pub filename: Option<String>,
    /// Document format
    pub format: ImageFormat,
    /// Document dimensions (width, height)
    pub dimensions: Option<(u32, u32)>,
    /// Document resolution (DPI)
    pub resolution: Option<u32>,
    /// Number of pages (for multi-page documents)
    pub page_count: u32,
    /// Current page number (1-based)
    pub current_page: u32,
    /// Document language(s) detected
    pub languages: Vec<String>,
    /// Document creation timestamp (if available)
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Document modification timestamp (if available)
    pub modified_at: Option<chrono::DateTime<chrono::Utc>>,
    /// File size in bytes
    pub file_size: usize,
    /// Document properties extracted from metadata
    pub properties: HashMap<String, String>,
}

/// Processing statistics for the OCR operation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessingStats {
    /// Model used for processing
    pub model_id: String,
    /// Processing start time
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// Processing duration
    pub duration: Duration,
    /// Total characters extracted
    pub character_count: usize,
    /// Total words extracted
    pub word_count: usize,
    /// Total lines extracted
    pub line_count: usize,
    /// Total text blocks extracted
    pub block_count: usize,
    /// Average confidence across all text
    pub average_confidence: f64,
    /// Processing quality metrics
    pub quality_metrics: QualityMetrics,
    /// Resource usage during processing
    pub resource_usage: ResourceUsage,
}

/// Quality metrics for the OCR result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityMetrics {
    /// Overall quality score (0.0 to 1.0)
    pub overall_quality: f64,
    /// Text clarity score
    pub text_clarity: f64,
    /// Layout detection accuracy
    pub layout_accuracy: f64,
    /// Language detection confidence
    pub language_confidence: f64,
    /// Character recognition accuracy (if ground truth available)
    pub character_accuracy: Option<f64>,
    /// Word recognition accuracy (if ground truth available)
    pub word_accuracy: Option<f64>,
}

/// Resource usage during processing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Peak memory usage in bytes
    pub peak_memory: Option<usize>,
    /// CPU time used
    pub cpu_time: Option<Duration>,
    /// GPU time used (if applicable)
    pub gpu_time: Option<Duration>,
    /// API calls made
    pub api_calls: u32,
    /// Data transferred in bytes
    pub data_transferred: usize,
}

/// Processing issues, errors, or warnings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessingIssue {
    /// Issue severity level
    pub severity: IssueSeverity,
    /// Issue code for programmatic handling
    pub code: String,
    /// Human-readable issue description
    pub message: String,
    /// Location where the issue occurred (if applicable)
    pub location: Option<BoundingBox>,
    /// Suggested resolution
    pub suggestion: Option<String>,
}

/// Severity levels for processing issues
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueSeverity {
    /// Informational message
    Info,
    /// Warning that doesn't prevent processing
    Warning,
    /// Error that may affect quality
    Error,
    /// Critical error that prevents processing
    Critical,
}

/// Extracted text with formatting and structure
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractedText {
    /// Plain text content
    pub plain_text: String,
    /// Text with basic formatting (Markdown-like)
    pub formatted_text: String,
    /// Structured text representation
    pub structured_text: StructuredText,
    /// Text extraction metadata
    pub extraction_metadata: ExtractionMetadata,
}

/// Structured representation of extracted text
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructuredText {
    /// Document sections
    pub sections: Vec<TextSection>,
    /// Tables found in the document
    pub tables: Vec<Table>,
    /// Lists found in the document
    pub lists: Vec<List>,
    /// Images and their captions
    pub images: Vec<ImageReference>,
}

/// Text section (chapter, section, subsection, etc.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextSection {
    /// Section title
    pub title: String,
    /// Section level (1 = top level, 2 = subsection, etc.)
    pub level: u8,
    /// Section content
    pub content: String,
    /// Subsections
    pub subsections: Vec<TextSection>,
    /// Section bounding box
    pub bounding_box: Option<BoundingBox>,
}

/// Table structure
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    /// Table caption
    pub caption: Option<String>,
    /// Number of rows
    pub rows: usize,
    /// Number of columns
    pub columns: usize,
    /// Table cells
    pub cells: Vec<Vec<TableCell>>,
    /// Table bounding box
    pub bounding_box: BoundingBox,
}

/// Table cell content
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableCell {
    /// Cell text content
    pub text: String,
    /// Cell confidence score
    pub confidence: f64,
    /// Row span
    pub row_span: usize,
    /// Column span
    pub column_span: usize,
    /// Cell bounding box
    pub bounding_box: BoundingBox,
}

/// List structure
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct List {
    /// List type
    pub list_type: ListType,
    /// List items
    pub items: Vec<ListItem>,
    /// List bounding box
    pub bounding_box: BoundingBox,
}

/// Type of list
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListType {
    /// Bulleted list
    Bulleted,
    /// Numbered list
    Numbered,
    /// Alphabetical list
    Alphabetical,
    /// Roman numeral list
    Roman,
}

/// List item
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListItem {
    /// Item text
    pub text: String,
    /// Item marker (bullet, number, etc.)
    pub marker: String,
    /// Nested items
    pub nested_items: Vec<ListItem>,
    /// Item bounding box
    pub bounding_box: BoundingBox,
}

/// Image reference in document
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageReference {
    /// Image caption
    pub caption: Option<String>,
    /// Image alt text
    pub alt_text: Option<String>,
    /// Image bounding box
    pub bounding_box: BoundingBox,
    /// Image properties
    pub properties: HashMap<String, String>,
}

/// Metadata about text extraction process
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractionMetadata {
    /// Extraction method used
    pub extraction_method: String,
    /// Text encoding detected
    pub encoding: Option<String>,
    /// Line ending style detected
    pub line_endings: LineEndingStyle,
    /// Text direction
    pub text_direction: TextDirection,
    /// Writing system
    pub writing_system: Option<String>,
}

/// Line ending styles
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineEndingStyle {
    /// Unix-style line endings (\n)
    Unix,
    /// Windows-style line endings (\r\n)
    Windows,
    /// Mac-style line endings (\r)
    Mac,
    /// Mixed line endings
    Mixed,
}

/// Text direction
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextDirection {
    /// Left-to-right
    LeftToRight,
    /// Right-to-left
    RightToLeft,
    /// Top-to-bottom
    TopToBottom,
    /// Mixed directions
    Mixed,
}

// Default implementations

impl Default for ConfidenceScore {
    fn default() -> Self {
        Self {
            overall: 0.0,
            character_level: None,
            word_level: None,
            line_level: None,
            method: ConfidenceMethod::ModelNative,
        }
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            underlined: false,
            strikethrough: false,
            subscript: false,
            superscript: false,
        }
    }
}

impl Default for TextProperties {
    fn default() -> Self {
        Self {
            font_family: None,
            font_size: None,
            style: TextStyle::default(),
            color: None,
            background_color: None,
            alignment: TextAlignment::Left,
            line_spacing: None,
        }
    }
}

impl Default for QualityMetrics {
    fn default() -> Self {
        Self {
            overall_quality: 0.0,
            text_clarity: 0.0,
            layout_accuracy: 0.0,
            language_confidence: 0.0,
            character_accuracy: None,
            word_accuracy: None,
        }
    }
}

impl Default for ResourceUsage {
    fn default() -> Self {
        Self {
            peak_memory: None,
            cpu_time: None,
            gpu_time: None,
            api_calls: 0,
            data_transferred: 0,
        }
    }
}

// Implementations

impl OcrResult {
    /// Create a new OCR result
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            task_id: None,
            confidence: ConfidenceScore::default(),
            text_blocks: Vec::new(),
            metadata: DocumentMetadata {
                filename: None,
                format: ImageFormat::Png,
                dimensions: None,
                resolution: None,
                page_count: 1,
                current_page: 1,
                languages: Vec::new(),
                created_at: None,
                modified_at: None,
                file_size: 0,
                properties: HashMap::new(),
            },
            processing_stats: ProcessingStats {
                model_id: String::new(),
                started_at: chrono::Utc::now(),
                duration: Duration::default(),
                character_count: 0,
                word_count: 0,
                line_count: 0,
                block_count: 0,
                average_confidence: 0.0,
                quality_metrics: QualityMetrics::default(),
                resource_usage: ResourceUsage::default(),
            },
            raw_text: String::new(),
            formatted_text: None,
            issues: Vec::new(),
        }
    }

    /// Get the total text content as a single string
    pub fn get_text(&self) -> &str {
        &self.raw_text
    }

    /// Check if the OCR result meets a minimum confidence threshold
    pub fn meets_confidence_threshold(&self, threshold: f64) -> bool {
        self.confidence.overall >= threshold
    }

    /// Get text blocks sorted by reading order
    pub fn text_blocks_by_reading_order(&self) -> Vec<&TextBlock> {
        let mut blocks: Vec<_> = self.text_blocks.iter().collect();
        blocks.sort_by_key(|block| block.reading_order);
        blocks
    }

    /// Filter text blocks by confidence threshold
    pub fn text_blocks_above_confidence(&self, threshold: f64) -> Vec<&TextBlock> {
        self.text_blocks
            .iter()
            .filter(|block| block.confidence.overall >= threshold)
            .collect()
    }

    /// Get issues of a specific severity level
    pub fn issues_by_severity(&self, severity: IssueSeverity) -> Vec<&ProcessingIssue> {
        self.issues
            .iter()
            .filter(|issue| issue.severity == severity)
            .collect()
    }

    /// Check if there are any critical issues
    pub fn has_critical_issues(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.severity == IssueSeverity::Critical)
    }
}

impl BoundingBox {
    /// Create a new bounding box
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
            coordinate_system: CoordinateSystem::ImageCoordinates,
        }
    }

    /// Get the right edge coordinate
    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    /// Get the bottom edge coordinate
    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    /// Get the center point coordinates
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Calculate the area of the bounding box
    pub fn area(&self) -> f64 {
        self.width * self.height
    }

    /// Check if this bounding box intersects with another
    pub fn intersects(&self, other: &BoundingBox) -> bool {
        !(self.right() <= other.x
            || self.x >= other.right()
            || self.bottom() <= other.y
            || self.y >= other.bottom())
    }

    /// Calculate intersection area with another bounding box
    pub fn intersection_area(&self, other: &BoundingBox) -> f64 {
        if !self.intersects(other) {
            return 0.0;
        }

        let left = self.x.max(other.x);
        let right = self.right().min(other.right());
        let top = self.y.max(other.y);
        let bottom = self.bottom().min(other.bottom());

        (right - left) * (bottom - top)
    }
}

impl ConfidenceScore {
    /// Create a new confidence score
    pub fn new(overall: f64) -> Self {
        Self {
            overall: overall.clamp(0.0, 1.0),
            character_level: None,
            word_level: None,
            line_level: None,
            method: ConfidenceMethod::ModelNative,
        }
    }

    /// Check if confidence meets threshold
    pub fn meets_threshold(&self, threshold: f64) -> bool {
        self.overall >= threshold
    }

    /// Get confidence as percentage
    pub fn as_percentage(&self) -> f64 {
        self.overall * 100.0
    }
}

impl fmt::Display for BlockType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockType::Paragraph => write!(f, "Paragraph"),
            BlockType::Heading(level) => write!(f, "Heading H{}", level),
            BlockType::ListItem => write!(f, "List Item"),
            BlockType::TableCell => write!(f, "Table Cell"),
            BlockType::Caption => write!(f, "Caption"),
            BlockType::Footnote => write!(f, "Footnote"),
            BlockType::HeaderFooter => write!(f, "Header/Footer"),
            BlockType::Formula => write!(f, "Formula"),
            BlockType::Code => write!(f, "Code"),
            BlockType::Unknown => write!(f, "Unknown"),
        }
    }
}

impl fmt::Display for IssueSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IssueSeverity::Info => write!(f, "INFO"),
            IssueSeverity::Warning => write!(f, "WARNING"),
            IssueSeverity::Error => write!(f, "ERROR"),
            IssueSeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocr_result_creation() {
        let result = OcrResult::new();
        assert!(!result.id.is_nil());
        assert_eq!(result.text_blocks.len(), 0);
        assert_eq!(result.raw_text, "");
    }

    #[test]
    fn test_bounding_box_operations() {
        let bbox1 = BoundingBox::new(10.0, 10.0, 20.0, 20.0);
        let bbox2 = BoundingBox::new(15.0, 15.0, 20.0, 20.0);

        assert_eq!(bbox1.right(), 30.0);
        assert_eq!(bbox1.bottom(), 30.0);
        assert_eq!(bbox1.center(), (20.0, 20.0));
        assert_eq!(bbox1.area(), 400.0);
        assert!(bbox1.intersects(&bbox2));
        assert_eq!(bbox1.intersection_area(&bbox2), 225.0);
    }

    #[test]
    fn test_confidence_score() {
        let confidence = ConfidenceScore::new(0.85);
        assert_eq!(confidence.overall, 0.85);
        assert!(confidence.meets_threshold(0.8));
        assert!(!confidence.meets_threshold(0.9));
        assert_eq!(confidence.as_percentage(), 85.0);
    }

    #[test]
    fn test_confidence_score_clamping() {
        let confidence1 = ConfidenceScore::new(1.5);
        assert_eq!(confidence1.overall, 1.0);

        let confidence2 = ConfidenceScore::new(-0.5);
        assert_eq!(confidence2.overall, 0.0);
    }

    #[test]
    fn test_text_block_creation() {
        let text_block = TextBlock {
            id: Uuid::new_v4(),
            text: "Hello World".to_string(),
            confidence: ConfidenceScore::new(0.95),
            bounding_box: BoundingBox::new(0.0, 0.0, 100.0, 20.0),
            properties: TextProperties::default(),
            block_type: BlockType::Paragraph,
            language: Some("en".to_string()),
            reading_order: 1,
        };

        assert_eq!(text_block.text, "Hello World");
        assert!(text_block.confidence.meets_threshold(0.9));
        assert_eq!(text_block.block_type, BlockType::Paragraph);
    }

    #[test]
    fn test_issue_severity_display() {
        assert_eq!(IssueSeverity::Info.to_string(), "INFO");
        assert_eq!(IssueSeverity::Warning.to_string(), "WARNING");
        assert_eq!(IssueSeverity::Error.to_string(), "ERROR");
        assert_eq!(IssueSeverity::Critical.to_string(), "CRITICAL");
    }

    #[test]
    fn test_block_type_display() {
        assert_eq!(BlockType::Paragraph.to_string(), "Paragraph");
        assert_eq!(BlockType::Heading(1).to_string(), "Heading H1");
        assert_eq!(BlockType::ListItem.to_string(), "List Item");
    }
}
