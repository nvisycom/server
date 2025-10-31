//! Typed chat completion response types.

use derive_builder::Builder;
use openrouter_rs::types::completion::CompletionsResponse;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::Error;

/// A typed chat completion response.
///
/// This wraps the parsed response data from an LLM completion request.
///
/// # Type Parameters
///
/// * `T` - The response payload type that implements `JsonSchema + DeserializeOwned`
///
/// # Example
///
/// ```rust
/// use nvisy_openrouter::completion::TypedChatResponse;
/// use serde::{Deserialize, Serialize};
/// use schemars::JsonSchema;
///
/// #[derive(Serialize, Deserialize, JsonSchema)]
/// struct MyResponse {
///     answer: String,
/// }
///
/// let response = TypedChatResponse::<MyResponse>::builder()
///     .with_data(MyResponse { answer: "test".to_string() })
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, Builder, Deserialize)]
#[builder(pattern = "owned", setter(into, strip_option, prefix = "with"))]
#[serde(bound(deserialize = "T: DeserializeOwned"))]
pub struct TypedChatResponse<T>
where
    T: JsonSchema + DeserializeOwned,
{
    /// The parsed response data
    pub data: T,

    /// Optional raw response text
    #[builder(default)]
    pub raw_response: Option<String>,

    /// Optional token usage information
    #[builder(default)]
    pub prompt_tokens: Option<u32>,

    #[builder(default)]
    pub completion_tokens: Option<u32>,

    #[builder(default)]
    pub total_tokens: Option<u32>,
}

impl<T> TypedChatResponse<T>
where
    T: JsonSchema + DeserializeOwned,
{
    /// Creates a new typed chat response builder.
    pub fn builder() -> TypedChatResponseBuilder<T> {
        TypedChatResponseBuilder::default()
    }

    /// Parses a completion response into the typed response.
    ///
    /// This method:
    /// - Extracts the content from the first choice in the response
    /// - Handles various response formats (pure JSON, markdown-wrapped, etc.)
    /// - Adds token usage information from the response
    ///
    /// # Arguments
    ///
    /// * `response` - The completion response from the LLM
    ///
    /// # Returns
    ///
    /// A parsed typed response with token usage information
    ///
    /// # Errors
    ///
    /// - Returns an error if no response choices are available
    /// - Returns an error if the response cannot be parsed as JSON
    pub fn from_llm_response(response: &CompletionsResponse) -> Result<Self, Error> {
        // Extract content from the first choice
        let content = Self::extract_response_content(response)?;

        // Parse the content into typed data
        let data = Self::parse_response_content(&content)?;

        // Build response with token usage
        let mut typed_response = Self {
            data,
            raw_response: Some(content),
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
        };

        // Add token usage information
        Self::add_token_usage(&mut typed_response, response);

        Ok(typed_response)
    }

    /// Extracts the content string from the completion response.
    ///
    /// # Arguments
    ///
    /// * `response` - The completion response from the LLM
    ///
    /// # Returns
    ///
    /// The content string from the first choice
    ///
    /// # Errors
    ///
    /// - Returns an error if no choices are available
    /// - Returns an error if the first choice has no content
    fn extract_response_content(response: &CompletionsResponse) -> Result<String, Error> {
        let choice = response
            .choices
            .first()
            .ok_or_else(|| Error::invalid_response("No response choices returned from LLM"))?;

        choice
            .content()
            .map(|s| s.to_string())
            .ok_or_else(|| Error::invalid_response("No content in LLM response"))
    }

    /// Parses a JSON response string into the typed data.
    ///
    /// This method handles various response formats:
    /// - Pure JSON object
    /// - JSON wrapped in markdown code blocks
    /// - JSON with extra whitespace or text
    ///
    /// # Arguments
    ///
    /// * `response` - The raw response string from the LLM
    ///
    /// # Returns
    ///
    /// The parsed typed data
    ///
    /// # Errors
    ///
    /// Returns an error if the response cannot be parsed as valid JSON
    fn parse_response_content(response: &str) -> Result<T, Error> {
        // Clean the response - remove markdown code blocks and extra whitespace
        let mut response_str = response.trim().strip_prefix("```json").unwrap_or(response);
        response_str = response_str.strip_prefix("```").unwrap_or(response_str);
        let cleaned = response_str
            .strip_suffix("```")
            .unwrap_or(response_str)
            .trim();

        // Try to parse as JSON object
        match serde_json::from_str::<T>(cleaned) {
            Ok(data) => Ok(data),
            Err(_) => {
                // Fallback: try to extract JSON object from text
                if let Some(start) = cleaned.find('{') {
                    if let Some(end) = cleaned.rfind('}') {
                        let json_part = &cleaned[start..=end];
                        serde_json::from_str::<T>(json_part).map_err(Error::Serialization)
                    } else {
                        Err(Error::invalid_response("No JSON object found in response"))
                    }
                } else {
                    Err(Error::invalid_response("No JSON object found in response"))
                }
            }
        }
    }

    /// Adds token usage information from the completion response.
    ///
    /// Updates the typed response with prompt tokens, completion tokens,
    /// and total tokens if available in the response.
    ///
    /// # Arguments
    ///
    /// * `typed_response` - The typed response to update
    /// * `response` - The completion response containing usage information
    fn add_token_usage(typed_response: &mut TypedChatResponse<T>, response: &CompletionsResponse) {
        if let Some(ref usage) = response.usage {
            typed_response.prompt_tokens = Some(usage.prompt_tokens);
            typed_response.completion_tokens = Some(usage.completion_tokens);
            typed_response.total_tokens = Some(usage.total_tokens);
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
    struct TestResponse {
        value: String,
    }

    #[test]
    fn test_typed_chat_response_builder() -> crate::Result<()> {
        let response: TypedChatResponse<TestResponse> = TypedChatResponse::builder()
            .with_data(TestResponse {
                value: "test".to_string(),
            })
            .build()
            .map_err(|e| crate::Error::config(e.to_string()))?;

        assert_eq!(response.data.value, "test");
        assert!(response.raw_response.is_none());
        Ok(())
    }

    #[test]
    fn test_typed_chat_response_with_metadata() -> crate::Result<()> {
        let response: TypedChatResponse<TestResponse> = TypedChatResponse::builder()
            .with_data(TestResponse {
                value: "test".to_string(),
            })
            .with_raw_response("raw text")
            .with_prompt_tokens(10u32)
            .with_completion_tokens(20u32)
            .with_total_tokens(30u32)
            .build()
            .map_err(|e| crate::Error::config(e.to_string()))?;

        assert_eq!(response.prompt_tokens, Some(10));
        assert_eq!(response.completion_tokens, Some(20));
        assert_eq!(response.total_tokens, Some(30));
        Ok(())
    }

    #[test]
    fn test_parse_response_content() -> crate::Result<()> {
        let json = r#"{"value": "test"}"#;
        let data = TypedChatResponse::<TestResponse>::parse_response_content(json)?;
        assert_eq!(data.value, "test");
        Ok(())
    }

    #[test]
    fn test_parse_response_content_with_markdown() -> crate::Result<()> {
        let json = "```json\n{\"value\": \"test\"}\n```";
        let data = TypedChatResponse::<TestResponse>::parse_response_content(json)?;
        assert_eq!(data.value, "test");
        Ok(())
    }

    #[test]
    fn test_parse_response_content_with_extra_text() -> crate::Result<()> {
        let json = "Here is the response: {\"value\": \"test\"} - done";
        let data = TypedChatResponse::<TestResponse>::parse_response_content(json)?;
        assert_eq!(data.value, "test");
        Ok(())
    }
}
