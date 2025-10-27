//! OpenRouter API client implementation.
//!
//! This module provides the main Client for interacting with OpenRouter's API,
//! including chat completions, model information, and rate limiting.

use std::num::NonZeroU32;
use std::sync::Arc;

use governor::clock::{Clock, MonotonicClock};
use governor::middleware::NoOpMiddleware;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};
use openrouter_api::{
    ChatCompletionRequest, ChatCompletionResponse, Message, ModelInfo, OpenRouterClient, Ready,
};

use super::config::LlmConfig;
use super::error::{Error, Result, convert_api_error};
use crate::OPENROUTER_TARGET;

/// OpenRouter API client with rate limiting and configuration.
///
/// This client provides a high-level interface to the OpenRouter API with built-in
/// rate limiting, error handling, and observability features.
///
/// # Features
///
/// - **Rate Limiting**: Automatic rate limiting to prevent API quota exhaustion
/// - **Error Handling**: Comprehensive error types with recovery strategies
/// - **Observability**: Structured logging and health monitoring
/// - **Configuration**: Flexible configuration with sensible defaults
///
/// # Examples
///
/// ```rust,no_run
/// use nvisy_openrouter::{LlmClient, LlmConfig};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let client = LlmClient::from_api_key("your-api-key")?;
///     let response = client.chat_completion("Hello, world!").await?;
///     println!("Response: {}", response.choices[0].message.content);
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct LlmClient {
    inner: Arc<ClientInner>,
}

struct ClientInner {
    client: OpenRouterClient<Ready>,
    config: LlmConfig,
    rate_limiter: RateLimiter<
        NotKeyed,
        InMemoryState,
        MonotonicClock,
        NoOpMiddleware<<MonotonicClock as Clock>::Instant>,
    >,
}

impl LlmClient {
    /// Creates a new OpenRouter client with custom configuration.
    ///
    /// # Parameters
    ///
    /// - `client`: Configured OpenRouter API client
    /// - `config`: Configuration for client behavior
    ///
    /// # Errors
    ///
    /// Returns an error if the rate limiter cannot be initialized with the provided configuration.
    pub fn new(client: OpenRouterClient<Ready>, config: LlmConfig) -> Result<Self, Error> {
        let quota_limit = config.rate_limit.unwrap_or(NonZeroU32::new(10).unwrap());
        let rate_limiter = RateLimiter::new(
            Quota::per_second(quota_limit),
            InMemoryState::default(),
            MonotonicClock,
        );

        let inner = Arc::new(ClientInner {
            client,
            config,
            rate_limiter,
        });

        Ok(Self { inner })
    }

    /// Creates a new OpenRouter client from an API key.
    ///
    /// Uses default configuration optimized for general usage.
    ///
    /// # Parameters
    ///
    /// - `api_key`: Your OpenRouter API key
    ///
    /// # Errors
    ///
    /// Returns an error if the API key is invalid or the client cannot be initialized.
    pub fn from_api_key(api_key: impl Into<String>) -> Result<Self, Error> {
        let client = OpenRouterClient::from_api_key(api_key).map_err(convert_api_error)?;
        Self::new(client, LlmConfig::default())
    }

    /// Creates a new OpenRouter client from an API key with custom configuration.
    ///
    /// # Parameters
    ///
    /// - `api_key`: Your OpenRouter API key
    /// - `config`: Custom configuration for client behavior
    ///
    /// # Errors
    ///
    /// Returns an error if the API key is invalid or the client cannot be initialized.
    pub fn from_api_key_with_config(
        api_key: impl Into<String>,
        config: LlmConfig,
    ) -> Result<Self, Error> {
        let client_builder = if let Some(base_url) = &config.base_url {
            OpenRouterClient::from_api_key_and_url(api_key, base_url.clone())
        } else {
            OpenRouterClient::from_api_key(api_key)
        };

        let client = client_builder.map_err(convert_api_error)?;
        Self::new(client, config)
    }

    /// Sends a simple chat completion request.
    ///
    /// This is a convenience method for sending a single user message and getting a response.
    ///
    /// # Parameters
    ///
    /// - `message`: The user message to send
    ///
    /// # Returns
    ///
    /// The completion response from the API.
    ///
    /// # Errors
    ///
    /// - [`OpenRouterError::RateLimit`]: If rate limit is exceeded
    /// - [`OpenRouterError::Api`]: If the API request fails
    /// - [`OpenRouterError::Network`]: If network communication fails
    pub async fn chat_completion(
        &self,
        message: impl Into<String>,
    ) -> Result<ChatCompletionResponse> {
        let messages = vec![Message {
            role: "user".to_string(),
            content: message.into(),
            name: None,
            tool_calls: None,
        }];

        self.chat_completion_with_messages(messages).await
    }

    /// Sends a chat completion request with multiple messages.
    ///
    /// # Parameters
    ///
    /// - `messages`: The conversation history
    ///
    /// # Returns
    ///
    /// The completion response from the API.
    ///
    /// # Errors
    ///
    /// - [`OpenRouterError::RateLimit`]: If rate limit is exceeded
    /// - [`OpenRouterError::Api`]: If the API request fails
    /// - [`OpenRouterError::Network`]: If network communication fails
    pub async fn chat_completion_with_messages(
        &self,
        messages: Vec<Message>,
    ) -> Result<ChatCompletionResponse> {
        // Apply rate limiting
        self.inner.rate_limiter.until_ready().await;

        let request = ChatCompletionRequest {
            model: self.inner.config.effective_model().to_string(),
            messages,
            stream: Some(false),
            response_format: None,
            tools: None,
            provider: None,
            models: None,
            transforms: None,
        };

        if self.inner.config.enable_tracing {
            tracing::debug!(
                target: OPENROUTER_TARGET,
                model = %request.model,
                message_count = request.messages.len(),
                "Sending chat completion request"
            );
        }

        let chat_api = self.inner.client.chat().map_err(convert_api_error)?;

        let response = chat_api
            .chat_completion(request)
            .await
            .map_err(convert_api_error)?;

        if self.inner.config.enable_tracing {
            tracing::debug!(
                target: OPENROUTER_TARGET,
                choices = response.choices.len(),
                usage = ?response.usage,
                "Received chat completion response"
            );
        }

        Ok(response)
    }

    /// Sends a custom chat completion request.
    ///
    /// This method provides full control over the request parameters.
    ///
    /// # Parameters
    ///
    /// - `request`: The complete chat completion request
    ///
    /// # Returns
    ///
    /// The completion response from the API.
    ///
    /// # Errors
    ///
    /// - [`OpenRouterError::RateLimit`]: If rate limit is exceeded
    /// - [`OpenRouterError::Api`]: If the API request fails
    /// - [`OpenRouterError::Network`]: If network communication fails
    pub async fn chat_completion_custom(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse> {
        // Apply rate limiting
        self.inner.rate_limiter.until_ready().await;

        if self.inner.config.enable_tracing {
            tracing::debug!(
                target: OPENROUTER_TARGET,
                model = %request.model,
                message_count = request.messages.len(),
                "Sending custom chat completion request"
            );
        }

        let chat_api = self.inner.client.chat().map_err(convert_api_error)?;

        let response = chat_api
            .chat_completion(request)
            .await
            .map_err(convert_api_error)?;

        if self.inner.config.enable_tracing {
            tracing::debug!(
                target: OPENROUTER_TARGET,
                choices = response.choices.len(),
                usage = ?response.usage,
                "Received chat completion response"
            );
        }

        Ok(response)
    }

    /// Lists all available models.
    ///
    /// # Returns
    ///
    /// A list of available models with their metadata.
    ///
    /// # Errors
    ///
    /// - [`OpenRouterError::Api`]: If the API request fails
    /// - [`OpenRouterError::Network`]: If network communication fails
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        // Apply rate limiting
        self.inner.rate_limiter.until_ready().await;

        if self.inner.config.enable_tracing {
            tracing::debug!(target: OPENROUTER_TARGET, "Listing available models");
        }

        let models_api = self.inner.client.models().map_err(convert_api_error)?;

        let response = models_api
            .list_models(None)
            .await
            .map_err(convert_api_error)?;
        let models = response.models;

        if self.inner.config.enable_tracing {
            tracing::debug!(
                target: OPENROUTER_TARGET,
                count = models.len(),
                "Retrieved model list"
            );
        }

        Ok(models)
    }

    /// Gets information about a specific model.
    ///
    /// # Parameters
    ///
    /// - `model_id`: The ID of the model to get information for
    ///
    /// # Returns
    ///
    /// Model information and metadata.
    ///
    /// # Errors
    ///
    /// - [`OpenRouterError::Api`]: If the API request fails or model is not found
    /// - [`OpenRouterError::Network`]: If network communication fails
    pub async fn get_model(&self, model_id: impl Into<String>) -> Result<ModelInfo> {
        // Apply rate limiting
        self.inner.rate_limiter.until_ready().await;

        let model_id = model_id.into();

        if self.inner.config.enable_tracing {
            tracing::debug!(target: OPENROUTER_TARGET, model_id = %model_id, "Getting model info");
        }

        // Get all models and find the requested one
        let models = self.list_models().await?;
        let model = models
            .into_iter()
            .find(|m| m.id == model_id)
            .ok_or_else(|| Error::api(format!("Model '{}' not found", model_id)))?;

        if self.inner.config.enable_tracing {
            tracing::debug!(
                target: OPENROUTER_TARGET,
                model_id = %model_id,
                "Retrieved model info"
            );
        }

        Ok(model)
    }

    /// Gets the current configuration.
    pub fn config(&self) -> &LlmConfig {
        &self.inner.config
    }

    /// Gets the current rate limiter status.
    pub fn rate_limit_status(&self) -> (u32, u32) {
        let state = self.inner.rate_limiter.check();
        match state {
            Ok(_) => (1, 1), // Available
            Err(negative) => (0, negative.quota().burst_size().get()),
        }
    }
}

impl std::fmt::Debug for LlmClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("config", &self.inner.config)
            .field("rate_limit_status", &self.rate_limit_status())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_debug() {
        // This test ensures the Debug implementation doesn't panic
        let config = LlmConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(!debug_str.is_empty());
    }
}
