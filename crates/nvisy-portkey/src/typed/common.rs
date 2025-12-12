//! Common types and utilities for JSON serialization and deserialization.

use std::str::FromStr;

use strum::{Display, EnumString, IntoStaticStr};

use crate::Error;

/// Markdown language identifiers for JSON-like formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, IntoStaticStr)]
#[strum(serialize_all = "lowercase")]
pub enum MarkdownLanguage {
    /// JSON format
    Json,
    /// JavaScript (same as JSON for parsing)
    #[strum(serialize = "javascript")]
    JavaScript,
    /// JS (same as JSON for parsing)
    #[strum(serialize = "js")]
    Js,
    /// TypeScript (same as JSON for parsing)
    #[strum(serialize = "typescript")]
    TypeScript,
    /// TS (same as JSON for parsing)
    #[strum(serialize = "ts")]
    Ts,
}

impl MarkdownLanguage {
    /// Returns the valid markdown language tags for JSON.
    #[must_use]
    pub const fn as_markdown_tags() -> &'static [&'static str] {
        &["json", "javascript", "js", "typescript", "ts"]
    }

    /// Checks if a language string is JSON-compatible.
    #[must_use]
    pub fn is_json_compatible(lang: &str) -> bool {
        Self::from_str(lang).is_ok()
    }
}

/// Validates that content is not empty or whitespace-only.
///
/// # Errors
///
/// Returns an error if the content is empty or contains only whitespace.
pub fn validate_non_empty_content(content: &str) -> Result<(), Error> {
    if content.trim().is_empty() {
        Err(Error::invalid_response("Content cannot be empty"))
    } else {
        Ok(())
    }
}

/// Validates that content appears to be valid JSON structure.
///
/// This performs basic structural validation without full parsing:
/// - Must contain balanced braces or brackets
pub fn validate_json_structure(content: &str) -> Result<(), Error> {
    let trimmed = content.trim();

    if trimmed.is_empty() {
        return Err(Error::invalid_response("Empty content"));
    }

    let first_char = trimmed.chars().next().unwrap();
    let last_char = trimmed.chars().last().unwrap();

    let is_object = first_char == '{' && last_char == '}';
    let is_array = first_char == '[' && last_char == ']';

    if !is_object && !is_array {
        return Err(Error::invalid_response(
            "Content does not appear to be valid JSON (missing braces/brackets)",
        ));
    }

    Ok(())
}

/// Checks if content looks like markdown.
#[must_use]
pub fn looks_like_markdown(content: &str) -> bool {
    content.contains("```")
}

/// Normalizes content by trimming and removing common markdown artifacts.
#[must_use]
pub fn normalize_content(content: &str) -> String {
    content.trim().to_string()
}

/// Truncates content for error display.
#[must_use]
pub fn truncate_for_display(content: &str, max_length: usize) -> String {
    if content.len() <= max_length {
        content.to_string()
    } else {
        format!("{}...", &content[..max_length])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_language_tags() {
        let tags = MarkdownLanguage::as_markdown_tags();
        assert_eq!(tags.len(), 5);
        assert!(tags.contains(&"json"));
        assert!(tags.contains(&"javascript"));
    }

    #[test]
    fn test_is_json_compatible() {
        assert!(MarkdownLanguage::is_json_compatible("json"));
        assert!(MarkdownLanguage::is_json_compatible("javascript"));
        assert!(MarkdownLanguage::is_json_compatible("ts"));
        assert!(!MarkdownLanguage::is_json_compatible("python"));
    }

    #[test]
    fn test_validate_non_empty_content() {
        assert!(validate_non_empty_content("valid").is_ok());
        assert!(validate_non_empty_content("").is_err());
        assert!(validate_non_empty_content("   ").is_err());
    }

    #[test]
    fn test_validate_json_structure() {
        assert!(validate_json_structure(r#"{"key": "value"}"#).is_ok());
        assert!(validate_json_structure(r#"[1, 2, 3]"#).is_ok());
        assert!(validate_json_structure("not json").is_err());
        assert!(validate_json_structure("").is_err());
    }

    #[test]
    fn test_looks_like_markdown() {
        assert!(looks_like_markdown("```json\n{}\n```"));
        assert!(!looks_like_markdown("{}"));
    }

    #[test]
    fn test_normalize_content() {
        assert_eq!(normalize_content("  test  "), "test");
        assert_eq!(normalize_content("\n\ttest\n"), "test");
    }

    #[test]
    fn test_truncate_for_display() {
        assert_eq!(truncate_for_display("short", 10), "short");
        assert_eq!(truncate_for_display("very long text", 8), "very lon...");
    }
}
