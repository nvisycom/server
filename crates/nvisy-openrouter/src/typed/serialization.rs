//! Serialization functionality for converting structs to JSON and TOON formats.

use serde::Serialize;

use super::common::{ParseConfig, SerializationFormat, validate_content_format};
use super::deserialization_utils::{create_parsing_error, get_format_sequence};
use crate::Error;

/// Serializes data to the specified format using the appropriate serializer.
///
/// Uses `serde_json` for JSON format and `serde_toon` for TOON format.
///
/// # Errors
///
/// Returns an error if the data cannot be serialized in the specified format.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::{serialization::serialize_with_format, format::SerializationFormat};
/// # use serde::{Deserialize, Serialize};
/// # use schemars::JsonSchema;
/// #
/// # #[derive(Serialize, Deserialize, JsonSchema)]
/// # struct TestData { id: u32, name: String }
///
/// let data = TestData { id: 42, name: "test".to_string() };
/// let json = serialize_with_format(&data, SerializationFormat::Json)?;
/// let toon = serialize_with_format(&data, SerializationFormat::Toon)?;
/// # Ok::<(), nvisy_openrouter::Error>(())
/// ```
pub fn serialize_with_format<T>(data: &T, format: SerializationFormat) -> Result<String, Error>
where
    T: Serialize,
{
    let result = match format {
        SerializationFormat::Json => {
            serde_json::to_string(data).map_err(Error::JsonSerialization)?
        }
        SerializationFormat::Toon => {
            serde_toon::to_string(data).map_err(Error::ToonSerialization)?
        }
    };

    // Validate the serialized output
    validate_content_format(&result, format).map_err(|_| {
        create_parsing_error(format, "serialized output failed validation", Some(&result))
    })?;

    Ok(result)
}

/// Serializes data with pretty formatting for the specified format.
///
/// Provides human-readable output with proper indentation and spacing.
///
/// # Errors
///
/// Returns an error if the data cannot be serialized in the specified format.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::{serialization::serialize_pretty, format::SerializationFormat};
/// # use serde::{Deserialize, Serialize};
/// # use schemars::JsonSchema;
/// #
/// # #[derive(Serialize, Deserialize, JsonSchema)]
/// # struct TestData { id: u32, name: String }
///
/// let data = TestData { id: 42, name: "test".to_string() };
/// let pretty_json = serialize_pretty(&data, SerializationFormat::Json)?;
/// # Ok::<(), nvisy_openrouter::Error>(())
/// ```
pub fn serialize_pretty<T>(data: &T, format: SerializationFormat) -> Result<String, Error>
where
    T: Serialize,
{
    let result = match format {
        SerializationFormat::Json => {
            serde_json::to_string_pretty(data).map_err(Error::JsonSerialization)?
        }
        SerializationFormat::Toon => {
            // TOON doesn't have built-in pretty printing, so we use regular serialization
            serde_toon::to_string(data).map_err(Error::ToonSerialization)?
        }
    };

    // Validate the serialized output
    validate_content_format(&result, format).map_err(|_| {
        create_parsing_error(
            format,
            "pretty serialized output failed validation",
            Some(&result),
        )
    })?;

    Ok(result)
}

/// Serializes data using configuration preferences with optional fallback.
///
/// This function respects the configuration's preferred format and can attempt
/// fallback serialization if the preferred format fails.
///
/// # Errors
///
/// Returns an error if serialization fails in all attempted formats.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::{serialization::serialize_with_config, config::ParseConfig, format::SerializationFormat};
/// # use serde::{Deserialize, Serialize};
/// # use schemars::JsonSchema;
/// #
/// # #[derive(Serialize, Deserialize, JsonSchema)]
/// # struct TestData { id: u32, name: String }
///
/// let data = TestData { id: 42, name: "test".to_string() };
/// let config = ParseConfig::new()
///     .with_preferred_format(SerializationFormat::Json)
///     .with_fallback(true);
/// let result = serialize_with_config(&data, &config)?;
/// # Ok::<(), nvisy_openrouter::Error>(())
/// ```
pub fn serialize_with_config<T>(data: &T, config: &ParseConfig) -> Result<String, Error>
where
    T: Serialize,
{
    let formats_to_try = get_format_sequence(config);
    let mut last_error: Option<Error> = None;

    for format in formats_to_try {
        match serialize_with_format(data, format) {
            Ok(result) => return Ok(result),
            Err(err) => {
                last_error = Some(err);
                if !config.enable_fallback {
                    break;
                }
            }
        }
    }

    match last_error {
        Some(err) => Err(err),
        None => Err(Error::invalid_response(
            "No serialization formats were attempted",
        )),
    }
}

/// Serializes data with markdown code block wrapping.
///
/// Wraps the serialized output in markdown code blocks with the appropriate
/// language tag for syntax highlighting.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::{serialization::serialize_as_markdown, format::SerializationFormat};
/// # use serde::{Deserialize, Serialize};
/// # use schemars::JsonSchema;
/// #
/// # #[derive(Serialize, Deserialize, JsonSchema)]
/// # struct TestData { id: u32, name: String }
///
/// let data = TestData { id: 42, name: "test".to_string() };
/// let markdown = serialize_as_markdown(&data, SerializationFormat::Json)?;
/// // Output: ```json\n{"id":42,"name":"test"}\n```
/// # Ok::<(), nvisy_openrouter::Error>(())
/// ```
pub fn serialize_as_markdown<T>(data: &T, format: SerializationFormat) -> Result<String, Error>
where
    T: Serialize,
{
    let content = serialize_with_format(data, format)?;
    let language_tag = match format {
        SerializationFormat::Json => "json",
        SerializationFormat::Toon => "toon",
    };

    Ok(format!("```{}\n{}\n```", language_tag, content))
}

/// Serializes data with pretty formatting and markdown wrapping.
///
/// Combines pretty formatting with markdown code block wrapping for
/// human-readable output suitable for documentation or display.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::{serialization::serialize_as_pretty_markdown, format::SerializationFormat};
/// # use serde::{Deserialize, Serialize};
/// # use schemars::JsonSchema;
/// #
/// # #[derive(Serialize, Deserialize, JsonSchema)]
/// # struct TestData { id: u32, name: String }
///
/// let data = TestData { id: 42, name: "test".to_string() };
/// let markdown = serialize_as_pretty_markdown(&data, SerializationFormat::Json)?;
/// # Ok::<(), nvisy_openrouter::Error>(())
/// ```
pub fn serialize_as_pretty_markdown<T>(
    data: &T,
    format: SerializationFormat,
) -> Result<String, Error>
where
    T: Serialize,
{
    let content = serialize_pretty(data, format)?;
    let language_tag = match format {
        SerializationFormat::Json => "json",
        SerializationFormat::Toon => "toon",
    };

    Ok(format!("```{}\n{}\n```", language_tag, content))
}

/// Convenience function for JSON serialization.
///
/// # Errors
///
/// Returns an error if the data cannot be serialized as JSON.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::serialization::serialize_to_json;
/// # use serde::{Deserialize, Serialize};
/// # use schemars::JsonSchema;
/// #
/// # #[derive(Serialize, Deserialize, JsonSchema)]
/// # struct TestData { id: u32 }
///
/// let data = TestData { id: 42 };
/// let json = serialize_to_json(&data)?;
/// # Ok::<(), nvisy_openrouter::Error>(())
/// ```
pub fn serialize_to_json<T>(data: &T) -> Result<String, Error>
where
    T: Serialize,
{
    serialize_with_format(data, SerializationFormat::Json)
}

/// Convenience function for TOON serialization.
///
/// # Errors
///
/// Returns an error if the data cannot be serialized as TOON.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::serialization::serialize_to_toon;
/// # use serde::{Deserialize, Serialize};
/// # use schemars::JsonSchema;
/// #
/// # #[derive(Serialize, Deserialize, JsonSchema)]
/// # struct TestData { id: u32 }
///
/// let data = TestData { id: 42 };
/// let toon = serialize_to_toon(&data)?;
/// # Ok::<(), nvisy_openrouter::Error>(())
/// ```
pub fn serialize_to_toon<T>(data: &T) -> Result<String, Error>
where
    T: Serialize,
{
    serialize_with_format(data, SerializationFormat::Toon)
}

/// Convenience function for pretty JSON serialization.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::serialization::serialize_to_pretty_json;
/// # use serde::{Deserialize, Serialize};
/// # use schemars::JsonSchema;
/// #
/// # #[derive(Serialize, Deserialize, JsonSchema)]
/// # struct TestData { id: u32, name: String }
///
/// let data = TestData { id: 42, name: "test".to_string() };
/// let pretty_json = serialize_to_pretty_json(&data)?;
/// # Ok::<(), nvisy_openrouter::Error>(())
/// ```
pub fn serialize_to_pretty_json<T>(data: &T) -> Result<String, Error>
where
    T: Serialize,
{
    serialize_pretty(data, SerializationFormat::Json)
}

/// Tries to serialize data, returning the first successful format.
///
/// Attempts serialization in multiple formats and returns the result from
/// the first format that succeeds.
///
/// # Examples
///
/// ```rust
/// # use nvisy_openrouter::typed::{serialization::serialize_any_format, format::SerializationFormat};
/// # use serde::{Deserialize, Serialize};
/// # use schemars::JsonSchema;
/// #
/// # #[derive(Serialize, Deserialize, JsonSchema)]
/// # struct TestData { id: u32 }
///
/// let data = TestData { id: 42 };
/// let formats = vec![SerializationFormat::Json, SerializationFormat::Toon];
/// let (result, used_format) = serialize_any_format(&data, &formats)?;
/// # Ok::<(), nvisy_openrouter::Error>(())
/// ```
pub fn serialize_any_format<T>(
    data: &T,
    formats: &[SerializationFormat],
) -> Result<(String, SerializationFormat), Error>
where
    T: Serialize,
{
    if formats.is_empty() {
        return Err(Error::invalid_response("No formats specified"));
    }

    let mut errors = Vec::new();

    for &format in formats {
        match serialize_with_format(data, format) {
            Ok(result) => return Ok((result, format)),
            Err(err) => errors.push((format, err)),
        }
    }

    // All formats failed
    let error_summary = errors
        .iter()
        .map(|(format, err)| format!("{}: {}", format.as_ref().to_uppercase(), err))
        .collect::<Vec<_>>()
        .join("; ");

    Err(Error::invalid_response(format!(
        "Failed to serialize data in any format. Errors: {}",
        error_summary
    )))
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
        tags: Vec<String>,
    }

    impl Default for TestData {
        fn default() -> Self {
            Self {
                id: 42,
                name: "test".to_string(),
                active: true,
                tags: vec!["tag1".to_string(), "tag2".to_string()],
            }
        }
    }

    #[test]
    fn test_serialize_with_format() {
        let data = TestData::default();

        let json = serialize_with_format(&data, SerializationFormat::Json).unwrap();
        assert!(json.contains("\"id\":42"));
        assert!(json.contains("\"name\":\"test\""));

        let toon = serialize_with_format(&data, SerializationFormat::Toon).unwrap();
        assert!(toon.contains("id: 42"));
        assert!(toon.contains("name: test"));
    }

    #[test]
    fn test_serialize_pretty() {
        let data = TestData::default();

        let pretty_json = serialize_pretty(&data, SerializationFormat::Json).unwrap();
        assert!(pretty_json.contains("{\n"));
        assert!(pretty_json.contains("  \"id\": 42"));

        let toon = serialize_pretty(&data, SerializationFormat::Toon).unwrap();
        assert!(toon.contains("id: 42"));
    }

    #[test]
    fn test_serialize_with_config() {
        let data = TestData::default();

        // JSON preferred
        let config = ParseConfig::new().with_preferred_format(SerializationFormat::Json);
        let result = serialize_with_config(&data, &config).unwrap();
        assert!(result.contains("\"id\":42"));

        // TOON preferred
        let config = ParseConfig::new().with_preferred_format(SerializationFormat::Toon);
        let result = serialize_with_config(&data, &config).unwrap();
        assert!(result.contains("id: 42"));

        // Default (no preference)
        let config = ParseConfig::new();
        let result = serialize_with_config(&data, &config).unwrap();
        assert!(result.contains("\"id\":42")); // Should default to JSON
    }

    #[test]
    fn test_serialize_as_markdown() {
        let data = TestData::default();

        let json_markdown = serialize_as_markdown(&data, SerializationFormat::Json).unwrap();
        assert!(json_markdown.starts_with("```json\n"));
        assert!(json_markdown.ends_with("\n```"));
        assert!(json_markdown.contains("\"id\":42"));

        let toon_markdown = serialize_as_markdown(&data, SerializationFormat::Toon).unwrap();
        assert!(toon_markdown.starts_with("```toon\n"));
        assert!(toon_markdown.ends_with("\n```"));
        assert!(toon_markdown.contains("id: 42"));
    }

    #[test]
    fn test_serialize_as_pretty_markdown() {
        let data = TestData::default();

        let pretty_markdown =
            serialize_as_pretty_markdown(&data, SerializationFormat::Json).unwrap();
        assert!(pretty_markdown.starts_with("```json\n"));
        assert!(pretty_markdown.ends_with("\n```"));
        assert!(pretty_markdown.contains("  \"id\": 42"));
    }

    #[test]
    fn test_convenience_functions() {
        let data = TestData::default();

        let json = serialize_to_json(&data).unwrap();
        assert!(json.contains("\"id\":42"));

        let toon = serialize_to_toon(&data).unwrap();
        assert!(toon.contains("id: 42"));

        let pretty_json = serialize_to_pretty_json(&data).unwrap();
        assert!(pretty_json.contains("  \"id\": 42"));
    }

    #[test]
    fn test_serialize_any_format() {
        let data = TestData::default();

        let formats = vec![SerializationFormat::Json, SerializationFormat::Toon];
        let (result, used_format) = serialize_any_format(&data, &formats).unwrap();
        assert_eq!(used_format, SerializationFormat::Json); // Should use first successful format
        assert!(result.contains("\"id\":42"));

        let formats = vec![SerializationFormat::Toon, SerializationFormat::Json];
        let (result, used_format) = serialize_any_format(&data, &formats).unwrap();
        assert_eq!(used_format, SerializationFormat::Toon);
        assert!(result.contains("id: 42"));
    }

    #[test]
    fn test_serialize_any_format_empty() {
        let data = TestData::default();
        let result = serialize_any_format(&data, &[]);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No formats specified")
        );
    }

    #[test]
    fn test_config_with_fallback() {
        let data = TestData::default();
        let config = ParseConfig::new()
            .with_preferred_format(SerializationFormat::Json)
            .with_fallback(true);

        let result = serialize_with_config(&data, &config).unwrap();
        assert!(result.contains("\"id\":42"));
    }

    #[test]
    fn test_config_without_fallback() {
        let data = TestData::default();
        let config = ParseConfig::new()
            .with_preferred_format(SerializationFormat::Json)
            .with_fallback(false);

        let result = serialize_with_config(&data, &config).unwrap();
        assert!(result.contains("\"id\":42"));
    }
}
