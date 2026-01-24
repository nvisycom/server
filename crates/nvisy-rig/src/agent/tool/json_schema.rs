//! JSON schema validation and response parsing.
//!
//! This module provides:
//! - Schema generation from Rust types via `schemars`
//! - JSON validation against schemas via `jsonschema`
//! - LLM response parsing (handles markdown code blocks, etc.)

use std::marker::PhantomData;

use jsonschema::Validator;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Error, Result};

/// Error type for JSON schema operations.
#[derive(Debug, thiserror::Error)]
#[error("json schema error")]
pub struct JsonSchemaError;

/// Arguments for JSON schema validation.
///
/// Generic over `T` which defines the expected schema via `schemars::JsonSchema`.
#[derive(Debug, Deserialize)]
pub struct JsonSchemaArgs<T> {
    /// The JSON data to validate.
    pub data: Value,
    #[serde(skip)]
    _marker: PhantomData<T>,
}

/// Result of JSON schema validation.
#[derive(Debug, Serialize)]
pub struct JsonSchemaResult {
    /// Whether the data is valid.
    pub valid: bool,
    /// Validation errors if any.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

/// Tool for validating JSON against a schema derived from a Rust type.
///
/// Uses `schemars` to generate the JSON schema from the type parameter `T`,
/// and `jsonschema` for validation.
pub struct JsonSchemaTool<T> {
    validator: Validator,
    _marker: PhantomData<T>,
}

impl<T: JsonSchema> JsonSchemaTool<T> {
    /// Creates a new JSON schema tool for type `T`.
    pub fn new() -> Self {
        let mut generator = SchemaSettings::draft07().into_generator();
        let schema = generator.root_schema_for::<T>();
        let schema_value = serde_json::to_value(&schema).expect("schema serialization cannot fail");
        let validator = Validator::new(&schema_value).expect("valid schema");

        Self {
            validator,
            _marker: PhantomData,
        }
    }

    /// Validates JSON data against the schema.
    fn validate_data(&self, data: &Value) -> Vec<String> {
        self.validator
            .iter_errors(data)
            .map(|e| e.to_string())
            .collect()
    }
}

impl<T: JsonSchema> Default for JsonSchemaTool<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: JsonSchema + DeserializeOwned + Send + Sync> Tool for JsonSchemaTool<T> {
    type Args = JsonSchemaArgs<T>;
    type Error = JsonSchemaError;
    type Output = JsonSchemaResult;

    const NAME: &'static str = "json_schema";

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Validate JSON data against a JSON Schema. Use this to verify that structured data conforms to expected format.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "data": {
                        "description": "The JSON data to validate"
                    }
                },
                "required": ["data"]
            }),
        }
    }

    #[tracing::instrument(skip(self, args), fields(tool = Self::NAME))]
    async fn call(&self, args: Self::Args) -> std::result::Result<Self::Output, Self::Error> {
        let errors = self.validate_data(&args.data);
        let valid = errors.is_empty();

        tracing::debug!(valid, error_count = errors.len(), "json_schema completed");

        Ok(JsonSchemaResult { valid, errors })
    }
}

/// Parser for extracting and validating JSON from LLM responses.
///
/// Handles common LLM output patterns:
/// - Plain JSON
/// - JSON wrapped in markdown code blocks (```json ... ```)
/// - JSON wrapped in generic code blocks (``` ... ```)
/// - JSON with surrounding explanatory text
///
/// # Example
///
/// ```ignore
/// use nvisy_rig::agent::tool::JsonResponse;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct UserInfo {
///     name: String,
///     age: u32,
/// }
///
/// let response = r#"Here's the extracted data:
/// ```json
/// {"name": "Alice", "age": 30}
/// ```"#;
///
/// let info: UserInfo = JsonResponse::parse(response)?;
/// ```
pub struct JsonResponse;

impl JsonResponse {
    /// Extracts JSON content from a response, stripping markdown formatting.
    pub fn extract(response: &str) -> &str {
        // Try ```json block first
        if let Some(start) = response.find("```json") {
            let after_marker = &response[start + 7..];
            if let Some(end) = after_marker.find("```") {
                return after_marker[..end].trim();
            }
        }

        // Try generic ``` block
        if let Some(start) = response.find("```") {
            let after_marker = &response[start + 3..];
            // Skip language identifier if on same line
            let content_start = after_marker.find('\n').map(|i| i + 1).unwrap_or(0);
            let after_newline = &after_marker[content_start..];
            if let Some(end) = after_newline.find("```") {
                return after_newline[..end].trim();
            }
        }

        // Try to find JSON object or array boundaries
        let trimmed = response.trim();
        if (trimmed.starts_with('{') && trimmed.ends_with('}'))
            || (trimmed.starts_with('[') && trimmed.ends_with(']'))
        {
            return trimmed;
        }

        // Find first { or [ and last } or ]
        let start = trimmed.find(['{', '[']).unwrap_or(0);
        let end = trimmed
            .rfind(['}', ']'])
            .map(|i| i + 1)
            .unwrap_or(trimmed.len());

        if start < end {
            &trimmed[start..end]
        } else {
            trimmed
        }
    }

    /// Parses JSON from an LLM response into the specified type.
    ///
    /// Automatically strips markdown code blocks and surrounding text.
    pub fn parse<T: DeserializeOwned>(response: &str) -> Result<T> {
        let json_str = Self::extract(response);
        serde_json::from_str(json_str).map_err(|e| Error::parse(format!("invalid JSON: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use schemars::JsonSchema;
    use serde::Deserialize;
    use serde_json::json;

    use super::*;

    #[derive(Debug, Deserialize, JsonSchema, PartialEq)]
    struct TestPerson {
        name: String,
        age: u32,
    }

    #[tokio::test]
    async fn test_valid_object() {
        let tool = JsonSchemaTool::<TestPerson>::new();
        let result = tool
            .call(JsonSchemaArgs {
                data: json!({
                    "name": "Alice",
                    "age": 30
                }),
                _marker: PhantomData,
            })
            .await
            .unwrap();

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_missing_required() {
        let tool = JsonSchemaTool::<TestPerson>::new();
        let result = tool
            .call(JsonSchemaArgs {
                data: json!({}),
                _marker: PhantomData,
            })
            .await
            .unwrap();

        assert!(!result.valid);
        assert!(!result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_type_mismatch() {
        let tool = JsonSchemaTool::<TestPerson>::new();
        let result = tool
            .call(JsonSchemaArgs {
                data: json!({
                    "name": 123,
                    "age": 30
                }),
                _marker: PhantomData,
            })
            .await
            .unwrap();

        assert!(!result.valid);
        assert!(!result.errors.is_empty());
    }

    // JsonResponse tests

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestData {
        key: String,
    }

    #[test]
    fn parse_plain_json() {
        let response = r#"{"key": "value"}"#;
        let result: TestData = JsonResponse::parse(response).unwrap();
        assert_eq!(result.key, "value");
    }

    #[test]
    fn parse_json_with_markdown_block() {
        let response = r#"Here's the JSON:
```json
{"key": "value"}
```"#;
        let result: TestData = JsonResponse::parse(response).unwrap();
        assert_eq!(result.key, "value");
    }

    #[test]
    fn parse_json_with_generic_code_block() {
        let response = r#"```
{"key": "value"}
```"#;
        let result: TestData = JsonResponse::parse(response).unwrap();
        assert_eq!(result.key, "value");
    }

    #[test]
    fn parse_json_with_surrounding_text() {
        let response = r#"The result is: {"key": "value"} as requested."#;
        let result: TestData = JsonResponse::parse(response).unwrap();
        assert_eq!(result.key, "value");
    }

    #[test]
    fn parse_array() {
        let response = r#"[{"key": "a"}, {"key": "b"}]"#;
        let result: Vec<TestData> = JsonResponse::parse(response).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].key, "a");
        assert_eq!(result[1].key, "b");
    }

    #[test]
    fn extract_returns_json_content() {
        let extracted = JsonResponse::extract(
            r#"```json
{"key": "value"}
```"#,
        );
        assert_eq!(extracted, r#"{"key": "value"}"#);
    }
}
