//! Typed chat completion request types.

use derive_builder::Builder;
use openrouter_rs::api::chat::{ChatCompletionRequest, Message};
use openrouter_rs::types::{ResponseFormat, Role};
use serde::Serialize;

use crate::client::LlmConfig;
use crate::error::Error;

/// A typed chat completion request.
///
/// This wraps messages along with a typed request payload that will be
/// included in the conversation for structured prompting.
///
/// # Type Parameters
///
/// * `T` - The request payload type that implements `Serialize`
///
/// # Example
///
/// ```rust
/// use nvisy_openrouter::completion::TypedChatRequest;
/// use serde::Serialize;
/// use openrouter_rs::{api::chat::Message, types::Role};
///
/// #[derive(Serialize)]
/// struct MyRequest {
///     query: String,
/// }
///
/// let request: TypedChatRequest<MyRequest> = TypedChatRequest::builder()
///     .with_messages(vec![Message::new(Role::User, "Hello")])
///     .with_request(MyRequest { query: "test".to_string() })
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, Builder, Serialize)]
#[builder(pattern = "owned", setter(into, strip_option, prefix = "with"))]
pub struct TypedChatRequest<T>
where
    T: Serialize,
{
    /// The conversation messages
    pub messages: Vec<Message>,

    /// The typed request payload
    pub request: T,

    /// Optional system prompt override
    #[builder(default)]
    pub system_prompt: Option<String>,

    /// Optional model override
    #[builder(default)]
    pub model: Option<String>,

    /// Optional temperature override
    #[builder(default)]
    pub temperature: Option<f32>,

    /// Optional max tokens override
    #[builder(default)]
    pub max_tokens: Option<u32>,
}

impl<T> TypedChatRequest<T>
where
    T: Serialize,
{
    /// Creates a new typed chat request builder.
    pub fn builder() -> TypedChatRequestBuilder<T> {
        TypedChatRequestBuilder::default()
    }

    /// Builds an OpenRouter chat completion request from this typed request.
    ///
    /// This method:
    /// - Prepares the messages (including system prompt if present)
    /// - Applies the model from the request or uses the default from config
    /// - Sets the response format for structured output
    /// - Applies configuration defaults
    /// - Applies request-specific overrides (temperature, max_tokens)
    ///
    /// # Arguments
    ///
    /// * `config` - The LLM configuration to use for defaults
    /// * `response_format` - The JSON schema format for the response
    ///
    /// # Returns
    ///
    /// A configured `ChatCompletionRequest` ready to send
    ///
    /// # Errors
    ///
    /// Returns an error if the request cannot be built
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::TypedChatRequest;
    /// use nvisy_openrouter::LlmConfig;
    /// use openrouter_rs::{api::chat::Message, types::{Role, ResponseFormat}};
    /// use serde::Serialize;
    ///
    /// #[derive(Serialize)]
    /// struct Request { query: String }
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = LlmConfig::default();
    /// let request: TypedChatRequest<Request> = TypedChatRequest::builder()
    ///     .with_messages(vec![Message::new(Role::User, "Hello")])
    ///     .with_request(Request { query: "test".to_string() })
    ///     .build()?;
    ///
    /// let response_format = ResponseFormat::json_schema("test", true, serde_json::json!({}));
    /// let chat_request = request.build_chat_request(&config, response_format)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn build_chat_request(
        &self,
        config: &LlmConfig,
        response_format: ResponseFormat,
    ) -> Result<ChatCompletionRequest, Error> {
        let mut builder = ChatCompletionRequest::builder();

        let model = self
            .model
            .as_deref()
            .unwrap_or_else(|| config.effective_model());

        let messages = self.prepare_messages();

        builder
            .model(model)
            .messages(messages)
            .response_format(response_format);

        // Apply config defaults first
        if let Some(max_tokens) = config.default_max_tokens {
            builder.max_tokens(max_tokens);
        }

        if let Some(temperature) = config.default_temperature {
            builder.temperature(temperature as f64);
        }

        if let Some(top_p) = config.default_top_p {
            builder.top_p(top_p as f64);
        }

        if let Some(presence_penalty) = config.default_presence_penalty {
            builder.presence_penalty(presence_penalty as f64);
        }

        if let Some(frequency_penalty) = config.default_frequency_penalty {
            builder.frequency_penalty(frequency_penalty as f64);
        }

        // Request-specific overrides take precedence over config defaults
        if let Some(temperature) = self.temperature {
            builder.temperature(temperature as f64);
        }

        if let Some(max_tokens) = self.max_tokens {
            builder.max_tokens(max_tokens);
        }

        builder
            .build()
            .map_err(|e| Error::api(format!("Failed to build chat completion request: {}", e)))
    }

    /// Prepares the final message list for the chat completion request.
    ///
    /// This method:
    /// - Clones the messages from the request
    /// - Inserts the system prompt at the beginning if provided
    ///
    /// # Returns
    ///
    /// A vector of messages ready to be sent to the LLM
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::TypedChatRequest;
    /// use openrouter_rs::{api::chat::Message, types::Role};
    /// use serde::Serialize;
    ///
    /// #[derive(Serialize)]
    /// struct Request { query: String }
    ///
    /// let request: TypedChatRequest<Request> = TypedChatRequest::builder()
    ///     .with_messages(vec![Message::new(Role::User, "Hello")])
    ///     .with_request(Request { query: "test".to_string() })
    ///     .with_system_prompt("You are helpful")
    ///     .build()
    ///     .unwrap();
    ///
    /// let messages = request.prepare_messages();
    /// assert_eq!(messages.len(), 2); // System + User
    /// ```
    pub fn prepare_messages(&self) -> Vec<Message> {
        let mut messages = self.messages.clone();

        if let Some(system_prompt) = &self.system_prompt {
            messages.insert(0, Message::new(Role::System, system_prompt));
        }

        messages
    }
}

#[cfg(test)]
mod tests {
    use openrouter_rs::types::Role;
    use serde::Deserialize;

    use super::*;

    #[derive(Serialize, Deserialize)]
    struct TestRequest {
        value: String,
    }

    #[test]
    fn test_typed_chat_request_builder() -> crate::Result<()> {
        let request: TypedChatRequest<TestRequest> = TypedChatRequest::builder()
            .with_messages(vec![Message::new(Role::User, "test")])
            .with_request(TestRequest {
                value: "test".to_string(),
            })
            .build()
            .map_err(|e| crate::Error::builder(e.to_string()))?;

        assert_eq!(request.messages.len(), 1);
        assert!(request.system_prompt.is_none());
        Ok(())
    }

    #[test]
    fn test_typed_chat_request_with_overrides() -> crate::Result<()> {
        let request: TypedChatRequest<TestRequest> = TypedChatRequest::builder()
            .with_messages(vec![Message::new(Role::User, "test")])
            .with_request(TestRequest {
                value: "test".to_string(),
            })
            .with_system_prompt("Custom system prompt")
            .with_model("custom-model")
            .with_temperature(0.8f32)
            .with_max_tokens(1000u32)
            .build()
            .map_err(|e| crate::Error::builder(e.to_string()))?;

        assert_eq!(
            request.system_prompt.as_deref(),
            Some("Custom system prompt")
        );
        assert_eq!(request.model.as_deref(), Some("custom-model"));
        assert_eq!(request.temperature, Some(0.8));
        assert_eq!(request.max_tokens, Some(1000));
        Ok(())
    }

    #[test]
    fn test_prepare_messages_without_system_prompt() -> crate::Result<()> {
        let request: TypedChatRequest<TestRequest> = TypedChatRequest::builder()
            .with_messages(vec![
                Message::new(Role::User, "Hello"),
                Message::new(Role::Assistant, "Hi there"),
            ])
            .with_request(TestRequest {
                value: "test".to_string(),
            })
            .build()
            .map_err(|e| crate::Error::builder(e.to_string()))?;

        let messages = request.prepare_messages();

        // Verify no system prompt was added
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "Hello");
        assert_eq!(messages[1].content, "Hi there");
        Ok(())
    }

    #[test]
    fn test_prepare_messages_with_system_prompt() -> crate::Result<()> {
        let request: TypedChatRequest<TestRequest> = TypedChatRequest::builder()
            .with_messages(vec![Message::new(Role::User, "Hello")])
            .with_request(TestRequest {
                value: "test".to_string(),
            })
            .with_system_prompt("You are a helpful assistant")
            .build()
            .map_err(|e| crate::Error::builder(e.to_string()))?;

        let messages = request.prepare_messages();

        // Verify system prompt was inserted at the beginning
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "You are a helpful assistant");
        assert_eq!(messages[1].content, "Hello");
        Ok(())
    }
}
