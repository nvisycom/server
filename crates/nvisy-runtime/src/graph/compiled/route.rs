//! Compiled routing node types.

use nvisy_dal::AnyDataValue;

use crate::graph::definition::{ContentTypeCategory, SwitchBranch, SwitchCondition};

/// Compiled switch node - ready to route data.
///
/// Evaluates conditions against input data and determines
/// which branch to route the data to.
#[derive(Debug, Clone)]
pub struct CompiledSwitch {
    /// Branches with conditions and targets.
    branches: Vec<SwitchBranch>,
    /// Default target if no condition matches.
    default: Option<String>,
}

impl CompiledSwitch {
    /// Creates a new compiled switch from branches and default target.
    pub fn new(branches: Vec<SwitchBranch>, default: Option<String>) -> Self {
        Self { branches, default }
    }

    /// Returns the branches.
    pub fn branches(&self) -> &[SwitchBranch] {
        &self.branches
    }

    /// Returns the default target.
    pub fn default(&self) -> Option<&str> {
        self.default.as_deref()
    }

    /// Evaluates the switch conditions against input data.
    ///
    /// Returns the target slot name for routing, or None if no match
    /// and no default is configured.
    pub fn evaluate(&self, data: &AnyDataValue) -> Option<&str> {
        for branch in &self.branches {
            if self.evaluate_condition(&branch.condition, data) {
                return Some(&branch.target);
            }
        }
        self.default.as_deref()
    }

    /// Evaluates a single condition against the data.
    fn evaluate_condition(&self, condition: &SwitchCondition, data: &AnyDataValue) -> bool {
        match condition {
            SwitchCondition::Always => true,
            SwitchCondition::ContentType { category } => {
                // Check if data matches the content type category
                match data {
                    AnyDataValue::Blob(blob) => {
                        let mime = blob
                            .content_type
                            .as_deref()
                            .unwrap_or("application/octet-stream");
                        match category {
                            ContentTypeCategory::Image => mime.starts_with("image/"),
                            ContentTypeCategory::Document => {
                                mime == "application/pdf"
                                    || mime.starts_with("application/vnd.")
                                    || mime == "application/msword"
                            }
                            ContentTypeCategory::Text => {
                                mime.starts_with("text/") || mime == "application/json"
                            }
                            ContentTypeCategory::Audio => mime.starts_with("audio/"),
                            ContentTypeCategory::Video => mime.starts_with("video/"),
                            ContentTypeCategory::Spreadsheet => {
                                mime == "application/vnd.ms-excel"
                                    || mime.contains("spreadsheet")
                                    || mime == "text/csv"
                            }
                            ContentTypeCategory::Presentation => {
                                mime == "application/vnd.ms-powerpoint"
                                    || mime.contains("presentation")
                            }
                            ContentTypeCategory::Archive => {
                                mime == "application/zip"
                                    || mime == "application/x-tar"
                                    || mime == "application/gzip"
                            }
                            ContentTypeCategory::Code => {
                                mime.starts_with("text/x-")
                                    || mime == "application/javascript"
                                    || mime == "application/typescript"
                            }
                        }
                    }
                    _ => false,
                }
            }
            SwitchCondition::FileSizeAbove { threshold_bytes } => match data {
                AnyDataValue::Blob(blob) => blob.data.len() as u64 > *threshold_bytes,
                _ => false,
            },
            SwitchCondition::FileSizeBelow { threshold_bytes } => match data {
                AnyDataValue::Blob(blob) => (blob.data.len() as u64) < *threshold_bytes,
                _ => false,
            },
            SwitchCondition::HasMetadata { key } => {
                // Check if the data has metadata with the given key
                match data {
                    AnyDataValue::Blob(blob) => blob.metadata.contains_key(key),
                    AnyDataValue::Record(record) => record.columns.contains_key(key),
                    _ => false,
                }
            }
            SwitchCondition::MetadataEquals { key, value } => {
                // Check if metadata key equals value
                match data {
                    AnyDataValue::Blob(blob) => {
                        blob.metadata.get(key).map(|v| v == value).unwrap_or(false)
                    }
                    _ => false,
                }
            }
            // TODO: Implement remaining conditions
            SwitchCondition::PageCountAbove { .. } => false,
            SwitchCondition::DurationAbove { .. } => false,
            SwitchCondition::Language { .. } => false,
            SwitchCondition::DateNewerThan { .. } => false,
            SwitchCondition::FileNameMatches { pattern } => match data {
                AnyDataValue::Blob(blob) => {
                    // Simple glob-style matching for common patterns
                    glob_match(pattern, &blob.path)
                }
                _ => false,
            },
            SwitchCondition::FileExtension { extension } => match data {
                AnyDataValue::Blob(blob) => blob
                    .path
                    .rsplit('.')
                    .next()
                    .map(|ext| ext.eq_ignore_ascii_case(extension))
                    .unwrap_or(false),
                _ => false,
            },
        }
    }
}

impl From<crate::graph::definition::SwitchDef> for CompiledSwitch {
    fn from(def: crate::graph::definition::SwitchDef) -> Self {
        Self::new(def.branches, def.default)
    }
}

/// Simple glob-style pattern matching.
///
/// Supports:
/// - `*` matches any sequence of characters (except path separators)
/// - `?` matches any single character
/// - Literal matching for other characters
fn glob_match(pattern: &str, text: &str) -> bool {
    let mut pattern_chars = pattern.chars().peekable();
    let mut text_chars = text.chars().peekable();

    while let Some(p) = pattern_chars.next() {
        match p {
            '*' => {
                // Try matching zero or more characters
                if pattern_chars.peek().is_none() {
                    // Pattern ends with *, matches everything remaining
                    return true;
                }
                // Try each position in the remaining text
                loop {
                    let remaining_pattern: String = pattern_chars.clone().collect();
                    let remaining_text: String = text_chars.clone().collect();
                    if glob_match(&remaining_pattern, &remaining_text) {
                        return true;
                    }
                    if text_chars.next().is_none() {
                        return false;
                    }
                }
            }
            '?' => {
                // Match any single character
                if text_chars.next().is_none() {
                    return false;
                }
            }
            c => {
                // Literal match (case-insensitive for file matching)
                match text_chars.next() {
                    Some(t) if c.eq_ignore_ascii_case(&t) => {}
                    _ => return false,
                }
            }
        }
    }

    // Pattern is exhausted, text should also be exhausted
    text_chars.peek().is_none()
}
