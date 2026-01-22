//! JSON schema validation tool.

use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Error type for JSON schema operations.
#[derive(Debug, thiserror::Error)]
pub enum JsonSchemaError {
    #[error("invalid schema: {0}")]
    InvalidSchema(String),
    #[error("invalid JSON: {0}")]
    InvalidJson(String),
    #[error("validation failed: {errors:?}")]
    ValidationFailed { errors: Vec<String> },
}

/// Arguments for JSON schema validation.
#[derive(Debug, Deserialize)]
pub struct JsonSchemaArgs {
    /// The JSON schema to validate against.
    pub schema: Value,
    /// The JSON data to validate.
    pub data: Value,
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

/// Tool for validating JSON against a schema.
pub struct JsonSchemaTool;

impl JsonSchemaTool {
    /// Creates a new JSON schema tool.
    pub fn new() -> Self {
        Self
    }

    /// Validates JSON data against a schema.
    ///
    /// This is a simplified validator that checks:
    /// - Type matching
    /// - Required properties
    /// - Basic constraints
    fn validate(schema: &Value, data: &Value, path: &str) -> Vec<String> {
        let mut errors = Vec::new();

        // Get the expected type
        let expected_type = schema.get("type").and_then(|t| t.as_str());

        match expected_type {
            Some("object") => {
                if !data.is_object() {
                    errors.push(format!("{path}: expected object, got {}", type_name(data)));
                    return errors;
                }

                let obj = data.as_object().unwrap();

                // Check required properties
                if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
                    for req in required {
                        if let Some(field) = req.as_str()
                            && !obj.contains_key(field)
                        {
                            errors.push(format!("{path}: missing required property '{field}'"));
                        }
                    }
                }

                // Validate properties
                if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
                    for (key, prop_schema) in properties {
                        if let Some(value) = obj.get(key) {
                            let prop_path = if path.is_empty() {
                                key.clone()
                            } else {
                                format!("{path}.{key}")
                            };
                            errors.extend(Self::validate(prop_schema, value, &prop_path));
                        }
                    }
                }
            }
            Some("array") => {
                if !data.is_array() {
                    errors.push(format!("{path}: expected array, got {}", type_name(data)));
                    return errors;
                }

                let arr = data.as_array().unwrap();

                // Check min/max items
                if let Some(min) = schema.get("minItems").and_then(|m| m.as_u64())
                    && (arr.len() as u64) < min
                {
                    errors.push(format!(
                        "{path}: array has {} items, minimum is {min}",
                        arr.len()
                    ));
                }
                if let Some(max) = schema.get("maxItems").and_then(|m| m.as_u64())
                    && (arr.len() as u64) > max
                {
                    errors.push(format!(
                        "{path}: array has {} items, maximum is {max}",
                        arr.len()
                    ));
                }

                // Validate items
                if let Some(items_schema) = schema.get("items") {
                    for (i, item) in arr.iter().enumerate() {
                        let item_path = format!("{path}[{i}]");
                        errors.extend(Self::validate(items_schema, item, &item_path));
                    }
                }
            }
            Some("string") => {
                if !data.is_string() {
                    errors.push(format!("{path}: expected string, got {}", type_name(data)));
                    return errors;
                }

                let s = data.as_str().unwrap();

                // Check min/max length
                if let Some(min) = schema.get("minLength").and_then(|m| m.as_u64())
                    && (s.len() as u64) < min
                {
                    errors.push(format!(
                        "{path}: string length {} is less than minimum {min}",
                        s.len()
                    ));
                }
                if let Some(max) = schema.get("maxLength").and_then(|m| m.as_u64())
                    && (s.len() as u64) > max
                {
                    errors.push(format!(
                        "{path}: string length {} exceeds maximum {max}",
                        s.len()
                    ));
                }

                // Check enum
                if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array())
                    && !enum_values.contains(data)
                {
                    errors.push(format!("{path}: value not in enum"));
                }
            }
            Some("number") | Some("integer") => {
                let is_valid = if expected_type == Some("integer") {
                    data.is_i64() || data.is_u64()
                } else {
                    data.is_number()
                };

                if !is_valid {
                    errors.push(format!(
                        "{path}: expected {}, got {}",
                        expected_type.unwrap(),
                        type_name(data)
                    ));
                    return errors;
                }

                if let Some(num) = data.as_f64() {
                    if let Some(min) = schema.get("minimum").and_then(|m| m.as_f64())
                        && num < min
                    {
                        errors.push(format!("{path}: {num} is less than minimum {min}"));
                    }
                    if let Some(max) = schema.get("maximum").and_then(|m| m.as_f64())
                        && num > max
                    {
                        errors.push(format!("{path}: {num} exceeds maximum {max}"));
                    }
                }
            }
            Some("boolean") => {
                if !data.is_boolean() {
                    errors.push(format!("{path}: expected boolean, got {}", type_name(data)));
                }
            }
            Some("null") => {
                if !data.is_null() {
                    errors.push(format!("{path}: expected null, got {}", type_name(data)));
                }
            }
            None => {
                // No type specified, accept anything
            }
            Some(t) => {
                errors.push(format!("{path}: unknown type '{t}'"));
            }
        }

        errors
    }
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "integer"
            } else {
                "number"
            }
        }
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

impl Default for JsonSchemaTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for JsonSchemaTool {
    const NAME: &'static str = "json_schema";

    type Error = JsonSchemaError;
    type Args = JsonSchemaArgs;
    type Output = JsonSchemaResult;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Validate JSON data against a JSON Schema. Use this to verify that structured data conforms to expected format.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "schema": {
                        "type": "object",
                        "description": "The JSON Schema to validate against"
                    },
                    "data": {
                        "description": "The JSON data to validate"
                    }
                },
                "required": ["schema", "data"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let errors = Self::validate(&args.schema, &args.data, "");

        Ok(JsonSchemaResult {
            valid: errors.is_empty(),
            errors,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_valid_object() {
        let tool = JsonSchemaTool::new();
        let result = tool
            .call(JsonSchemaArgs {
                schema: json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "age": { "type": "integer" }
                    },
                    "required": ["name"]
                }),
                data: json!({
                    "name": "Alice",
                    "age": 30
                }),
            })
            .await
            .unwrap();

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_missing_required() {
        let tool = JsonSchemaTool::new();
        let result = tool
            .call(JsonSchemaArgs {
                schema: json!({
                    "type": "object",
                    "required": ["name"]
                }),
                data: json!({}),
            })
            .await
            .unwrap();

        assert!(!result.valid);
        assert!(result.errors[0].contains("missing required"));
    }

    #[tokio::test]
    async fn test_type_mismatch() {
        let tool = JsonSchemaTool::new();
        let result = tool
            .call(JsonSchemaArgs {
                schema: json!({ "type": "string" }),
                data: json!(42),
            })
            .await
            .unwrap();

        assert!(!result.valid);
        assert!(result.errors[0].contains("expected string"));
    }
}
