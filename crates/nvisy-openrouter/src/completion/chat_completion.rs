//! Typed chat completion handler.

use std::marker::PhantomData;

use openrouter_rs::types::ResponseFormat;
use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::chat_request::TypedChatRequest;
use super::chat_response::TypedChatResponse;
use crate::client::LlmClient;
use crate::{Error, Result, SCHEMA_TARGET};

/// A typed chat completion handler that enforces request and response schemas.
///
/// This struct provides a type-safe interface for chat completions, ensuring
/// that requests are properly serialized and responses are validated against
/// a JSON schema.
///
/// # Type Parameters
///
/// * `T` - The request type that implements `Serialize`
/// * `U` - The response type that implements `JsonSchema + DeserializeOwned`
///
/// # Example
///
/// ```rust
/// use nvisy_openrouter::{LlmClient, completion::TypedChatCompletion};
/// use serde::{Deserialize, Serialize};
/// use schemars::JsonSchema;
///
/// #[derive(Serialize)]
/// struct MyRequest {
///     query: String,
/// }
///
/// #[derive(Serialize, Deserialize, JsonSchema)]
/// struct MyResponse {
///     answer: String,
/// }
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = LlmClient::from_api_key("your-api-key")?;
/// let completion = TypedChatCompletion::<MyRequest, MyResponse>::new(client);
/// # Ok(())
/// # }
/// ```
pub struct TypedChatCompletion<T, U>
where
    T: Serialize,
    U: JsonSchema + DeserializeOwned,
{
    client: LlmClient,
    _phantom_request: PhantomData<T>,
    _phantom_response: PhantomData<U>,
}

impl<T, U> TypedChatCompletion<T, U>
where
    T: Serialize,
    U: JsonSchema + DeserializeOwned,
{
    /// Creates a new typed chat completion handler.
    ///
    /// # Arguments
    ///
    /// * `client` - The LLM client to use for completions
    ///
    /// # Example
    ///
    /// ```rust
    /// use nvisy_openrouter::{LlmClient, completion::TypedChatCompletion};
    /// use serde::{Deserialize, Serialize};
    /// use schemars::JsonSchema;
    ///
    /// #[derive(Serialize)]
    /// struct Request { query: String }
    ///
    /// #[derive(Serialize, Deserialize, JsonSchema)]
    /// struct Response { answer: String }
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = LlmClient::from_api_key("key")?;
    /// let completion = TypedChatCompletion::<Request, Response>::new(client);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(client: LlmClient) -> Self {
        Self {
            client,
            _phantom_request: PhantomData,
            _phantom_response: PhantomData,
        }
    }

    /// Performs a typed chat completion.
    ///
    /// This method sends a typed request to the LLM and returns a typed response,
    /// with automatic schema generation and validation.
    ///
    /// # Arguments
    ///
    /// * `request` - The typed chat request
    ///
    /// # Returns
    ///
    /// A typed chat response containing the parsed data
    ///
    /// # Errors
    ///
    /// - [`Error::Api`]: If the API request fails or schema generation fails
    /// - [`Error::RateLimit`]: If rate limit is exceeded
    ///
    /// # Example
    ///
    /// ```rust
    /// # use nvisy_openrouter::{LlmClient, completion::{TypedChatCompletion, TypedChatRequest}};
    /// # use serde::{Deserialize, Serialize};
    /// # use schemars::JsonSchema;
    /// # use openrouter_rs::{api::chat::Message, types::Role};
    /// #
    /// # #[derive(Serialize)]
    /// # struct Request { query: String }
    /// #
    /// # #[derive(Serialize, Deserialize, JsonSchema)]
    /// # struct Response { answer: String }
    /// #
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = LlmClient::from_api_key("key")?;
    /// let completion = TypedChatCompletion::<Request, Response>::new(client);
    ///
    /// let request = TypedChatRequest::builder()
    ///     .with_messages(vec![Message::new(Role::User, "Hello")])
    ///     .with_request(Request { query: "test".to_string() })
    ///     .build()?;
    ///
    /// let response = completion.chat_completion(request).await?;
    /// println!("Answer: {}", response.data.answer);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn chat_completion(
        &self,
        request: TypedChatRequest<T>,
    ) -> Result<TypedChatResponse<U>> {
        let response_format = self.generate_response_schema()?;

        let llm_response = self
            .client
            .send_request(|client, config| {
                let client = client.clone();
                let chat_request_result = request.build_chat_request(config, response_format);
                async move {
                    let chat_request = chat_request_result?;
                    client
                        .send_chat_completion(&chat_request)
                        .await
                        .map_err(Into::into)
                }
            })
            .await?;

        TypedChatResponse::<U>::from_llm_response(&llm_response)
    }

    /// Returns the underlying LLM client.
    ///
    /// This can be useful for accessing the client configuration or
    /// performing other operations with the same client instance.
    ///
    /// # Returns
    ///
    /// A clone of the LLM client
    pub fn client(&self) -> LlmClient {
        self.client.clone()
    }

    /// Generates the JSON schema for the response type.
    ///
    /// This method creates a JSON schema from the response type's `JsonSchema`
    /// implementation and wraps it in the appropriate `ResponseFormat` for
    /// OpenRouter's structured output feature.
    ///
    /// # Returns
    ///
    /// A `ResponseFormat` containing the JSON schema
    ///
    /// # Errors
    ///
    /// Returns an error if the schema cannot be serialized to JSON
    fn generate_response_schema(&self) -> Result<ResponseFormat> {
        let schema_name = std::any::type_name::<U>()
            .split("::")
            .last()
            .unwrap_or("response_schema");

        tracing::debug!(
            target: SCHEMA_TARGET,
            schema_name = %schema_name,
            "Generating JSON schema for typed completion"
        );

        let schema = schema_for!(U);
        let json_value = serde_json::to_value(&schema).map_err(|e| {
            tracing::error!(
                target: SCHEMA_TARGET,
                error = %e,
                schema_name = %schema_name,
                "Failed to serialize JSON schema"
            );
            Error::Serialization(e)
        })?;

        let response_format = ResponseFormat::json_schema(schema_name, true, json_value);

        Ok(response_format)
    }
}

impl<T, U> Clone for TypedChatCompletion<T, U>
where
    T: Serialize,
    U: JsonSchema + DeserializeOwned,
{
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            _phantom_request: PhantomData,
            _phantom_response: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use openrouter_rs::api::chat::Message;
    use openrouter_rs::types::Role;
    use serde::Deserialize;

    use super::*;

    #[derive(Serialize, Deserialize)]
    struct TestRequest {
        query: String,
    }

    #[derive(Serialize, Deserialize, JsonSchema, PartialEq, Debug)]
    struct TestResponse {
        answer: String,
    }

    #[test]
    fn test_typed_chat_completion_creation() {
        let client = crate::LlmClient::from_api_key("test-key").unwrap();
        let completion = TypedChatCompletion::<TestRequest, TestResponse>::new(client.clone());

        // Verify we can get the client back
        let retrieved_client = completion.client();
        assert_eq!(
            retrieved_client.config().effective_model(),
            client.config().effective_model()
        );
    }

    #[test]
    fn test_generate_response_schema() -> Result<()> {
        let client = crate::LlmClient::from_api_key("test-key")?;
        let completion = TypedChatCompletion::<TestRequest, TestResponse>::new(client);

        let _schema = completion.generate_response_schema()?;

        // Verify the schema is created (we can't easily check the exact format)
        // but we can verify it doesn't error
        Ok(())
    }

    #[test]
    fn test_build_chat_request_with_defaults() -> Result<()> {
        let client = crate::LlmClient::from_api_key("test-key")?;
        let _completion = TypedChatCompletion::<TestRequest, TestResponse>::new(client.clone());

        let request: TypedChatRequest<TestRequest> = TypedChatRequest::builder()
            .with_messages(vec![Message::new(Role::User, "Hello")])
            .with_request(TestRequest {
                query: "test".to_string(),
            })
            .build()
            .unwrap();

        let response_format = ResponseFormat::json_schema("test", true, serde_json::json!({}));

        let _chat_request = request.build_chat_request(client.config(), response_format)?;

        // Verify request is built correctly (doesn't panic)
        Ok(())
    }

    #[test]
    fn test_build_chat_request_with_overrides() -> Result<()> {
        let config = crate::LlmConfig::builder()
            .with_default_temperature(0.5)
            .with_default_max_tokens(100u32)
            .build()
            .unwrap();
        let client = crate::LlmClient::from_api_key_with_config("test-key", config)?;
        let _completion = TypedChatCompletion::<TestRequest, TestResponse>::new(client.clone());

        let request: TypedChatRequest<TestRequest> = TypedChatRequest::builder()
            .with_messages(vec![Message::new(Role::User, "Hello")])
            .with_request(TestRequest {
                query: "test".to_string(),
            })
            .with_temperature(0.9)
            .with_max_tokens(500u32)
            .with_model("custom-model")
            .build()
            .unwrap();

        let response_format = ResponseFormat::json_schema("test", true, serde_json::json!({}));

        let _chat_request = request.build_chat_request(client.config(), response_format)?;

        // Verify request is built correctly with overrides (doesn't panic)
        // We can't easily test the actual values since ChatCompletionRequest fields are private
        Ok(())
    }

    #[test]
    fn test_clone() {
        let client = crate::LlmClient::from_api_key("test-key").unwrap();
        let completion = TypedChatCompletion::<TestRequest, TestResponse>::new(client);

        let cloned = completion.clone();

        // Verify both have the same config
        assert_eq!(
            completion.client().config().effective_model(),
            cloned.client().config().effective_model()
        );
    }
}
