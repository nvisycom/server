//! Typed chat completion response types.

use derive_builder::Builder;
use portkey_sdk::model::ChatCompletionResponse;
use serde::de::DeserializeOwned;

use crate::{Error, Result};

/// A typed chat completion response.
///
/// This wraps a chat completion response along with the parsed, typed response data
/// using portkey-sdk 0.2's built-in deserialization.
///
/// # Type Parameters
///
/// * `T` - The response type that implements `DeserializeOwned`
#[derive(Debug, Clone, Builder)]
#[builder(
    name = "TypedChatResponseBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
pub struct TypedChatResponse<T> {
    /// The raw chat completion response
    pub raw_response: ChatCompletionResponse,

    /// The parsed, typed response data
    pub response: T,

    /// The response message content
    pub content: String,
}

impl<T> TypedChatResponse<T>
where
    T: DeserializeOwned,
{
    /// Creates a new typed chat response builder.
    pub fn builder() -> TypedChatResponseBuilder<T> {
        TypedChatResponseBuilder::default()
    }

    /// Parses a Portkey chat completion response into a typed response.
    ///
    /// This method uses portkey-sdk 0.2's `deserialize_content` method for parsing.
    ///
    /// # Arguments
    ///
    /// * `response` - The raw chat completion response from Portkey
    ///
    /// # Returns
    ///
    /// A typed response containing both the raw response and parsed data
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The response has no choices
    /// - The response content cannot be extracted
    /// - The content cannot be parsed into type T
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use nvisy_portkey::completion::TypedChatResponse;
    /// use portkey_sdk::model::ChatCompletionResponse;
    /// use serde::Deserialize;
    /// use schemars::JsonSchema;
    ///
    /// #[derive(Deserialize, JsonSchema)]
    /// struct MyResponse {
    ///     answer: String,
    /// }
    ///
    /// # fn example(response: ChatCompletionResponse) -> Result<(), Box<dyn std::error::Error>> {
    /// let typed_response = TypedChatResponse::<MyResponse>::from_response(response)?;
    /// println!("Answer: {}", typed_response.response.answer);
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_response(response: ChatCompletionResponse) -> Result<Self> {
        // Extract the first choice
        let choice = response
            .choices
            .first()
            .ok_or_else(|| Error::invalid_response("No choices in response"))?;

        // Get content string for storage
        let content = choice
            .message
            .content
            .clone()
            .ok_or_else(|| Error::invalid_response("No content in response message"))?;

        // Use portkey-sdk 0.2's deserialize_content method
        let parsed_response = choice
            .message
            .deserialize_content::<T>()?
            .ok_or_else(|| Error::invalid_response("Failed to deserialize content"))?;

        Ok(Self {
            raw_response: response,
            response: parsed_response,
            content,
        })
    }

    /// Gets a reference to the raw chat completion response.
    pub fn raw(&self) -> &ChatCompletionResponse {
        &self.raw_response
    }

    /// Gets a reference to the parsed response data.
    pub fn data(&self) -> &T {
        &self.response
    }

    /// Gets the response message content as a string.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Consumes self and returns the parsed response data.
    pub fn into_data(self) -> T {
        self.response
    }
}

#[cfg(test)]
mod tests {
    use portkey_sdk::model::{ChatCompletionChoice, ChatCompletionResponseMessage, Usage};
    use schemars::JsonSchema;
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize, JsonSchema, PartialEq)]
    struct TestResponse {
        answer: String,
        confidence: f64,
    }

    fn create_mock_response(content: String) -> ChatCompletionResponse {
        ChatCompletionResponse {
            id: "test-id".to_string(),
            model: "gpt-4".to_string(),
            choices: vec![ChatCompletionChoice {
                index: 0,
                message: ChatCompletionResponseMessage {
                    role: "assistant".to_string(),
                    content: Some(content),
                    tool_calls: None,
                    content_blocks: None,
                    function_call: None,
                },
                finish_reason: "stop".to_string(),
                logprobs: None,
            }],
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
            }),
            created: 1234567890,
            object: "chat.completion".to_string(),
            system_fingerprint: None,
        }
    }

    #[test]
    fn test_from_response_with_json() {
        let json_content = r#"{"answer": "42", "confidence": 0.95}"#;
        let response = create_mock_response(json_content.to_string());

        let typed_response = TypedChatResponse::<TestResponse>::from_response(response);
        assert!(typed_response.is_ok());

        let typed = typed_response.unwrap();
        assert_eq!(typed.response.answer, "42");
        assert_eq!(typed.response.confidence, 0.95);
    }

    #[test]
    fn test_from_response_no_choices() {
        let mut response = create_mock_response("test".to_string());
        response.choices.clear();

        let result = TypedChatResponse::<TestResponse>::from_response(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_accessors() {
        let json_content = r#"{"answer": "test", "confidence": 0.8}"#;
        let response = create_mock_response(json_content.to_string());

        let typed = TypedChatResponse::<TestResponse>::from_response(response).unwrap();

        assert_eq!(typed.data().answer, "test");
        assert_eq!(typed.content(), json_content);
        assert_eq!(typed.raw().id, "test-id");

        let data = typed.into_data();
        assert_eq!(data.answer, "test");
    }
}
