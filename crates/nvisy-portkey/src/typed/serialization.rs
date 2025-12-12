//! JSON serialization utilities for typed requests and responses.

use serde::Serialize;

use crate::Result;

/// Serializes a value to a JSON string.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn serialize_to_json<T: Serialize>(value: &T) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

/// Serializes a value to a pretty-printed JSON string.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn serialize_to_pretty_json<T: Serialize>(value: &T) -> Result<String> {
    Ok(serde_json::to_string_pretty(value)?)
}

/// Serializes a value and wraps it in a markdown JSON code block.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn serialize_as_markdown<T: Serialize>(value: &T) -> Result<String> {
    let json = serialize_to_json(value)?;
    Ok(format!("```json\n{json}\n```"))
}

/// Serializes a value and wraps it in a markdown JSON code block with pretty printing.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn serialize_as_pretty_markdown<T: Serialize>(value: &T) -> Result<String> {
    let json = serialize_to_pretty_json(value)?;
    Ok(format!("```json\n{json}\n```"))
}

#[cfg(test)]
mod tests {
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Serialize, Deserialize, JsonSchema)]
    struct TestStruct {
        name: String,
        value: i32,
    }

    #[test]
    fn test_serialize_to_json() {
        let data = TestStruct {
            name: "test".to_string(),
            value: 42,
        };
        let json = serialize_to_json(&data).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("42"));
    }

    #[test]
    fn test_serialize_to_pretty_json() {
        let data = TestStruct {
            name: "test".to_string(),
            value: 42,
        };
        let json = serialize_to_pretty_json(&data).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("\n"));
    }

    #[test]
    fn test_serialize_as_markdown() {
        let data = TestStruct {
            name: "test".to_string(),
            value: 42,
        };
        let markdown = serialize_as_markdown(&data).unwrap();
        assert!(markdown.starts_with("```json"));
        assert!(markdown.ends_with("```"));
        assert!(markdown.contains("test"));
    }

    #[test]
    fn test_serialize_as_pretty_markdown() {
        let data = TestStruct {
            name: "test".to_string(),
            value: 42,
        };
        let markdown = serialize_as_pretty_markdown(&data).unwrap();
        assert!(markdown.starts_with("```json"));
        assert!(markdown.contains("\n"));
    }
}
