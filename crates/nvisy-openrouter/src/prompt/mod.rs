//! Data redaction prompt utilities.
//!
//! This module provides specialized prompt handling for data redaction tasks,
//! where the LLM needs to identify which data items should be redacted based
//! on user-specified criteria.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

pub mod client;

/// A single data item that may need redaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionItem {
    /// Unique identifier for this data item
    pub id: String,
    /// The text content that may contain sensitive data
    pub text: String,
    /// The entity this data belongs to (e.g., person name, organization)
    pub entity: String,
    /// The type of data (e.g., "address", "phone", "date of birth")
    #[serde(rename = "type")]
    pub data_type: String,
}

/// A complete redaction request containing data items and redaction criteria.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionRequest {
    /// List of data items to evaluate for redaction
    pub data: Vec<RedactionItem>,
    /// User prompt specifying what should be redacted
    pub prompt: String,
}

/// Response from the LLM containing the IDs of items to redact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionResponse {
    /// List of IDs that should be redacted
    pub redact_ids: Vec<String>,
}

/// Builder for creating redaction prompts optimized for LLM processing.
pub struct RedactionPrompt {
    system_prompt: String,
}

impl Default for RedactionPrompt {
    fn default() -> Self {
        Self::new()
    }
}

impl RedactionPrompt {
    /// Creates a new redaction prompt builder with default system instructions.
    pub fn new() -> Self {
        Self {
            system_prompt: Self::default_system_prompt(),
        }
    }

    /// Creates a redaction prompt builder with custom system instructions.
    pub fn with_system_prompt(system_prompt: impl Into<String>) -> Self {
        Self {
            system_prompt: system_prompt.into(),
        }
    }

    /// Formats a redaction request into a complete prompt for the LLM.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_openrouter::prompt::{RedactionPrompt, RedactionRequest, RedactionItem};
    ///
    /// let request = RedactionRequest {
    ///     data: vec![
    ///         RedactionItem {
    ///             id: "1".to_string(),
    ///             text: "123 Main St, 555-1234".to_string(),
    ///             entity: "John Doe".to_string(),
    ///             data_type: "address".to_string(),
    ///         }
    ///     ],
    ///     prompt: "Redact all addresses that belong to John Doe".to_string(),
    /// };
    ///
    /// let prompt = RedactionPrompt::new();
    /// let formatted = prompt.format_request(&request);
    /// ```
    pub fn format_request(&self, request: &RedactionRequest) -> String {
        let data_json =
            serde_json::to_string_pretty(&request.data).unwrap_or_else(|_| "[]".to_string());

        format!(
            "{}\n\n## Data Items\n```json\n{}\n```\n\n## Redaction Request\n{}\n\n## Instructions\nAnalyze the data items above and return a JSON array containing only the IDs of items that should be redacted according to the request. Return only the JSON array, no other text.\n\nExample response format: [\"1\", \"3\", \"7\"]",
            self.system_prompt, data_json, request.prompt
        )
    }

    /// Parses the LLM response to extract redaction IDs.
    ///
    /// This method is robust and handles various response formats:
    /// - Pure JSON array: `["1", "2", "3"]`
    /// - JSON wrapped in markdown: ```json\n["1", "2"]\n```
    /// - JSON with extra whitespace or text
    pub fn parse_response(&self, response: &str) -> Result<RedactionResponse, ParseError> {
        // Clean the response - remove markdown code blocks and extra whitespace
        let cleaned = response
            .trim()
            .strip_prefix("```json")
            .unwrap_or(response)
            .strip_prefix("```")
            .unwrap_or(response)
            .strip_suffix("```")
            .unwrap_or(response)
            .trim();

        // Try to parse as JSON array
        match serde_json::from_str::<Vec<String>>(cleaned) {
            Ok(ids) => Ok(RedactionResponse { redact_ids: ids }),
            Err(_) => {
                // Fallback: try to extract JSON array from text
                if let Some(start) = cleaned.find('[') {
                    if let Some(end) = cleaned.rfind(']') {
                        let json_part = &cleaned[start..=end];
                        match serde_json::from_str::<Vec<String>>(json_part) {
                            Ok(ids) => Ok(RedactionResponse { redact_ids: ids }),
                            Err(e) => Err(ParseError::InvalidJson(e.to_string())),
                        }
                    } else {
                        Err(ParseError::NoJsonArray)
                    }
                } else {
                    Err(ParseError::NoJsonArray)
                }
            }
        }
    }

    /// Returns the default system prompt for redaction tasks.
    fn default_system_prompt() -> String {
        "You are a data privacy assistant that helps identify which data items should be redacted based on specific criteria. You will receive a list of data items, each with an ID, text content, associated entity, and data type. Your task is to determine which items should be redacted according to the user's request.

Guidelines:
- Only redact items that clearly match the specified criteria
- Be precise - don't redact items unless they explicitly match the request
- Consider the entity, data type, and text content when making decisions
- Return only the IDs of items to redact, nothing else".to_string()
    }
}

/// Errors that can occur when parsing LLM responses.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("No JSON array found in response")]
    NoJsonArray,
    #[error("Invalid JSON format: {0}")]
    InvalidJson(String),
}

/// Validates that all redaction IDs exist in the original data.
pub fn validate_redaction_ids(
    request: &RedactionRequest,
    response: &RedactionResponse,
) -> Result<(), ValidationError> {
    let valid_ids: HashSet<&str> = request.data.iter().map(|item| item.id.as_str()).collect();

    for id in &response.redact_ids {
        if !valid_ids.contains(id.as_str()) {
            return Err(ValidationError::InvalidId(id.clone()));
        }
    }

    Ok(())
}

/// Errors that can occur during validation.
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Invalid redaction ID: {0}")]
    InvalidId(String),
}

// Re-export main types for convenience
pub use client::RedactionClient;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redaction_item_serialization() {
        let item = RedactionItem {
            id: "1".to_string(),
            text: "123 Main St".to_string(),
            entity: "John Doe".to_string(),
            data_type: "address".to_string(),
        };

        let json = serde_json::to_string(&item).unwrap();
        let deserialized: RedactionItem = serde_json::from_str(&json).unwrap();

        assert_eq!(item.id, deserialized.id);
        assert_eq!(item.text, deserialized.text);
        assert_eq!(item.entity, deserialized.entity);
        assert_eq!(item.data_type, deserialized.data_type);
    }

    #[test]
    fn test_prompt_formatting() {
        let request = RedactionRequest {
            data: vec![RedactionItem {
                id: "1".to_string(),
                text: "123 Main St".to_string(),
                entity: "John Doe".to_string(),
                data_type: "address".to_string(),
            }],
            prompt: "Redact all addresses".to_string(),
        };

        let prompt = RedactionPrompt::new();
        let formatted = prompt.format_request(&request);

        assert!(formatted.contains("123 Main St"));
        assert!(formatted.contains("Redact all addresses"));
        assert!(formatted.contains("JSON array"));
    }

    #[test]
    fn test_response_parsing() {
        let prompt = RedactionPrompt::new();

        // Test clean JSON
        let response1 = r#"["1", "2", "3"]"#;
        let parsed1 = prompt.parse_response(response1).unwrap();
        assert_eq!(parsed1.redact_ids, vec!["1", "2", "3"]);

        // Test JSON with markdown
        let response2 = "```json\n[\"1\", \"2\"]\n```";
        let parsed2 = prompt.parse_response(response2).unwrap();
        assert_eq!(parsed2.redact_ids, vec!["1", "2"]);

        // Test JSON with extra text
        let response3 = "Based on the criteria, here are the IDs: [\"1\"] that should be redacted.";
        let parsed3 = prompt.parse_response(response3).unwrap();
        assert_eq!(parsed3.redact_ids, vec!["1"]);

        // Test empty array
        let response4 = "[]";
        let parsed4 = prompt.parse_response(response4).unwrap();
        assert!(parsed4.redact_ids.is_empty());
    }

    #[test]
    fn test_validation() {
        let request = RedactionRequest {
            data: vec![RedactionItem {
                id: "1".to_string(),
                text: "data".to_string(),
                entity: "entity".to_string(),
                data_type: "type".to_string(),
            }],
            prompt: "test".to_string(),
        };

        // Valid response
        let valid_response = RedactionResponse {
            redact_ids: vec!["1".to_string()],
        };
        assert!(validate_redaction_ids(&request, &valid_response).is_ok());

        // Invalid response - ID doesn't exist
        let invalid_response = RedactionResponse {
            redact_ids: vec!["999".to_string()],
        };
        assert!(validate_redaction_ids(&request, &invalid_response).is_err());
    }

    #[test]
    fn test_custom_system_prompt() {
        let custom_prompt = "Custom instructions for redaction";
        let prompt = RedactionPrompt::with_system_prompt(custom_prompt);

        let request = RedactionRequest {
            data: vec![],
            prompt: "test".to_string(),
        };

        let formatted = prompt.format_request(&request);
        assert!(formatted.contains(custom_prompt));
    }
}
