//! JSON deserialization utilities for parsing LLM responses.
//!
//! This module provides robust JSON parsing that can handle:
//! - Plain JSON strings
//! - JSON embedded in markdown code blocks
//! - Mixed content with JSON extraction

use serde::de::DeserializeOwned;

use super::common::{looks_like_markdown, normalize_content, validate_non_empty_content};
use super::deserialization_utils::create_parsing_error;
use crate::{Error, Result};

/// Result of extracting content from markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownExtractResult {
    /// The extracted content
    pub content: String,
    /// The detected language tag (if any)
    pub language: Option<String>,
}

impl MarkdownExtractResult {
    /// Attempts to extract JSON from markdown code blocks.
    ///
    /// # Errors
    ///
    /// Returns an error if no valid markdown code block is found.
    pub fn from_markdown(markdown: &str) -> Result<Self> {
        validate_non_empty_content(markdown)?;

        // Find code block patterns
        if let Some(start_idx) = markdown.find("```") {
            let after_backticks = &markdown[start_idx + 3..];

            // Extract language tag
            let (language, content_start) = if let Some(newline_idx) = after_backticks.find('\n') {
                let lang = after_backticks[..newline_idx].trim();
                let language = if lang.is_empty() {
                    None
                } else {
                    Some(lang.to_string())
                };
                (language, start_idx + 3 + newline_idx + 1)
            } else {
                (None, start_idx + 3)
            };

            // Find end of code block
            if let Some(end_idx) = markdown[content_start..].find("```") {
                let content = markdown[content_start..content_start + end_idx]
                    .trim()
                    .to_string();

                if content.is_empty() {
                    return Err(Error::invalid_response("Empty code block in markdown"));
                }

                return Ok(Self { content, language });
            }
        }

        Err(Error::invalid_response(
            "No valid markdown code block found",
        ))
    }
}

/// Extracts JSON object from text that may contain additional content.
///
/// This attempts to find and extract a JSON object even if surrounded by other text.
///
/// # Errors
///
/// Returns an error if no valid JSON object can be found.
pub fn extract_json_object(content: &str) -> Result<String> {
    validate_non_empty_content(content)?;

    let trimmed = content.trim();

    // Find the first { and matching }
    if let Some(start) = trimmed.find('{') {
        let mut depth = 0;
        let mut in_string = false;
        let mut escape_next = false;

        for (i, ch) in trimmed[start..].char_indices() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match ch {
                '\\' if in_string => escape_next = true,
                '"' => in_string = !in_string,
                '{' if !in_string => depth += 1,
                '}' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(trimmed[start..start + i + 1].to_string());
                    }
                }
                _ => {}
            }
        }
    }

    Err(Error::invalid_response(
        "No valid JSON object found in content",
    ))
}

/// Extracts content from various response formats.
///
/// Attempts to extract JSON in this order:
/// 1. From markdown code blocks
/// 2. Directly as JSON
/// 3. By finding JSON object in mixed content
///
/// # Errors
///
/// Returns an error if no valid content can be extracted.
pub fn extract_response_content(response: &str) -> Result<String> {
    validate_non_empty_content(response)?;

    // Try markdown extraction first if it looks like markdown
    if looks_like_markdown(response)
        && let Ok(result) = MarkdownExtractResult::from_markdown(response)
    {
        return Ok(result.content);
    }

    // Try direct JSON
    let normalized = normalize_content(response);
    if normalized.starts_with('{') || normalized.starts_with('[') {
        return Ok(normalized);
    }

    // Try extracting JSON object from mixed content
    extract_json_object(response)
}

/// Parses a JSON response into a typed structure.
///
/// # Errors
///
/// Returns an error if the JSON cannot be parsed or deserialized.
///
/// # Examples
///
/// ```rust
/// use nvisy_portkey::typed::parse_json_response;
/// use serde::{Deserialize, Serialize};
/// use schemars::JsonSchema;
///
/// #[derive(Serialize, Deserialize, JsonSchema)]
/// struct User {
///     id: u32,
///     name: String,
/// }
///
/// let json = r#"{"id": 123, "name": "Alice"}"#;
/// let user: User = parse_json_response(json).unwrap();
/// assert_eq!(user.id, 123);
/// ```
pub fn parse_json_response<T: DeserializeOwned>(json: &str) -> Result<T> {
    validate_non_empty_content(json)?;

    serde_json::from_str(json)
        .map_err(|e| create_parsing_error(&format!("JSON parsing failed: {e}"), json))
}

/// Parses response content with automatic extraction and JSON parsing.
///
/// This is the main entry point for parsing LLM responses. It handles:
/// - Extracting JSON from markdown code blocks
/// - Parsing plain JSON
/// - Finding JSON in mixed content
///
/// # Errors
///
/// Returns an error if extraction or parsing fails.
///
/// # Examples
///
/// ```rust
/// use nvisy_portkey::typed::parse_response_content;
/// use serde::{Deserialize, Serialize};
/// use schemars::JsonSchema;
///
/// #[derive(Serialize, Deserialize, JsonSchema)]
/// struct Data {
///     value: i32,
/// }
///
/// // Works with plain JSON
/// let json = r#"{"value": 42}"#;
/// let data: Data = parse_response_content(json).unwrap();
///
/// // Works with markdown
/// let markdown = "```json\n{\"value\": 42}\n```";
/// let data: Data = parse_response_content(markdown).unwrap();
/// ```
pub fn parse_response_content<T: DeserializeOwned>(response: &str) -> Result<T> {
    let content = extract_response_content(response)?;
    parse_json_response(&content)
}

#[cfg(test)]
mod tests {
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
    struct TestData {
        id: u32,
        name: String,
    }

    #[test]
    fn test_markdown_extract_with_language() {
        let markdown = "```json\n{\"id\": 1, \"name\": \"test\"}\n```";
        let result = MarkdownExtractResult::from_markdown(markdown).unwrap();
        assert_eq!(result.language, Some("json".to_string()));
        assert!(result.content.contains("test"));
    }

    #[test]
    fn test_markdown_extract_without_language() {
        let markdown = "```\n{\"id\": 1}\n```";
        let result = MarkdownExtractResult::from_markdown(markdown).unwrap();
        assert_eq!(result.language, None);
    }

    #[test]
    fn test_extract_json_object() {
        let mixed = "Here is the data: {\"id\": 42, \"name\": \"test\"} and more text";
        let json = extract_json_object(mixed).unwrap();
        assert_eq!(json, r#"{"id": 42, "name": "test"}"#);
    }

    #[test]
    fn test_extract_json_object_with_nested() {
        let nested = r#"{"outer": {"inner": "value"}, "other": 123}"#;
        let json = extract_json_object(nested).unwrap();
        assert!(json.contains("inner"));
        assert!(json.contains("outer"));
    }

    #[test]
    fn test_extract_response_content_markdown() {
        let markdown = "```json\n{\"id\": 1}\n```";
        let content = extract_response_content(markdown).unwrap();
        assert!(content.contains("id"));
    }

    #[test]
    fn test_extract_response_content_plain() {
        let json = r#"{"id": 1}"#;
        let content = extract_response_content(json).unwrap();
        assert_eq!(content, json);
    }

    #[test]
    fn test_parse_json_response() {
        let json = r#"{"id": 42, "name": "test"}"#;
        let data: TestData = parse_json_response(json).unwrap();
        assert_eq!(data.id, 42);
        assert_eq!(data.name, "test");
    }

    #[test]
    fn test_parse_response_content_with_markdown() {
        let markdown = "```json\n{\"id\": 123, \"name\": \"Alice\"}\n```";
        let data: TestData = parse_response_content(markdown).unwrap();
        assert_eq!(data.id, 123);
        assert_eq!(data.name, "Alice");
    }

    #[test]
    fn test_parse_response_content_with_mixed() {
        let mixed = "The result is: {\"id\": 999, \"name\": \"Bob\"}";
        let data: TestData = parse_response_content(mixed).unwrap();
        assert_eq!(data.id, 999);
        assert_eq!(data.name, "Bob");
    }

    #[test]
    fn test_empty_response() {
        let result: Result<TestData> = parse_response_content("");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_json() {
        let result: Result<TestData> = parse_response_content("{invalid}");
        assert!(result.is_err());
    }
}
