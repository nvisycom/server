//! Typed chat completion response types.

use derive_builder::Builder;
use openrouter_rs::types::completion::CompletionsResponse;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::Result;
use crate::typed::{extract_response_content, parse_json_response};

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
    pub fn from_llm_response(response: &CompletionsResponse) -> Result<Self> {
        // Extract content from the first choice with content
        let content = extract_response_content(response)?;

        // Parse the content into typed data
        let data = parse_json_response::<T>(&content)?;
        let mut this = Self::builder().with_data(data).with_raw_response(content);

        if let Some(ref usage) = response.usage {
            this = this
                .with_prompt_tokens(usage.prompt_tokens)
                .with_completion_tokens(usage.completion_tokens)
                .with_total_tokens(usage.total_tokens);
        }

        Ok(this.build()?)
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
            .build()?;

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
            .build()?;
        assert_eq!(response.prompt_tokens, Some(10));
        assert_eq!(response.completion_tokens, Some(20));
        assert_eq!(response.total_tokens, Some(30));
        Ok(())
    }

    #[test]
    fn test_parse_response_content() -> crate::Result<()> {
        let json = r#"{"value": "test"}"#;
        let data = parse_json_response::<TestResponse>(json)?;
        assert_eq!(data.value, "test");
        Ok(())
    }

    #[test]
    fn test_parse_response_content_with_markdown() -> crate::Result<()> {
        let json = "```json\n{\"value\": \"test\"}\n```";
        let data = parse_json_response::<TestResponse>(json)?;
        assert_eq!(data.value, "test");
        Ok(())
    }

    #[test]
    fn test_parse_response_content_with_extra_text() -> crate::Result<()> {
        let json = "Here is the response: {\"value\": \"test\"} - done";
        let data = parse_json_response::<TestResponse>(json)?;
        assert_eq!(data.value, "test");
        Ok(())
    }
}
