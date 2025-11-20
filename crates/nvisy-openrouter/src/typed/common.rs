//! Common types, utilities, and configurations shared across serialization and deserialization.

use std::str::FromStr;

use strum::{AsRefStr, Display, EnumIter, EnumString, IntoStaticStr};

use crate::Error;

// ============================================================================
// Format Types
// ============================================================================

/// Markdown language identifiers that map to serialization formats.
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
    /// TOON format
    Toon,
}

impl MarkdownLanguage {
    /// Converts a markdown language to its corresponding serialization format.
    #[must_use]
    pub const fn to_serialization_format(self) -> SerializationFormat {
        match self {
            Self::Json | Self::JavaScript | Self::Js | Self::TypeScript | Self::Ts => {
                SerializationFormat::Json
            }
            Self::Toon => SerializationFormat::Toon,
        }
    }

    /// Attempts to detect the serialization format from a markdown language string.
    #[must_use]
    pub fn detect_format(lang: &str) -> Option<SerializationFormat> {
        Self::from_str(lang).ok().map(Self::to_serialization_format)
    }
}

/// Supported serialization formats for typed responses.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Display, EnumString, EnumIter, AsRefStr
)]
#[strum(serialize_all = "lowercase")]
pub enum SerializationFormat {
    /// JSON (JavaScript Object Notation) format
    #[default]
    Json,
    /// TOON (Token-Oriented Object Notation) format - more compact for LLMs
    Toon,
}

impl SerializationFormat {
    /// Returns the valid markdown language tags for this format.
    #[must_use]
    pub const fn as_markdown_tags(self) -> &'static [&'static str] {
        match self {
            Self::Json => &["json", "javascript", "js", "typescript", "ts"],
            Self::Toon => &["toon"],
        }
    }

    /// Returns the opposite format for fallback scenarios.
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Json => Self::Toon,
            Self::Toon => Self::Json,
        }
    }
}

// ============================================================================
// Configuration Types
// ============================================================================

/// Configuration for typed parsing operations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParseConfig {
    /// Preferred format to try first (if None, auto-detect from markdown or use JSON)
    pub preferred_format: Option<SerializationFormat>,
    /// Whether to attempt fallback to the other format if primary fails
    pub enable_fallback: bool,
}

impl ParseConfig {
    /// Creates a new ParseConfig with default values.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            preferred_format: None,
            enable_fallback: false,
        }
    }

    /// Creates a ParseConfig with JSON as preferred format and no fallback.
    #[must_use]
    pub const fn json_only() -> Self {
        Self {
            preferred_format: Some(SerializationFormat::Json),
            enable_fallback: false,
        }
    }

    /// Creates a ParseConfig with TOON as preferred format and no fallback.
    #[must_use]
    pub const fn toon_only() -> Self {
        Self {
            preferred_format: Some(SerializationFormat::Toon),
            enable_fallback: false,
        }
    }

    /// Creates a ParseConfig with JSON preferred and TOON fallback.
    #[must_use]
    pub const fn json_with_toon_fallback() -> Self {
        Self {
            preferred_format: Some(SerializationFormat::Json),
            enable_fallback: true,
        }
    }

    /// Creates a ParseConfig with TOON preferred and JSON fallback.
    #[must_use]
    pub const fn toon_with_json_fallback() -> Self {
        Self {
            preferred_format: Some(SerializationFormat::Toon),
            enable_fallback: true,
        }
    }

    /// Sets the preferred format.
    #[must_use]
    pub const fn with_preferred_format(mut self, format: SerializationFormat) -> Self {
        self.preferred_format = Some(format);
        self
    }

    /// Sets whether fallback is enabled.
    #[must_use]
    pub const fn with_fallback(mut self, enable: bool) -> Self {
        self.enable_fallback = enable;
        self
    }

    /// Gets the fallback format (the opposite of preferred).
    #[must_use]
    pub const fn fallback_format(&self) -> Option<SerializationFormat> {
        match self.preferred_format {
            Some(format) => Some(format.opposite()),
            None => None,
        }
    }

    /// Returns the preferred format or JSON as default.
    #[must_use]
    pub const fn preferred_or_default(&self) -> SerializationFormat {
        match self.preferred_format {
            Some(format) => format,
            None => SerializationFormat::Json,
        }
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Validates that content is not empty or whitespace-only.
///
/// # Errors
///
/// Returns an error if the content is empty or contains only whitespace.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::common::validate_non_empty_content;
/// assert!(validate_non_empty_content("valid content").is_ok());
/// assert!(validate_non_empty_content("").is_err());
/// assert!(validate_non_empty_content("   \n\t   ").is_err());
/// ```
pub fn validate_non_empty_content(content: &str) -> Result<(), Error> {
    if content.trim().is_empty() {
        Err(Error::invalid_response("Content cannot be empty"))
    } else {
        Ok(())
    }
}

/// Validates that content appears to be in the expected format.
///
/// This performs basic structural validation without full parsing:
/// - JSON: Must contain balanced braces
/// - TOON: Must contain key-value patterns
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::common::{validate_content_format, SerializationFormat};
/// assert!(validate_content_format(r#"{"key": "value"}"#, SerializationFormat::Json).is_ok());
/// assert!(validate_content_format("key: value", SerializationFormat::Toon).is_ok());
/// assert!(validate_content_format("invalid", SerializationFormat::Json).is_err());
/// ```
pub fn validate_content_format(content: &str, format: SerializationFormat) -> Result<(), Error> {
    validate_non_empty_content(content)?;

    match format {
        SerializationFormat::Json => validate_json_structure(content),
        SerializationFormat::Toon => validate_toon_structure(content),
    }
}

/// Validates basic JSON structure by checking for balanced braces.
fn validate_json_structure(content: &str) -> Result<(), Error> {
    let trimmed = content.trim();

    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return Err(Error::invalid_response(
            "JSON content must be wrapped in curly braces",
        ));
    }

    // Basic brace balance check
    let mut brace_count = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for ch in trimmed.chars() {
        if escape_next {
            escape_next = false;
            continue;
        }

        match ch {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => brace_count += 1,
            '}' if !in_string => brace_count -= 1,
            _ => {}
        }
    }

    if brace_count != 0 {
        Err(Error::invalid_response("JSON has unbalanced braces"))
    } else {
        Ok(())
    }
}

/// Validates basic TOON structure by checking for key-value patterns.
fn validate_toon_structure(content: &str) -> Result<(), Error> {
    let trimmed = content.trim();

    // Check for at least one key-value pair pattern
    let has_key_value = trimmed.lines().any(|line| {
        let line = line.trim();
        !line.is_empty() && line.contains(": ") && !line.starts_with("//") && !line.starts_with('#')
    });

    if !has_key_value {
        Err(Error::invalid_response(
            "TOON content must contain at least one key-value pair (key: value)",
        ))
    } else {
        Ok(())
    }
}

/// Normalizes whitespace in content for consistent processing.
///
/// - Trims leading and trailing whitespace
/// - Normalizes line endings to \n
/// - Removes excessive whitespace while preserving structure
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::common::normalize_content;
/// let input = "  \r\n  key: value  \r\n  \r\n  other: data  \r\n  ";
/// let normalized = normalize_content(input);
/// assert_eq!(normalized, "key: value\n\nother: data");
/// ```
#[must_use]
pub fn normalize_content(content: &str) -> String {
    content
        .trim()
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end()
        .to_string()
}

/// Truncates content for error messages and logging.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::common::truncate_for_display;
/// let long_content = "a".repeat(200);
/// let truncated = truncate_for_display(&long_content, 50);
/// assert!(truncated.len() <= 53); // 50 + "..."
/// ```
#[must_use]
pub fn truncate_for_display(content: &str, max_length: usize) -> String {
    if content.len() <= max_length {
        content.to_string()
    } else {
        format!("{}...", &content[..max_length])
    }
}

/// Checks if content looks like it might be wrapped in markdown code blocks.
///
/// This is a quick heuristic check before doing full markdown parsing.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::common::looks_like_markdown;
/// assert!(looks_like_markdown("```json\n{\"key\": \"value\"}\n```"));
/// assert!(looks_like_markdown("Some text\n```\ncontent\n```\nmore text"));
/// assert!(!looks_like_markdown("just plain text"));
/// ```
#[must_use]
pub fn looks_like_markdown(content: &str) -> bool {
    content.contains("```")
}

/// Estimates the serialization format based on content heuristics.
///
/// This provides a best-guess estimate without full parsing:
/// - JSON: Starts and ends with braces, has JSON-like syntax
/// - TOON: Has key-value patterns, no complex nesting
///
/// Returns `None` if the format cannot be determined.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::common::{estimate_format, SerializationFormat};
/// assert_eq!(estimate_format(r#"{"key": "value"}"#), Some(SerializationFormat::Json));
/// assert_eq!(estimate_format("key: value\nother: data"), Some(SerializationFormat::Toon));
/// assert_eq!(estimate_format("ambiguous content"), None);
/// ```
#[must_use]
pub fn estimate_format(content: &str) -> Option<SerializationFormat> {
    let trimmed = content.trim();

    // JSON indicators
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        // Check for JSON-like syntax patterns
        if trimmed.contains("\":") || trimmed.contains("\": ") {
            return Some(SerializationFormat::Json);
        }
    }

    // TOON indicators
    if trimmed.contains(": ") && !trimmed.starts_with('{') {
        // Check for TOON-like patterns
        let lines_with_colons = trimmed
            .lines()
            .filter(|line| {
                let line = line.trim();
                !line.is_empty()
                    && line.contains(": ")
                    && !line.starts_with("//")
                    && !line.starts_with('#')
            })
            .count();

        if lines_with_colons > 0 {
            return Some(SerializationFormat::Toon);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // Format Tests
    // ============================================================================

    #[test]
    fn test_markdown_language_detection() {
        assert_eq!(
            MarkdownLanguage::detect_format("json"),
            Some(SerializationFormat::Json)
        );
        assert_eq!(
            MarkdownLanguage::detect_format("javascript"),
            Some(SerializationFormat::Json)
        );
        assert_eq!(
            MarkdownLanguage::detect_format("js"),
            Some(SerializationFormat::Json)
        );
        assert_eq!(
            MarkdownLanguage::detect_format("typescript"),
            Some(SerializationFormat::Json)
        );
        assert_eq!(
            MarkdownLanguage::detect_format("ts"),
            Some(SerializationFormat::Json)
        );
        assert_eq!(
            MarkdownLanguage::detect_format("toon"),
            Some(SerializationFormat::Toon)
        );
        assert_eq!(MarkdownLanguage::detect_format("python"), None);
    }

    #[test]
    fn test_serialization_format() {
        assert_eq!(SerializationFormat::Json.as_ref(), "json");
        assert_eq!(SerializationFormat::Toon.as_ref(), "toon");
        assert_eq!(SerializationFormat::default(), SerializationFormat::Json);
    }

    #[test]
    fn test_as_markdown_tags() {
        let json_tags = SerializationFormat::Json.as_markdown_tags();
        assert!(json_tags.contains(&"json"));
        assert!(json_tags.contains(&"javascript"));
        assert!(json_tags.contains(&"js"));
        assert!(json_tags.contains(&"typescript"));
        assert!(json_tags.contains(&"ts"));

        let toon_tags = SerializationFormat::Toon.as_markdown_tags();
        assert!(toon_tags.contains(&"toon"));
    }

    #[test]
    fn test_opposite_format() {
        assert_eq!(
            SerializationFormat::Json.opposite(),
            SerializationFormat::Toon
        );
        assert_eq!(
            SerializationFormat::Toon.opposite(),
            SerializationFormat::Json
        );
    }

    // ============================================================================
    // Config Tests
    // ============================================================================

    #[test]
    fn test_parse_config_default() {
        let config = ParseConfig::new();
        assert_eq!(config.preferred_format, None);
        assert!(!config.enable_fallback);

        let config_default = ParseConfig::default();
        assert_eq!(config_default.preferred_format, None);
        assert!(!config_default.enable_fallback);
    }

    #[test]
    fn test_parse_config_presets() {
        let config = ParseConfig::json_only();
        assert_eq!(config.preferred_format, Some(SerializationFormat::Json));
        assert!(!config.enable_fallback);

        let config = ParseConfig::toon_only();
        assert_eq!(config.preferred_format, Some(SerializationFormat::Toon));
        assert!(!config.enable_fallback);

        let config = ParseConfig::json_with_toon_fallback();
        assert_eq!(config.preferred_format, Some(SerializationFormat::Json));
        assert!(config.enable_fallback);

        let config = ParseConfig::toon_with_json_fallback();
        assert_eq!(config.preferred_format, Some(SerializationFormat::Toon));
        assert!(config.enable_fallback);
    }

    #[test]
    fn test_parse_config_builder_methods() {
        let config = ParseConfig::new()
            .with_preferred_format(SerializationFormat::Json)
            .with_fallback(true);
        assert_eq!(config.preferred_format, Some(SerializationFormat::Json));
        assert!(config.enable_fallback);

        let config = ParseConfig::new()
            .with_preferred_format(SerializationFormat::Toon)
            .with_fallback(false);
        assert_eq!(config.preferred_format, Some(SerializationFormat::Toon));
        assert!(!config.enable_fallback);

        // Test method chaining
        let config = ParseConfig::new()
            .with_fallback(true)
            .with_preferred_format(SerializationFormat::Toon);
        assert_eq!(config.preferred_format, Some(SerializationFormat::Toon));
        assert!(config.enable_fallback);
    }

    #[test]
    fn test_parse_config_fallback_format() {
        let config = ParseConfig::json_only();
        assert_eq!(config.fallback_format(), Some(SerializationFormat::Toon));

        let config = ParseConfig::toon_only();
        assert_eq!(config.fallback_format(), Some(SerializationFormat::Json));

        let config = ParseConfig::new(); // No preferred format
        assert_eq!(config.fallback_format(), None);
    }

    #[test]
    fn test_preferred_or_default() {
        let config = ParseConfig::json_only();
        assert_eq!(config.preferred_or_default(), SerializationFormat::Json);

        let config = ParseConfig::toon_only();
        assert_eq!(config.preferred_or_default(), SerializationFormat::Toon);

        let config = ParseConfig::new();
        assert_eq!(config.preferred_or_default(), SerializationFormat::Json);
    }

    // ============================================================================
    // Utility Tests
    // ============================================================================

    #[test]
    fn test_validate_non_empty_content() {
        assert!(validate_non_empty_content("valid content").is_ok());
        assert!(validate_non_empty_content("a").is_ok());

        assert!(validate_non_empty_content("").is_err());
        assert!(validate_non_empty_content("   ").is_err());
        assert!(validate_non_empty_content("\n\t\r  ").is_err());
    }

    #[test]
    fn test_validate_json_structure() {
        assert!(validate_json_structure(r#"{"key": "value"}"#).is_ok());
        assert!(validate_json_structure(r#"{"nested": {"inner": "value"}}"#).is_ok());

        assert!(validate_json_structure("not json").is_err());
        assert!(validate_json_structure(r#"{"unbalanced": "braces""#).is_err());
        assert!(validate_json_structure("[]").is_err()); // arrays not supported in this validation
    }

    #[test]
    fn test_validate_toon_structure() {
        assert!(validate_toon_structure("key: value").is_ok());
        assert!(validate_toon_structure("key: value\nother: data").is_ok());

        assert!(validate_toon_structure("no colons here").is_err());
        assert!(validate_toon_structure("// just: comments").is_err());
        assert!(validate_toon_structure("# hash: comments").is_err());
    }

    #[test]
    fn test_validate_content_format() {
        assert!(validate_content_format(r#"{"key": "value"}"#, SerializationFormat::Json).is_ok());
        assert!(validate_content_format("key: value", SerializationFormat::Toon).is_ok());

        assert!(validate_content_format("", SerializationFormat::Json).is_err());
        assert!(validate_content_format("invalid", SerializationFormat::Json).is_err());
    }

    #[test]
    fn test_normalize_content() {
        let input = "  \r\n  key: value  \r\n  \r\n  other: data  \r\n  ";
        let expected = "key: value\n\nother: data";
        assert_eq!(normalize_content(input), expected);

        let simple = "  simple content  ";
        assert_eq!(normalize_content(simple), "simple content");
    }

    #[test]
    fn test_truncate_for_display() {
        let short = "short";
        assert_eq!(truncate_for_display(short, 10), "short");

        let long = "a".repeat(100);
        let truncated = truncate_for_display(&long, 20);
        assert_eq!(truncated.len(), 23); // 20 + "..."
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_looks_like_markdown() {
        assert!(looks_like_markdown("```json\ncontent\n```"));
        assert!(looks_like_markdown(
            "text before ```\ncontent\n``` text after"
        ));

        assert!(!looks_like_markdown("plain text"));
        assert!(!looks_like_markdown("no code blocks here"));
    }

    #[test]
    fn test_estimate_format() {
        // JSON patterns
        assert_eq!(
            estimate_format(r#"{"key": "value"}"#),
            Some(SerializationFormat::Json)
        );
        assert_eq!(
            estimate_format(r#"{"nested": {"inner": "value"}}"#),
            Some(SerializationFormat::Json)
        );

        // TOON patterns
        assert_eq!(
            estimate_format("key: value"),
            Some(SerializationFormat::Toon)
        );
        assert_eq!(
            estimate_format("key: value\nother: data"),
            Some(SerializationFormat::Toon)
        );

        // Ambiguous or unclear
        assert_eq!(estimate_format("ambiguous content"), None);
        assert_eq!(estimate_format(""), None);
        assert_eq!(estimate_format("// comment: not data"), None);
    }
}
