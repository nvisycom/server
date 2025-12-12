//! Chat completion orchestration.
//!
//! This module provides high-level abstractions for chat completions with structured output support.

use std::future::Future;

use portkey_sdk::model::{ChatCompletionRequest, ChatCompletionResponse};
use portkey_sdk::service::ChatService;
use schemars::JsonSchema;
use serde::Deserialize;

use super::ChatContext;
use crate::client::LlmClient;
use crate::{Result, TRACING_TARGET_COMPLETION};

/// Trait for performing chat completions with structured output.
///
/// This trait provides methods for both untyped and typed (structured) chat completions
/// that automatically manage conversation context and token usage tracking.
///
/// # Examples
///
/// ## Basic Chat Completion
///
/// ```rust,no_run
/// use nvisy_portkey::{LlmClient, completion::{ChatCompletion, ChatContext}};
/// use portkey_sdk::model::{ChatCompletionRequest, ChatCompletionRequestMessage};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = LlmClient::from_api_key("your-api-key")?;
/// let mut context = ChatContext::new("You are a helpful assistant");
/// context.add_user_message("What is 2+2?");
///
/// let request = ChatCompletionRequest::new(
///     "gpt-4o",
///     context.to_messages()
/// );
///
/// let response = client.chat_completion(&mut context, request).await?;
/// println!("Response: {:?}", response.choices.first().unwrap().message.content);
/// # Ok(())
/// # }
/// ```
///
/// ## Structured Output with JSON Schema
///
/// ```rust,no_run
/// use nvisy_portkey::{LlmClient, completion::{ChatCompletion, ChatContext}};
/// use portkey_sdk::model::{ChatCompletionRequest, ChatCompletionRequestMessage, ResponseFormat};
/// use serde::{Serialize, Deserialize};
/// use schemars::JsonSchema;
///
/// #[derive(Serialize, Deserialize, JsonSchema, Debug)]
/// struct MovieRecommendation {
///     title: String,
///     year: u16,
///     rating: f32,
///     genre: String,
///     reason: String,
/// }
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = LlmClient::from_api_key("your-api-key")?;
/// let mut context = ChatContext::new("You are a movie expert");
/// context.add_user_message("Recommend a great sci-fi movie from the 1980s");
///
/// let mut request = ChatCompletionRequest::new(
///     "gpt-4o",
///     context.to_messages()
/// );
///
/// // Configure structured output using JSON Schema
/// request.response_format = Some(ResponseFormat::JsonSchema {
///     json_schema: ResponseFormat::json_schema::<MovieRecommendation>()
///         .with_description("A movie recommendation with details")
///         .with_strict(true),
/// });
///
/// let response = client.structured_chat_completion::<MovieRecommendation>(
///     &mut context,
///     request
/// ).await?;
///
/// if let Some(movie) = response {
///     println!("Title: {}", movie.title);
///     println!("Year: {}", movie.year);
/// }
/// # Ok(())
/// # }
/// ```
pub trait ChatCompletion {
    /// Executes a chat completion with a raw request.
    ///
    /// This method automatically updates the context with the assistant's response
    /// and tracks token usage.
    ///
    /// # Arguments
    ///
    /// * `context` - Mutable reference to the chat context (updated with response and usage)
    /// * `request` - The chat completion request
    ///
    /// # Returns
    ///
    /// The raw chat completion response from the API
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails
    fn chat_completion(
        &self,
        context: &mut ChatContext,
        request: ChatCompletionRequest,
    ) -> impl Future<Output = Result<ChatCompletionResponse>> + Send;

    /// Executes a structured chat completion with automatic JSON schema parsing.
    ///
    /// This method:
    /// 1. Sends the request to the API with JSON Schema response format configured
    /// 2. Receives and parses the structured JSON response
    /// 3. Deserializes into the target type
    /// 4. Updates the context with the assistant's response and token usage
    ///
    /// The request should already have `response_format` configured with `ResponseFormat::JsonSchema`.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The response type that implements `Deserialize` and `JsonSchema`
    ///
    /// # Arguments
    ///
    /// * `context` - Mutable reference to the chat context (updated with response and usage)
    /// * `request` - The chat completion request with response_format configured
    ///
    /// # Returns
    ///
    /// An optional typed response (None if no content in response)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The API request fails
    /// - The response cannot be deserialized into type T
    fn structured_chat_completion<T>(
        &self,
        context: &mut ChatContext,
        request: ChatCompletionRequest,
    ) -> impl Future<Output = Result<Option<T>>> + Send
    where
        T: for<'de> Deserialize<'de> + JsonSchema + Send;
}

impl ChatCompletion for LlmClient {
    async fn chat_completion(
        &self,
        context: &mut ChatContext,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse> {
        tracing::debug!(
            target: TRACING_TARGET_COMPLETION,
            model = request.model,
            message_count = request.messages.len(),
            "Starting chat completion"
        );

        // Send the request using the underlying Portkey client
        let response = self.as_client().create_chat_completion(request).await?;

        tracing::info!(
            target: TRACING_TARGET_COMPLETION,
            response_id = response.id,
            "Chat completion successful"
        );

        // Extract assistant message and update context
        if let Some(choice) = response.choices.first() {
            if let Some(content) = &choice.message.content {
                context.add_assistant_message(content.clone());
            }
        }

        // Update usage tracking
        if let Some(usage) = &response.usage {
            context.update_usage(usage.clone());
        }

        Ok(response)
    }

    async fn structured_chat_completion<T>(
        &self,
        context: &mut ChatContext,
        request: ChatCompletionRequest,
    ) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de> + JsonSchema + Send,
    {
        tracing::debug!(
            target: TRACING_TARGET_COMPLETION,
            model = request.model,
            message_count = request.messages.len(),
            "Starting structured chat completion"
        );

        // Send the request using chat_completion
        let response = self.chat_completion(context, request).await?;

        tracing::debug!(
            target: TRACING_TARGET_COMPLETION,
            response_id = response.id,
            "Received structured response, parsing..."
        );

        // Use the deserialize_content method from portkey-sdk 0.2
        if let Some(choice) = response.choices.first() {
            let parsed = choice.message.deserialize_content::<T>()?;

            tracing::info!(
                target: TRACING_TARGET_COMPLETION,
                "Structured chat completion parsed successfully"
            );

            Ok(parsed)
        } else {
            tracing::warn!(
                target: TRACING_TARGET_COMPLETION,
                "No choices in response"
            );
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Serialize, Deserialize, JsonSchema)]
    struct TestResponse {
        answer: String,
    }

    #[test]
    fn test_chat_completion_trait_exists() {
        // Verify the trait is properly defined and can be used
        fn assert_implements_trait<T: ChatCompletion>(_: &T) {}

        let config = crate::client::LlmConfig::builder()
            .with_api_key("test-key")
            .with_virtual_key("test-virtual-key")
            .with_default_model("gpt-4")
            .build()
            .unwrap();
        let client = LlmClient::new(config).unwrap();
        assert_implements_trait(&client);
    }
}
