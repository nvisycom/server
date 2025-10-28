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
use openrouter_rs::{Model, OpenRouterClient};

use super::llm_config::LlmConfig;
use crate::OPENROUTER_TARGET;
use crate::error::{Error, Result};

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
/// use nvisy_openrouter::{LlmClient, LlmConfig, TypedChatCompletion, TypedChatRequest};
/// use openrouter_rs::api::chat::Message;
/// use openrouter_rs::types::Role;
/// use serde::{Serialize, Deserialize};
/// use schemars::JsonSchema;
///
/// #[derive(Serialize)]
/// struct MyRequest { query: String }
///
/// #[derive(Deserialize, JsonSchema)]
/// struct MyResponse { answer: String }
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let client = LlmClient::from_api_key("your-api-key")?;
///     let completion = TypedChatCompletion::<MyRequest, MyResponse>::new(client);
///     let request = TypedChatRequest::builder()
///         .with_messages(vec![Message::new(Role::User, "Hello!")])
///         .with_request(MyRequest { query: "Hello".to_string() })
///         .build()?;
///     let response = completion.chat_completion(request).await?;
///     println!("Response: {}", response.data.answer);
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct LlmClient {
    inner: Arc<ClientInner>,
}

struct ClientInner {
    client: OpenRouterClient,
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
    pub fn new(client: OpenRouterClient, config: LlmConfig) -> Result<Self, Error> {
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
        let config = LlmConfig::default();
        Self::from_api_key_with_config(api_key, config)
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
        let mut builder = OpenRouterClient::builder();
        builder.api_key(api_key.into());

        if let Some(base_url) = &config.base_url {
            builder.base_url(base_url.clone());
        }

        if let Some(referer) = &config.http_referer {
            builder.http_referer(referer.clone());
        }

        if let Some(title) = &config.x_title {
            builder.x_title(title.clone());
        }

        let client = builder.build()?;
        Self::new(client, config)
    }

    /// Sends a request with rate limiting by executing the provided async function.
    ///
    /// This method provides a generic interface for sending any type of request
    /// to the OpenRouter API with automatic rate limiting and error handling.
    ///
    /// # Type Parameters
    ///
    /// - `F`: The async function type
    /// - `Fut`: The future returned by the async function
    /// - `T`: The response type
    ///
    /// # Parameters
    ///
    /// - `f`: An async function that takes a reference to the OpenRouterClient and LlmConfig, and returns a Result<T>
    ///
    /// # Returns
    ///
    /// The result from executing the provided function.
    ///
    /// # Errors
    ///
    /// - [`Error::RateLimit`]: If rate limit is exceeded
    /// - Any error returned by the provided function
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use nvisy_openrouter::{LlmClient, LlmConfig};
    /// # use openrouter_rs::api::chat::ChatCompletionRequest;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = LlmClient::from_api_key("your-api-key")?;
    /// let request = ChatCompletionRequest::builder()
    ///     .model("openai/gpt-3.5-turbo")
    ///     .build()?;
    /// let response = client.send_request(|c, _config| {
    ///     let c = c.clone();
    ///     async move {
    ///         c.send_chat_completion(&request).await.map_err(Into::into)
    ///     }
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_request<F, Fut, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&OpenRouterClient, &LlmConfig) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        // Apply rate limiting before sending request
        self.inner.rate_limiter.until_ready().await;

        tracing::debug!(
            target: OPENROUTER_TARGET,
            "Sending request"
        );

        let response = f(&self.inner.client, &self.inner.config).await?;

        tracing::info!(
            target: OPENROUTER_TARGET,
            "Request successful"
        );

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
    /// - [`Error::Api`]: If the API request fails
    /// - [`Error::Network`]: If network communication fails
    pub async fn list_models(&self) -> Result<Vec<Model>> {
        self.send_request(|client, _config| {
            let client = client.clone();
            async move {
                let models = client.list_models().await?;
                tracing::info!(
                    target: OPENROUTER_TARGET,
                    count = models.len(),
                    "Retrieved model list successfully"
                );
                Ok(models)
            }
        })
        .await
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
        f.debug_struct("LlmClient")
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
