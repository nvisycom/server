//! Switch types for conditional data routing.

use serde::{Deserialize, Serialize};

/// A switch node definition that routes data based on a condition.
///
/// Switch nodes evaluate a condition against incoming data and route it
/// to either the `true` or `false` output branch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchDef {
    /// The condition to evaluate.
    pub condition: SwitchCondition,
}

impl SwitchDef {
    /// Creates a new switch definition.
    pub fn new(condition: SwitchCondition) -> Self {
        Self { condition }
    }
}

/// Switch condition enum - each variant is a distinct condition type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SwitchCondition {
    /// Match by file category (based on extension).
    FileCategory(FileCategoryCondition),
    /// Match by detected content language.
    Language(LanguageCondition),
}

/// Condition that matches by file category based on extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileCategoryCondition {
    /// File category to match.
    pub category: FileCategory,
}

/// Condition that matches by detected content language.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LanguageCondition {
    /// Language codes to match (e.g., "en", "es", "fr").
    pub codes: Vec<String>,
    /// Minimum confidence threshold (0.0 to 1.0).
    #[serde(default = "default_confidence")]
    pub min_confidence: f32,
}

fn default_confidence() -> f32 {
    0.8
}

/// File categories for routing based on extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileCategory {
    /// Text files (.txt, .md, etc.).
    Text,
    /// Image files (.jpg, .png, .gif, etc.).
    Image,
    /// Audio files (.mp3, .wav, .flac, etc.).
    Audio,
    /// Video files (.mp4, .webm, etc.).
    Video,
    /// Document files (.pdf, .docx, etc.).
    Document,
    /// Archive files (.zip, .tar, etc.).
    Archive,
    /// Spreadsheet files (.xlsx, .csv, etc.).
    Spreadsheet,
    /// Presentation files (.pptx, etc.).
    Presentation,
    /// Code/source files.
    Code,
    /// Other/unknown file type.
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_switch_def_new() {
        let switch = SwitchDef::new(SwitchCondition::FileCategory(FileCategoryCondition {
            category: FileCategory::Image,
        }));

        assert!(matches!(switch.condition, SwitchCondition::FileCategory(_)));
    }

    #[test]
    fn test_serialization() {
        let switch = SwitchDef::new(SwitchCondition::Language(LanguageCondition {
            codes: vec!["en".into(), "es".into()],
            min_confidence: 0.9,
        }));

        let json = serde_json::to_string_pretty(&switch).unwrap();
        let deserialized: SwitchDef = serde_json::from_str(&json).unwrap();
        assert_eq!(switch, deserialized);
    }
}
