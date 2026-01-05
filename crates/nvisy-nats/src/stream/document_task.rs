//! Predefined document processing tasks.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Predefined processing tasks that can be applied to documents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "task", rename_all = "snake_case")]
pub enum PredefinedTask {
    /// Redact sensitive information matching patterns.
    Redact {
        /// Patterns to redact (emails, phone numbers, SSNs, etc.).
        patterns: Vec<String>,
    },

    /// Summarize document content.
    Summarize {
        /// Maximum length of summary.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_length: Option<u32>,
    },

    /// Translate document to target language.
    Translate {
        /// Target language code (e.g., "es", "fr", "de").
        target_language: String,
    },

    /// Extract key information from document.
    ExtractInfo {
        /// Fields to extract (e.g., "dates", "names", "amounts").
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        fields: Vec<String>,
    },

    /// Insert information into document at specified locations.
    InsertInfo {
        /// Key-value pairs to insert.
        values: Vec<InsertValue>,
    },

    /// Generate information based on document content.
    GenerateInfo {
        /// Type of information to generate.
        info_type: GenerateInfoType,
    },

    /// Reformat document structure.
    Reformat {
        /// Target format style.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        style: Option<String>,
    },

    /// Proofread and fix grammar/spelling.
    Proofread,

    /// Generate table of contents.
    GenerateToc,

    /// Split document into multiple files.
    Split {
        /// How to split the document.
        strategy: SplitStrategy,
    },

    /// Merge multiple files into one document.
    Merge {
        /// File IDs to merge with this document.
        file_ids: Vec<Uuid>,
        /// Order of files in the merged document.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        order: Option<MergeOrder>,
    },
}

/// Value to insert into a document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct InsertValue {
    /// Field or placeholder name.
    pub field: String,
    /// Value to insert.
    pub value: String,
    /// Location hint (e.g., "header", "footer", "after:section1").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

/// Types of information that can be generated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GenerateInfoType {
    /// Generate an executive summary.
    ExecutiveSummary,
    /// Generate keywords/tags.
    Keywords,
    /// Generate document metadata.
    Metadata,
    /// Generate abstract.
    Abstract,
    /// Generate key takeaways.
    KeyTakeaways,
    /// Generate action items.
    ActionItems,
    /// Generate FAQ from content.
    Faq,
}

/// Strategy for splitting documents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "by", rename_all = "snake_case")]
pub enum SplitStrategy {
    /// Split by page count.
    Pages {
        /// Number of pages per split.
        pages_per_file: u32,
    },
    /// Split by sections/chapters.
    Sections,
    /// Split by heading level.
    Headings {
        /// Heading level to split on (1-6).
        level: u8,
    },
    /// Split by file size.
    Size {
        /// Maximum size per file in bytes.
        max_bytes: u64,
    },
    /// Split at specific page numbers.
    AtPages {
        /// Page numbers to split at.
        page_numbers: Vec<u32>,
    },
}

/// Order for merging documents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum MergeOrder {
    /// Use the order provided in file_ids.
    AsProvided,
    /// Sort by filename alphabetically.
    Alphabetical,
    /// Sort by creation date.
    ByDate,
    /// Sort by file size.
    BySize,
}

impl Default for MergeOrder {
    fn default() -> Self {
        Self::AsProvided
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_predefined_task_redact() {
        let task = PredefinedTask::Redact {
            patterns: vec!["email".to_string(), "phone".to_string()],
        };

        let json = serde_json::to_string(&task).unwrap();
        let parsed: PredefinedTask = serde_json::from_str(&json).unwrap();
        assert_eq!(task, parsed);
    }

    #[test]
    fn test_predefined_task_translate() {
        let task = PredefinedTask::Translate {
            target_language: "es".to_string(),
        };

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("translate"));
        assert!(json.contains("es"));
    }

    #[test]
    fn test_predefined_task_split() {
        let task = PredefinedTask::Split {
            strategy: SplitStrategy::Pages { pages_per_file: 10 },
        };

        let json = serde_json::to_string(&task).unwrap();
        let parsed: PredefinedTask = serde_json::from_str(&json).unwrap();
        assert_eq!(task, parsed);
    }

    #[test]
    fn test_predefined_task_merge() {
        let task = PredefinedTask::Merge {
            file_ids: vec![Uuid::now_v7(), Uuid::now_v7()],
            order: Some(MergeOrder::Alphabetical),
        };

        let json = serde_json::to_string(&task).unwrap();
        let parsed: PredefinedTask = serde_json::from_str(&json).unwrap();
        assert_eq!(task, parsed);
    }

    #[test]
    fn test_predefined_task_insert_info() {
        let task = PredefinedTask::InsertInfo {
            values: vec![
                InsertValue {
                    field: "company_name".to_string(),
                    value: "Acme Corp".to_string(),
                    location: Some("header".to_string()),
                },
                InsertValue {
                    field: "date".to_string(),
                    value: "2024-01-15".to_string(),
                    location: None,
                },
            ],
        };

        let json = serde_json::to_string(&task).unwrap();
        let parsed: PredefinedTask = serde_json::from_str(&json).unwrap();
        assert_eq!(task, parsed);
    }

    #[test]
    fn test_predefined_task_generate_info() {
        let task = PredefinedTask::GenerateInfo {
            info_type: GenerateInfoType::ExecutiveSummary,
        };

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("generate_info"));
        assert!(json.contains("executive_summary"));
    }

    #[test]
    fn test_split_strategy_serialization() {
        let strategies = vec![
            SplitStrategy::Pages { pages_per_file: 5 },
            SplitStrategy::Sections,
            SplitStrategy::Headings { level: 2 },
            SplitStrategy::Size {
                max_bytes: 1024 * 1024,
            },
            SplitStrategy::AtPages {
                page_numbers: vec![5, 10, 15],
            },
        ];

        for strategy in strategies {
            let json = serde_json::to_string(&strategy).unwrap();
            let parsed: SplitStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(strategy, parsed);
        }
    }
}
