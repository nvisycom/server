//! Switch node for conditional routing.

use serde::{Deserialize, Serialize};

/// A switch node that routes data to different branches based on conditions.
///
/// Switch nodes evaluate conditions against incoming data and route it
/// to the appropriate output branch. Each branch has a condition and a
/// target cache slot or output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchNode {
    /// Branches to evaluate in order.
    pub branches: Vec<SwitchBranch>,
    /// Default branch if no conditions match.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// A single branch in a switch node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchBranch {
    /// Condition to evaluate.
    pub condition: SwitchCondition,
    /// Target cache slot name to route matching data.
    pub target: String,
}

/// Condition for switch branch evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SwitchCondition {
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

impl SwitchNode {
    /// Creates a new switch node with the given branches.
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

impl SwitchBranch {
    /// Creates a new branch with the given condition and target.
    pub fn new(condition: SwitchCondition, target: impl Into<String>) -> Self {
        Self {
            condition,
            target: target.into(),
        }
    }
}
