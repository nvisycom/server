//! Deserialization functionality for typed responses with format detection and extraction.

use openrouter_rs::types::completion::CompletionsResponse;
use serde::de::DeserializeOwned;

use super::common::{MarkdownLanguage, ParseConfig, SerializationFormat};
use super::deserialization_utils::get_format_sequence;
use crate::Error;

/// Result of markdown content extraction with format detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownExtractResult {
    /// The extracted content
    pub content: String,
    /// The detected format (if any)
    pub detected_format: Option<SerializationFormat>,
}

impl MarkdownExtractResult {
    /// Creates a new extraction result.
    #[must_use]
    pub const fn new(content: String, detected_format: Option<SerializationFormat>) -> Self {
        Self {
            content,
            detected_format,
        }
    }

    /// Extracts content from markdown code blocks with format detection.
    ///
    /// This method looks for markdown code blocks (```language) and extracts both
    /// the content and the detected serialization format based on the language tag.
    ///
    /// # Errors
    ///
    /// Returns an error if no markdown code blocks are found in the input.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use nvisy_openrouter::typed::deserialization::MarkdownExtractResult;
    /// let input = "```json\n{\"key\": \"value\"}\n```";
    /// let result = MarkdownExtractResult::from_markdown(input).unwrap();
    /// assert_eq!(result.content, r#"{"key": "value"}"#);
    /// ```
    pub fn from_markdown(input: &str) -> Result<Self, Error> {
        let mut lines = input.lines();

        while let Some(line) = lines.next() {
            let trimmed = line.trim();
            if trimmed.starts_with("```") {
                let lang = trimmed.trim_start_matches("```").trim().to_lowercase();

                // Detect format from markdown language tag
                let detected_format = if lang.is_empty() {
                    None
                } else {
                    MarkdownLanguage::detect_format(&lang)
                };

                let mut content = String::new();
                for content_line in lines.by_ref() {
                    if content_line.trim() == "```" {
                        let result = content.trim().to_string();
                        if !result.is_empty() {
                            return Ok(Self::new(result, detected_format));
                        }
                        break;
                    }
                    if !content.is_empty() {
                        content.push('\n');
                    }
                    content.push_str(content_line);
                }
            }
        }

        Err(Error::invalid_response("No markdown code blocks found"))
    }
}

/// Extracts a JSON object from text using balanced brace counting.
///
/// Finds the first complete JSON object by properly matching braces and handling
/// string escapes to avoid false matches within string literals.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::deserialization::extract_json_object;
/// let input = r#"Here's the data: {"key": "value", "nested": {"inner": "data"}} more text"#;
/// let result = extract_json_object(input).unwrap();
/// assert!(result.contains("key"));
/// ```
#[must_use]
pub fn extract_json_object(input: &str) -> Option<String> {
    let start_pos = input.find('{')?;
    let mut brace_count = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, ch) in input[start_pos..].char_indices() {
        let actual_pos = start_pos + i;

        if escape_next {
            escape_next = false;
            continue;
        }

        match ch {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => {
                brace_count += 1;
            }
            '}' if !in_string => {
                brace_count -= 1;
                if brace_count == 0 {
                    return Some(input[start_pos..=actual_pos].to_string());
                }
            }
            _ => {}
        }
    }

    None
}

/// Extracts TOON data using pattern recognition.
///
/// TOON format supports several patterns:
/// - Simple key-value pairs: `key: value`
/// - TOON object format: `{fields}: data`
/// - TOON array format: `[number]{fields}: data`
///
/// This function looks for these patterns and extracts the relevant content.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::deserialization::extract_toon_data;
/// let input = "id: 123\nname: Alice\nactive: true";
/// let result = extract_toon_data(input).unwrap();
/// assert!(result.contains("id: 123"));
/// ```
#[must_use]
pub fn extract_toon_data(input: &str) -> Option<String> {
    // Look for TOON array pattern: [number]{fields}: followed by data
    if let Some(start) = input.find('[')
        && let Some(bracket_end) = input[start..].find(']')
    {
        let bracket_end = start + bracket_end;
        if let Some(brace_start) = input[bracket_end..].find('{') {
            let brace_start = bracket_end + brace_start;
            if let Some(brace_end) = input[brace_start..].find('}') {
                let brace_end = brace_start + brace_end;
                if let Some(colon_pos) = input[brace_end..].find(':') {
                    let colon_pos = brace_end + colon_pos;
                    let end_pos = find_toon_data_end(input, colon_pos + 1);
                    return Some(input[start..end_pos].trim().to_string());
                }
            }
        }
    }

    // Look for simple TOON object pattern: {fields}: followed by data
    if let Some(start) = input.find('{')
        && let Some(brace_end) = input[start..].find('}')
    {
        let brace_end = start + brace_end;
        if let Some(colon_pos) = input[brace_end..].find(':') {
            let colon_pos = brace_end + colon_pos;
            let end_pos = find_simple_toon_data_end(input, colon_pos + 1);
            return Some(input[start..end_pos].trim().to_string());
        }
    }

    // Look for TOON-like key-value content (key: value patterns)
    let lines: Vec<&str> = input
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty()
                && trimmed.contains(": ")
                && !trimmed.starts_with("//")
                && !trimmed.starts_with('#')
        })
        .collect();

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

/// Extracts structured content based on the specified format.
///
/// Delegates to the appropriate format-specific extraction function.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::{deserialization::extract_structured_content, common::format::SerializationFormat};
/// let json_input = r#"Here's data: {"key": "value"} done"#;
/// let result = extract_structured_content(json_input, SerializationFormat::Json);
/// assert!(result.is_some());
/// ```
#[must_use]
pub fn extract_structured_content(input: &str, format: SerializationFormat) -> Option<String> {
    match format {
        SerializationFormat::Json => extract_json_object(input),
        SerializationFormat::Toon => extract_toon_data(input),
    }
}

/// Helper function to find the end of TOON data starting from the given position.
///
/// TOON data ends at:
/// - Double newline (\n\n)
/// - Start of a new markdown block (```)
/// - End of content
fn find_toon_data_end(content: &str, data_start: usize) -> usize {
    let remaining = &content[data_start..];

    // Look for double newline
    if let Some(pos) = remaining.find("\n\n") {
        return data_start + pos;
    }

    // Look for markdown block start
    if let Some(pos) = remaining.find("\n```") {
        return data_start + pos;
    }

    // Default to end of content
    content.len()
}

/// Helper function to find the end of simple TOON data (single line format).
///
/// Simple TOON data ends at:
/// - Space followed by a word that doesn't look like TOON content
/// - Newline character
/// - End of content
fn find_simple_toon_data_end(content: &str, data_start: usize) -> usize {
    let remaining = &content[data_start..];
    let trimmed = remaining.trim_start();
    let trimmed_start = remaining.len() - trimmed.len();

    let chars = trimmed.char_indices();
    let mut last_valid_pos = 0;

    for (i, ch) in chars {
        match ch {
            // Valid TOON data characters
            'a'..='z' | 'A'..='Z' | '0'..='9' | ',' | '_' | '-' | '.' | '@' => {
                last_valid_pos = i + ch.len_utf8();
            }
            // Space might indicate end of data
            ' ' => {
                // Check what comes after the space
                let after_space = &trimmed[i + 1..];
                if let Some(next_word) = after_space.split_whitespace().next() {
                    // If the next word doesn't look like TOON data
                    if !next_word.contains(',')
                        && matches!(next_word, "and" | "or" | "done" | "that" | "with" | "more")
                    {
                        break;
                    }
                }
                last_valid_pos = i + ch.len_utf8();
            }
            // Newlines definitely end the data
            '\n' | '\r' => break,
            // Other characters probably end the data
            _ => break,
        }
    }

    data_start + trimmed_start + last_valid_pos
}

/// Extracts the content string from the completion response.
///
/// Iterates through all choices to find the first one with non-empty content.
///
/// # Errors
///
/// Returns an error if no response choices are available or if no content is found.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::deserialization::extract_response_content;
/// # use openrouter_rs::types::completion::CompletionsResponse;
/// // Assuming you have a CompletionsResponse
/// // let content = extract_response_content(&response)?;
/// ```
pub fn extract_response_content(response: &CompletionsResponse) -> Result<String, Error> {
    if response.choices.is_empty() {
        return Err(Error::invalid_response(
            "No response choices returned from LLM",
        ));
    }

    for choice in &response.choices {
        if let Some(content) = choice.content() {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return Ok(content.to_string());
            }
        }
    }

    Err(Error::invalid_response(
        "No content found in any LLM response choice",
    ))
}

/// Parses content with the specified format using the appropriate serializer.
///
/// Uses `serde_json` for JSON format and `serde_toon` for TOON format.
///
/// # Errors
///
/// Returns an error if the content cannot be parsed in the specified format.
fn parse_with_format<T>(content: &str, format: SerializationFormat) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    match format {
        SerializationFormat::Json => {
            serde_json::from_str::<T>(content).map_err(Error::JsonSerialization)
        }
        SerializationFormat::Toon => {
            serde_toon::from_str::<T>(content).map_err(Error::ToonSerialization)
        }
    }
}

/// Attempts to parse content with a specific format, providing detailed error context.
///
/// This is a wrapper around `parse_with_format` that adds additional context to errors.
fn try_parse_with_format<T>(content: &str, format: SerializationFormat) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    parse_with_format(content, format).map_err(|err| {
        Error::invalid_response(format!(
            "Failed to parse {} content: {}",
            format.as_ref().to_uppercase(),
            err
        ))
    })
}

/// Parses a response string into typed data with configurable format support.
///
/// This is the main parsing function that implements intelligent format detection
/// and fallback strategies. The parsing strategy is:
///
/// 1. Try direct parsing with preferred format (if configured)
/// 2. Check for markdown code blocks with automatic format detection
/// 3. Try extracting structured content and parsing with configured formats
///
/// # Errors
///
/// Returns an error if:
/// - The response is empty
/// - No valid content can be extracted in any supported format
/// - Parsing fails in all attempted formats
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::{deserialization::parse_response_content, common::config::ParseConfig};
/// # use serde::{Deserialize, Serialize};
/// # use schemars::JsonSchema;
/// #
/// # #[derive(Serialize, Deserialize, JsonSchema)]
/// # struct MyData { id: u32, name: String }
///
/// let config = ParseConfig::new().with_preferred_format(
///     nvisy_openrouter::typed::common::format::SerializationFormat::Json
/// ).with_fallback(true);
///
/// let response = r#"{"id": 123, "name": "test"}"#;
/// let data: MyData = parse_response_content(response, &config)?;
/// # Ok::<(), nvisy_openrouter::Error>(())
/// ```
pub fn parse_response_content<T>(response: &str, config: &ParseConfig) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    if response.trim().is_empty() {
        return Err(Error::invalid_response("Empty response content"));
    }

    // Strategy 1: Try direct parsing with preferred format
    if let Some(preferred) = config.preferred_format
        && let Ok(data) = try_parse_with_format::<T>(response.trim(), preferred)
    {
        return Ok(data);
    }

    // Strategy 2: Check for markdown code blocks with format detection
    if let Ok(extract_result) = MarkdownExtractResult::from_markdown(response) {
        return parse_from_markdown_extract(extract_result, config);
    }

    // Strategy 3: Try extracting structured content and parsing
    parse_from_structured_extraction(response, config)
}

/// Parses extracted markdown content with intelligent format handling.
fn parse_from_markdown_extract<T>(
    extract_result: MarkdownExtractResult,
    config: &ParseConfig,
) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    // If format was detected from markdown, prefer that
    if let Some(detected) = extract_result.detected_format {
        match try_parse_with_format::<T>(&extract_result.content, detected) {
            Ok(data) => return Ok(data),
            Err(detected_error) => {
                // Try fallback format if enabled
                if config.enable_fallback {
                    let fallback_format = detected.opposite();
                    match try_parse_with_format::<T>(&extract_result.content, fallback_format) {
                        Ok(data) => return Ok(data),
                        Err(fallback_error) => {
                            return Err(Error::invalid_response(format!(
                                "Failed to parse markdown content. Detected {} error: {}. Fallback {} error: {}",
                                detected.as_ref().to_uppercase(),
                                detected_error,
                                fallback_format.as_ref().to_uppercase(),
                                fallback_error
                            )));
                        }
                    }
                } else {
                    return Err(detected_error);
                }
            }
        }
    } else {
        // No format detected in markdown, try preferred format or default to JSON
        let format = config.preferred_or_default();
        if let Ok(data) = try_parse_with_format::<T>(&extract_result.content, format) {
            return Ok(data);
        }
    }

    Err(Error::invalid_response(
        "Failed to parse extracted markdown content",
    ))
}

/// Attempts to parse content by extracting structured data and trying configured formats.
fn parse_from_structured_extraction<T>(response: &str, config: &ParseConfig) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let formats_to_try = get_format_sequence(config);

    for format in formats_to_try {
        if let Some(extracted) = extract_structured_content(response, format)
            && let Ok(data) = try_parse_with_format::<T>(&extracted, format)
        {
            return Ok(data);
        }
    }

    let formats_tried = if config.enable_fallback {
        "JSON, TOON".to_string()
    } else {
        match config.preferred_format {
            Some(f) => f.as_ref().to_string(),
            None => "JSON".to_string(),
        }
    };

    Err(Error::invalid_response(format!(
        "Could not parse response in any supported format. Tried: {}",
        formats_tried
    )))
}

/// Convenience function for parsing response content with JSON-only configuration.
///
/// # Errors
///
/// Returns an error if the content cannot be parsed as JSON.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::deserialization::parse_json_response;
/// # use serde::{Deserialize, Serialize};
/// # use schemars::JsonSchema;
/// #
/// # #[derive(Serialize, Deserialize, JsonSchema)]
/// # struct MyData { id: u32 }
///
/// let json = r#"{"id": 123}"#;
/// let data: MyData = parse_json_response(json)?;
/// # Ok::<(), nvisy_openrouter::Error>(())
/// ```
pub fn parse_json_response<T>(response: &str) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let config = ParseConfig::json_only();
    parse_response_content(response, &config)
}

/// Convenience function for parsing response content with TOON-only configuration.
///
/// # Errors
///
/// Returns an error if the content cannot be parsed as TOON.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::deserialization::parse_toon_response;
/// # use serde::{Deserialize, Serialize};
/// # use schemars::JsonSchema;
/// #
/// # #[derive(Serialize, Deserialize, JsonSchema)]
/// # struct MyData { id: u32 }
///
/// let toon = "id: 123";
/// let data: MyData = parse_toon_response(toon)?;
/// # Ok::<(), nvisy_openrouter::Error>(())
/// ```
pub fn parse_toon_response<T>(response: &str) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let config = ParseConfig::toon_only();
    parse_response_content(response, &config)
}

/// Convenience function for parsing with JSON preferred and TOON fallback.
///
/// # Errors
///
/// Returns an error if the content cannot be parsed in either format.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::deserialization::parse_json_with_toon_fallback;
/// # use serde::{Deserialize, Serialize};
/// # use schemars::JsonSchema;
/// #
/// # #[derive(Serialize, Deserialize, JsonSchema)]
/// # struct MyData { id: u32 }
///
/// // This will try JSON first, then TOON
/// let response = "id: 123"; // TOON format
/// let data: MyData = parse_json_with_toon_fallback(response)?;
/// # Ok::<(), nvisy_openrouter::Error>(())
/// ```
pub fn parse_json_with_toon_fallback<T>(response: &str) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let config = ParseConfig::json_with_toon_fallback();
    parse_response_content(response, &config)
}

/// Convenience function for parsing with TOON preferred and JSON fallback.
///
/// # Errors
///
/// Returns an error if the content cannot be parsed in either format.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::deserialization::parse_toon_with_json_fallback;
/// # use serde::{Deserialize, Serialize};
/// # use schemars::JsonSchema;
/// #
/// # #[derive(Serialize, Deserialize, JsonSchema)]
/// # struct MyData { id: u32 }
///
/// // This will try TOON first, then JSON
/// let response = r#"{"id": 123}"#; // JSON format
/// let data: MyData = parse_toon_with_json_fallback(response)?;
/// # Ok::<(), nvisy_openrouter::Error>(())
/// ```
pub fn parse_toon_with_json_fallback<T>(response: &str) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let config = ParseConfig::toon_with_json_fallback();
    parse_response_content(response, &config)
}

#[cfg(test)]
mod tests {
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
    struct TestData {
        id: u32,
        name: String,
        active: bool,
    }

    #[test]
    fn test_markdown_extract_result_from_markdown() {
        let input = "```json\n{\"test\": \"value\"}\n```";
        let result = MarkdownExtractResult::from_markdown(input).unwrap();
        assert_eq!(result.content, r#"{"test": "value"}"#);
        assert_eq!(result.detected_format, Some(SerializationFormat::Json));

        let input_ts = "```typescript\n{\"test\": \"value\"}\n```";
        let result_ts = MarkdownExtractResult::from_markdown(input_ts).unwrap();
        assert_eq!(result_ts.content, r#"{"test": "value"}"#);
        assert_eq!(result_ts.detected_format, Some(SerializationFormat::Json));

        let input_no_lang = "```\n{\"test\": \"value\"}\n```";
        let result_no_lang = MarkdownExtractResult::from_markdown(input_no_lang).unwrap();
        assert_eq!(result_no_lang.content, r#"{"test": "value"}"#);
        assert_eq!(result_no_lang.detected_format, None);
    }

    #[test]
    fn test_markdown_extract_result_no_blocks() {
        let input = "This is just plain text with no code blocks";
        let result = MarkdownExtractResult::from_markdown(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_json_object() {
        let input = r#"Here's the response: {"test": "value", "number": 42} and that's it."#;
        let result = extract_json_object(input).unwrap();
        assert!(result.contains("test"));
        assert!(result.contains("value"));
        assert!(result.contains("42"));

        // Test nested objects
        let nested_input = r#"{"outer": {"inner": {"deep": "value"}}, "simple": "test"}"#;
        let nested_result = extract_json_object(nested_input).unwrap();
        assert!(nested_result.contains("outer"));
        assert!(nested_result.contains("inner"));
        assert!(nested_result.contains("deep"));

        // Test no JSON
        let no_json = "No JSON here at all";
        assert_eq!(extract_json_object(no_json), None);
    }

    #[test]
    fn test_extract_toon_data() {
        // Simple key-value pairs
        let input = "id: 123\nname: Alice\nactive: true";
        let result = extract_toon_data(input).unwrap();
        assert!(result.contains("id: 123"));
        assert!(result.contains("name: Alice"));
        assert!(result.contains("active: true"));

        // TOON object format
        let toon_object = "Response: {id,name}: 123,Alice and that's it";
        let result = extract_toon_data(toon_object).unwrap();
        assert!(result.contains("{id,name}"));

        // No TOON data
        let no_toon = "This is just regular text without colons";
        assert_eq!(extract_toon_data(no_toon), None);
    }

    #[test]
    fn test_extract_structured_content() {
        // JSON format
        let json_result = extract_structured_content(
            "Here's some JSON: {\"key\": \"value\"} done",
            SerializationFormat::Json,
        );
        assert!(json_result.is_some());
        assert!(json_result.unwrap().contains("key"));

        // TOON format
        let toon_result =
            extract_structured_content("id: 123\nname: Alice", SerializationFormat::Toon);
        assert!(toon_result.is_some());
        assert!(toon_result.unwrap().contains("id: 123"));
    }

    #[test]
    fn test_parse_json_response() {
        let json = r#"{"id": 1, "name": "Test", "active": true}"#;
        let user: TestData = parse_json_response(json).unwrap();
        assert_eq!(user.id, 1);
        assert_eq!(user.name, "Test");
    }

    #[test]
    fn test_parse_toon_response() {
        let toon = "id: 2\nname: Toon\nactive: false";
        let user: TestData = parse_toon_response(toon).unwrap();
        assert_eq!(user.id, 2);
        assert_eq!(user.name, "Toon");
    }

    #[test]
    fn test_parse_json_from_markdown() {
        let markdown = "```json\n{\"id\": 3, \"name\": \"Markdown\", \"active\": true}\n```";
        let config = ParseConfig::new();
        let user: TestData = parse_response_content(markdown, &config).unwrap();
        assert_eq!(user.id, 3);
        assert_eq!(user.name, "Markdown");
    }

    #[test]
    fn test_parse_json_from_text() {
        let text = r#"Here is the data: {"id": 4, "name": "Text", "active": false} end"#;
        let config = ParseConfig::new().with_fallback(true);
        let user: TestData = parse_response_content(text, &config).unwrap();
        assert_eq!(user.id, 4);
        assert_eq!(user.name, "Text");
    }

    #[test]
    fn test_fallback_json_to_toon() {
        let toon_data = "id: 5\nname: Fallback\nactive: true";
        let config = ParseConfig::json_with_toon_fallback();
        let user: TestData = parse_response_content(toon_data, &config).unwrap();
        assert_eq!(user.id, 5);
        assert_eq!(user.name, "Fallback");
    }

    #[test]
    fn test_fallback_toon_to_json() {
        let json_data = r#"{"id": 6, "name": "JsonFallback", "active": false}"#;
        let config = ParseConfig::toon_with_json_fallback();
        let user: TestData = parse_response_content(json_data, &config).unwrap();
        assert_eq!(user.id, 6);
        assert_eq!(user.name, "JsonFallback");
    }

    #[test]
    fn test_markdown_format_detection_overrides_config() {
        // Even though config prefers TOON, markdown detection should override
        let markdown = "```json\n{\"id\": 7, \"name\": \"Override\", \"active\": true}\n```";
        let config = ParseConfig::new()
            .with_preferred_format(SerializationFormat::Toon)
            .with_fallback(false);

        let user: TestData = parse_response_content(markdown, &config).unwrap();
        assert_eq!(user.id, 7);
        assert_eq!(user.name, "Override");
    }

    #[test]
    fn test_new_parsing_behavior_with_fallbacks() {
        // Test that the new parsing behavior works with different fallback scenarios

        // Direct JSON parsing
        let json_data = r#"{"id": 8, "name": "Direct", "active": true}"#;
        let config = ParseConfig::new().with_preferred_format(SerializationFormat::Json);
        let user: TestData = parse_response_content(json_data, &config).unwrap();
        assert_eq!(user.id, 8);

        // JSON embedded in text
        let json_in_text =
            r#"Here's the result: {"id": 9, "name": "Embedded", "active": false} done"#;
        let config = ParseConfig::new().with_fallback(true);
        let user: TestData = parse_response_content(json_in_text, &config).unwrap();
        assert_eq!(user.id, 9);

        // TOON data with JSON fallback
        let toon_data = "id: 10\nname: ToonData\nactive: true";
        let config = ParseConfig::toon_with_json_fallback();
        let user: TestData = parse_response_content(toon_data, &config).unwrap();
        assert_eq!(user.id, 10);
        assert_eq!(user.name, "ToonData");

        // Markdown with explicit format
        let markdown_toon = "```toon\nid: 11\nname: MarkdownToon\nactive: false\n```";
        let config = ParseConfig::json_only(); // Config prefers JSON but markdown should override
        let user: TestData = parse_response_content(markdown_toon, &config).unwrap();
        assert_eq!(user.id, 11);
        assert_eq!(user.name, "MarkdownToon");
    }

    #[test]
    fn test_empty_response() {
        let config = ParseConfig::new();

        let result: Result<TestData, _> = parse_response_content("", &config);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Empty response content")
        );

        let result: Result<TestData, _> = parse_response_content("   \n\t  ", &config);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Empty response content")
        );
    }

    #[test]
    fn test_find_toon_data_end() {
        let content = "start: data here\n\nNext section";
        let end = find_toon_data_end(content, 7); // Start after "start: "
        assert_eq!(&content[7..end], "data here");

        let content_with_markdown = "start: data\n```\ncode block";
        let end = find_toon_data_end(content_with_markdown, 7);
        assert_eq!(&content_with_markdown[7..end], "data");
    }

    #[test]
    fn test_find_simple_toon_data_end() {
        let content = "prefix: value1,value2,value3 and more text";
        let end = find_simple_toon_data_end(content, 8); // Start after "prefix: "
        assert_eq!(&content[8..end], "value1,value2,value3");

        let content_with_newline = "prefix: simple\nNext line";
        let end = find_simple_toon_data_end(content_with_newline, 8);
        assert_eq!(&content_with_newline[8..end], "simple");
    }
}
