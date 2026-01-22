//! Routing types for conditional data flow.
//!
//! This module provides types for controlling data flow in workflows:
//! - [`CacheSlot`]: Named connection point for linking workflow branches
//! - [`SwitchDef`]: Conditional routing based on data properties

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

/// A switch node definition that routes data to different branches based on conditions.
///
/// Switch nodes evaluate conditions against incoming data and route it
/// to the appropriate output branch. Each branch has a condition and a
/// target cache slot or output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchDef {
    /// Branches to evaluate in order.
    pub branches: Vec<SwitchBranch>,
    /// Default branch if no conditions match.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

impl SwitchDef {
    /// Creates a new switch definition with the given branches.
    pub fn new(branches: Vec<SwitchBranch>) -> Self {
        Self {
            branches,
            default: None,
        }
    }

    /// Sets the default target for unmatched data.
    pub fn with_default(mut self, target: impl Into<String>) -> Self {
        self.default = Some(target.into());
        self
    }
}

/// A single branch in a switch node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchBranch {
    /// Condition to evaluate.
    pub condition: SwitchCondition,
    /// Target cache slot name to route matching data.
    pub target: String,
}

impl SwitchBranch {
    /// Creates a new branch with the given condition and target.
    pub fn new(condition: SwitchCondition, target: impl Into<String>) -> Self {
        Self {
            condition,
            target: target.into(),
        }
    }
}

/// Condition for switch branch evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SwitchCondition {
    /// Always matches (catch-all).
    Always,
    /// Match by content type category.
    ContentType {
        /// Content type category to match.
        category: ContentTypeCategory,
    },
    /// Match when file size exceeds threshold.
    FileSizeAbove {
        /// Size threshold in bytes.
        threshold_bytes: u64,
    },
    /// Match when file size is below threshold.
    FileSizeBelow {
        /// Size threshold in bytes.
        threshold_bytes: u64,
    },
    /// Match when page count exceeds threshold.
    PageCountAbove {
        /// Page count threshold.
        threshold_pages: u32,
    },
    /// Match when duration exceeds threshold (for audio/video).
    DurationAbove {
        /// Duration threshold in seconds.
        threshold_seconds: u64,
    },
    /// Match by detected content language.
    Language {
        /// Language code to match (e.g., "en", "es", "fr").
        language_code: String,
        /// Minimum confidence threshold (0.0 to 1.0).
        #[serde(default = "default_confidence")]
        min_confidence: f32,
    },
    /// Match when file date is newer than threshold.
    DateNewerThan {
        /// Which date field to use.
        #[serde(default)]
        date_field: DateField,
        /// Threshold as ISO 8601 datetime or relative duration (e.g., "7d", "30d", "1y").
        threshold: String,
    },
    /// Match by filename regex pattern.
    FileNameMatches {
        /// Regex pattern to match against filename.
        pattern: String,
    },
    /// Match by file extension.
    FileExtension {
        /// Extension to match (without dot, e.g., "pdf", "docx").
        extension: String,
    },
    /// Match when metadata key exists.
    HasMetadata {
        /// Metadata key to check for.
        key: String,
    },
    /// Match when metadata key equals value.
    MetadataEquals {
        /// Metadata key to check.
        key: String,
        /// Value to match.
        value: String,
    },
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
}

/// Date field to use for routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateField {
    /// File creation date.
    #[default]
    Created,
    /// File modification date.
    Modified,
}

fn default_confidence() -> f32 {
    0.8
}
