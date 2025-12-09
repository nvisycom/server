//! Utilities specific to deserialization and parsing operations.

use super::common::{ParseConfig, SerializationFormat};
use crate::Error;

/// Creates a detailed parsing error with context about the content and format.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::{deserialization_utils::create_parsing_error, common::format::SerializationFormat};
/// let error = create_parsing_error(
///     SerializationFormat::Json,
///     "invalid syntax",
///     Some("malformed content")
/// );
/// ```
#[must_use]
pub fn create_parsing_error(
    format: SerializationFormat,
    message: &str,
    content_preview: Option<&str>,
) -> Error {
    let format_name = format.as_ref().to_uppercase();
    let error_message = match content_preview {
        Some(preview) => {
            let truncated = if preview.len() > 100 {
                format!("{}...", &preview[..100])
            } else {
                preview.to_string()
            };
            format!(
                "Failed to parse {} content: {}. Content preview: '{}'",
                format_name, message, truncated
            )
        }
        None => format!("Failed to parse {} content: {}", format_name, message),
    };

    Error::invalid_response(error_message)
}

/// Gets the list of formats to try based on configuration preferences.
///
/// Returns formats in the order they should be attempted:
/// 1. Preferred format (if specified)
/// 2. Fallback format (if fallback is enabled)
/// 3. Default to JSON if no preference
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::{deserialization_utils::get_format_sequence, common::config::ParseConfig, common::format::SerializationFormat};
/// let config = ParseConfig::new()
///     .with_preferred_format(SerializationFormat::Toon)
///     .with_fallback(true);
/// let formats = get_format_sequence(&config);
/// assert_eq!(formats, vec![SerializationFormat::Toon, SerializationFormat::Json]);
/// ```
#[must_use]
pub fn get_format_sequence(config: &ParseConfig) -> Vec<SerializationFormat> {
    if let Some(preferred) = config.preferred_format {
        if config.enable_fallback {
            vec![preferred, preferred.opposite()]
        } else {
            vec![preferred]
        }
    } else if config.enable_fallback {
        // No preferred format, try both starting with JSON
        vec![SerializationFormat::Json, SerializationFormat::Toon]
    } else {
        // Default to JSON only
        vec![SerializationFormat::Json]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_parsing_error() {
        let error =
            create_parsing_error(SerializationFormat::Json, "syntax error", Some("malformed"));
        let error_str = error.to_string();
        assert!(error_str.contains("JSON"));
        assert!(error_str.contains("syntax error"));
        assert!(error_str.contains("malformed"));

        let error_no_preview =
            create_parsing_error(SerializationFormat::Toon, "parse failed", None);
        let error_str = error_no_preview.to_string();
        assert!(error_str.contains("TOON"));
        assert!(error_str.contains("parse failed"));
    }

    #[test]
    fn test_get_format_sequence() {
        // With preferred format and fallback
        let config = ParseConfig::new()
            .with_preferred_format(SerializationFormat::Json)
            .with_fallback(true);
        assert_eq!(
            get_format_sequence(&config),
            vec![SerializationFormat::Json, SerializationFormat::Toon]
        );

        // With preferred format but no fallback
        let config = ParseConfig::json_only();
        assert_eq!(
            get_format_sequence(&config),
            vec![SerializationFormat::Json]
        );

        // No preferred format but with fallback
        let config = ParseConfig::new().with_fallback(true);
        assert_eq!(
            get_format_sequence(&config),
            vec![SerializationFormat::Json, SerializationFormat::Toon]
        );

        // Default (no preferred, no fallback)
        let config = ParseConfig::new();
        assert_eq!(
            get_format_sequence(&config),
            vec![SerializationFormat::Json]
        );
    }
}
