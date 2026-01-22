//! Routing types for conditional data flow.
//!
//! This module provides types for controlling data flow in workflows:
//! - [`CacheSlot`]: Named connection point for linking workflow branches
//! - [`SwitchDef`]: Conditional routing based on data properties
//!
//! Switch conditions follow the same pattern as transforms - each condition
//! type is a separate struct, and `SwitchCondition` is an enum combining them.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// A cache slot reference for in-memory data passing.
///
/// Cache slots act as named connection points that link different parts
/// of a workflow graph. During compilation, cache slots are resolved by
/// connecting incoming edges directly to outgoing edges with matching slot names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheSlot {
    /// Slot identifier (used as the key for matching inputs to outputs).
    pub slot: String,
    /// Priority for ordering when multiple slots are available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u32>,
}

impl CacheSlot {
    /// Creates a new cache slot with the given slot name.
    pub fn new(slot: impl Into<String>) -> Self {
        Self {
            slot: slot.into(),
            priority: None,
        }
    }

    /// Sets the priority.
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = Some(priority);
        self
    }
}

/// A switch node definition that routes data to different output ports based on conditions.
///
/// Switch nodes evaluate a condition against incoming data and route it
/// to the appropriate output port. Edges then connect each port to downstream nodes.
///
/// Each switch has exactly one condition type, similar to how transforms work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchDef {
    /// The condition to evaluate.
    pub condition: SwitchCondition,
    /// Output port for data matching the condition.
    pub match_port: String,
    /// Output port for data not matching the condition.
    pub else_port: String,
}

impl SwitchDef {
    /// Returns all output port names defined by this switch.
    pub fn output_ports(&self) -> impl Iterator<Item = &str> {
        [self.match_port.as_str(), self.else_port.as_str()].into_iter()
    }
}

/// Switch condition enum - each variant is a distinct condition type.
///
/// Similar to `Transformer`, each condition is a separate struct with its
/// own configuration, wrapped in this enum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SwitchCondition {
    /// Match by content type category.
    ContentType(ContentTypeCondition),
    /// Match by file extension.
    FileExtension(FileExtensionCondition),
    /// Match when file size is within range.
    FileSize(FileSizeCondition),
    /// Match when page count is within range.
    PageCount(PageCountCondition),
    /// Match when duration is within range (for audio/video).
    Duration(DurationCondition),
    /// Match by detected content language.
    Language(LanguageCondition),
    /// Match when file date is within range.
    FileDate(FileDateCondition),
    /// Match by filename pattern.
    FileName(FileNameCondition),
}

/// Condition that matches by content type category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentTypeCondition {
    /// Content type category to match.
    pub category: ContentTypeCategory,
}

/// Condition that matches by file extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileExtensionCondition {
    /// Extensions to match (without dot, e.g., "pdf", "docx").
    pub extensions: Vec<String>,
}

/// Condition that matches when file size is within range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSizeCondition {
    /// Minimum size in bytes (inclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_bytes: Option<u64>,
    /// Maximum size in bytes (inclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<u64>,
}

/// Condition that matches when page count is within range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageCountCondition {
    /// Minimum page count (inclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_pages: Option<u32>,
    /// Maximum page count (inclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_pages: Option<u32>,
}

/// Condition that matches when duration is within range (for audio/video).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DurationCondition {
    /// Minimum duration in seconds (inclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_seconds: Option<i64>,
    /// Maximum duration in seconds (inclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_seconds: Option<i64>,
}

/// Condition that matches by detected content language.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LanguageCondition {
    /// Language code to match (e.g., "en", "es", "fr").
    pub code: String,
    /// Minimum confidence threshold (0.0 to 1.0).
    #[serde(default = "default_confidence")]
    pub min_confidence: f32,
}

/// Condition that matches when file date is within range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDateCondition {
    /// Which date field to check.
    #[serde(default)]
    pub field: DateField,
    /// Earliest date (inclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<Timestamp>,
    /// Latest date (inclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<Timestamp>,
}

/// Condition that matches by filename pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileNameCondition {
    /// Pattern to match against filename.
    pub pattern: String,
    /// Pattern type.
    #[serde(default)]
    pub match_type: PatternMatchType,
}

/// Content type categories for routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentTypeCategory {
    /// Text files (plain text, markdown, etc.).
    Text,
    /// Image files (JPEG, PNG, GIF, etc.).
    Image,
    /// Audio files (MP3, WAV, FLAC, etc.).
    Audio,
    /// Video files (MP4, WebM, etc.).
    Video,
    /// Document files (PDF, DOCX, etc.).
    Document,
    /// Archive files (ZIP, TAR, etc.).
    Archive,
    /// Spreadsheet files (XLSX, CSV, etc.).
    Spreadsheet,
    /// Presentation files (PPTX, etc.).
    Presentation,
    /// Code/source files.
    Code,
    /// Other/unknown content type.
    Other,
}

/// Date field to use for date-based routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateField {
    /// File creation date.
    #[default]
    Created,
    /// File modification date.
    Modified,
}

/// Pattern matching type for filename conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternMatchType {
    /// Glob pattern (e.g., "*.pdf", "report_*").
    #[default]
    Glob,
    /// Regular expression pattern.
    Regex,
    /// Exact string match.
    Exact,
    /// Case-insensitive contains.
    Contains,
}

fn default_confidence() -> f32 {
    0.8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_switch_def_output_ports() {
        let switch = SwitchDef {
            condition: SwitchCondition::ContentType(ContentTypeCondition {
                category: ContentTypeCategory::Image,
            }),
            match_port: "images".into(),
            else_port: "other".into(),
        };

        let ports: Vec<_> = switch.output_ports().collect();
        assert_eq!(ports, vec!["images", "other"]);
    }

    #[test]
    fn test_serialization() {
        let switch = SwitchDef {
            condition: SwitchCondition::FileExtension(FileExtensionCondition {
                extensions: vec!["pdf".into(), "docx".into()],
            }),
            match_port: "documents".into(),
            else_port: "other".into(),
        };

        let json = serde_json::to_string_pretty(&switch).unwrap();
        let deserialized: SwitchDef = serde_json::from_str(&json).unwrap();
        assert_eq!(switch, deserialized);
    }
}
