//! Typed chat completion request types.

use std::borrow::Cow;

use derive_builder::Builder;
use portkey_sdk::model::{ChatCompletionRequest, ChatCompletionRequestMessage, ResponseFormat};
use schemars::JsonSchema;
use serde::Serialize;

use crate::Result;
use crate::client::LlmConfig;

/// A typed chat completion request with structured output support.
///
/// This wraps messages along with a typed request payload and automatic
/// JSON Schema generation for structured responses.
///
/// # Type Parameters
///
/// * `Req` - The request payload type that implements `Serialize`
/// * `Res` - The expected response type that implements `Deserialize` and `JsonSchema`
#[derive(Debug, Clone, Builder, Serialize)]
#[builder(
    name = "TypedChatRequestBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with")
)]
pub struct TypedChatRequest<Req, Res> {
    /// The conversation messages
    pub messages: Vec<ChatCompletionRequestMessage>,

    /// The typed request payload
    pub request: Req,

    /// Optional system prompt override
    #[builder(default)]
    pub system_prompt: Option<Cow<'static, str>>,

    /// Optional model override
    #[builder(default)]
    pub model: Option<Cow<'static, str>>,

    /// Optional temperature override
    #[builder(default)]
    pub temperature: Option<f64>,

    /// Optional max tokens override
    #[builder(default)]
    pub max_tokens: Option<u32>,

    /// Optional JSON schema description
    #[builder(default)]
    pub schema_description: Option<Cow<'static, str>>,

    /// Whether to use strict JSON schema validation
    #[builder(default = "true")]
    pub strict_schema: bool,

    /// Phantom data for response type
    #[serde(skip)]
    #[builder(setter(skip))]
    _phantom: std::marker::PhantomData<Res>,
}

impl<Req, Res> TypedChatRequest<Req, Res>
where
    Res: JsonSchema,
{
    /// Creates a new typed chat request builder.
    pub fn builder() -> TypedChatRequestBuilder<Req, Res> {
        TypedChatRequestBuilder::default()
    }

    /// Builds a Portkey chat completion request with JSON Schema response format.
    ///
    /// This method:
    /// - Prepares the messages (including system prompt if present)
    /// - Applies the model from the request or uses the default from config
    /// - Generates JSON Schema from the response type
    /// - Configures structured output using ResponseFormat::JsonSchema
    /// - Applies configuration defaults and request-specific overrides
    ///
    /// # Arguments
    ///
    /// * `config` - The LLM configuration to use for defaults
    ///
    /// # Returns
    ///
    /// A configured `ChatCompletionRequest` with JSON Schema response format
    ///
    /// # Errors
    ///
    /// Returns an error if the request cannot be built
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use nvisy_portkey::completion::TypedChatRequest;
    /// use nvisy_portkey::{LlmConfig, Result};
    /// use portkey_sdk::model::ChatCompletionRequestMessage;
    /// use serde::{Serialize, Deserialize};
    /// use schemars::JsonSchema;
    ///
    /// #[derive(Serialize)]
    /// struct Request { query: String }
    ///
    /// #[derive(Deserialize, JsonSchema)]
    /// struct Response { answer: String }
    ///
    /// # fn example() -> Result<()> {
    /// let config = LlmConfig::builder()
    ///     .with_api_key("test-key")
    ///     .with_default_model("gpt-4o")
    ///     .build()?;
    ///
    /// let request: TypedChatRequest<Request, Response> = TypedChatRequest::builder()
    ///     .with_messages(vec![ChatCompletionRequestMessage::user("What is 2+2?")])
    ///     .with_request(Request { query: "math".to_string() })
    ///     .with_schema_description("A mathematical answer")
    ///     .build()?;
    ///
    /// let chat_request = request.build_chat_request(&config)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn build_chat_request(&self, config: &LlmConfig) -> Result<ChatCompletionRequest> {
        let mut messages = Vec::new();

        // Add system prompt if present
        if let Some(system_prompt) = &self.system_prompt {
            messages.push(ChatCompletionRequestMessage::System {
                content: system_prompt.to_string(),
                name: None,
            });
        }

        // Add conversation messages
        messages.extend(self.messages.clone());

        // Determine the model to use
        let model = self
            .model
            .as_ref()
            .map(|m| m.to_string())
            .or_else(|| config.default_model().map(|m| m.to_string()))
            .unwrap_or_else(|| config.effective_model().to_string());

        // Build the request using the new constructor
        let mut request = ChatCompletionRequest::new(model, messages);

        // Generate JSON Schema for structured output
        let json_schema = portkey_sdk::model::JsonSchema::from_type::<Res>()
            .with_description(
                self.schema_description
                    .as_ref()
                    .map(|s| s.as_ref())
                    .unwrap_or("Structured response schema"),
            )
            .with_strict(self.strict_schema);

        // Set the response format
        request.response_format = Some(ResponseFormat::JsonSchema { json_schema });

        // Set optional parameters
        request.temperature = self
            .temperature
            .or(config.default_temperature())
            .map(|v| v as f32);
        request.max_tokens = self
            .max_tokens
            .or(config.default_max_tokens())
            .map(|v| v as i32);
        request.top_p = config.default_top_p().map(|v| v as f32);
        request.frequency_penalty = config.default_frequency_penalty().map(|v| v as f32);
        request.presence_penalty = config.default_presence_penalty().map(|v| v as f32);

        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Serialize)]
    struct TestRequest {
        query: String,
    }

    #[derive(Debug, Deserialize, JsonSchema)]
    struct TestResponse {
        answer: String,
    }

    #[test]
    fn test_typed_request_builder() {
        let request: TypedChatRequest<TestRequest, TestResponse> = TypedChatRequest::builder()
            .with_messages(vec![ChatCompletionRequestMessage::user("Hello")])
            .with_request(TestRequest {
                query: "test".to_string(),
            })
            .build()
            .unwrap();

        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.request.query, "test");
        assert!(request.strict_schema);
    }

    #[test]
    fn test_build_chat_request_with_defaults() {
        let config = LlmConfig::builder()
            .with_api_key("test-key")
            .with_default_model("gpt-4o")
            .build()
            .unwrap();

        let request: TypedChatRequest<TestRequest, TestResponse> = TypedChatRequest::builder()
            .with_messages(vec![ChatCompletionRequestMessage::user("Hello")])
            .with_request(TestRequest {
                query: "test".to_string(),
            })
            .build()
            .unwrap();

        let chat_request = request.build_chat_request(&config).unwrap();

        assert_eq!(chat_request.model, "gpt-4o");
        assert_eq!(chat_request.messages.len(), 1);
        assert!(chat_request.response_format.is_some());
    }

    #[test]
    fn test_build_chat_request_with_system_prompt() {
        let config = LlmConfig::builder()
            .with_api_key("test-key")
            .build()
            .unwrap();

        let request: TypedChatRequest<TestRequest, TestResponse> = TypedChatRequest::builder()
            .with_messages(vec![ChatCompletionRequestMessage::user("Hello")])
            .with_request(TestRequest {
                query: "test".to_string(),
            })
            .with_system_prompt("You are a helpful assistant")
            .build()
            .unwrap();

        let chat_request = request.build_chat_request(&config).unwrap();

        // Should have system message + user message
        assert_eq!(chat_request.messages.len(), 2);
        if let ChatCompletionRequestMessage::System { content, .. } = &chat_request.messages[0] {
            assert_eq!(content, "You are a helpful assistant");
        } else {
            panic!("Expected System message");
        }
    }

    #[test]
    fn test_build_with_schema_description() {
        let config = LlmConfig::builder()
            .with_api_key("test-key")
            .build()
            .unwrap();

        let request: TypedChatRequest<TestRequest, TestResponse> = TypedChatRequest::builder()
            .with_messages(vec![ChatCompletionRequestMessage::user("Hello")])
            .with_request(TestRequest {
                query: "test".to_string(),
            })
            .with_schema_description("A test response schema")
            .build()
            .unwrap();

        let chat_request = request.build_chat_request(&config).unwrap();
        assert!(chat_request.response_format.is_some());
    }
}
