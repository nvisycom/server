//! OpenRouter API client implementation.
//!
//! This module provides the main client for interacting with OpenRouter's API,
//! including chat completions, model information, and rate limiting.

use std::fmt;
use std::future::Future;
use std::num::NonZeroU32;
use std::sync::Arc;

use governor::clock::{Clock, MonotonicClock};
use governor::middleware::NoOpMiddleware;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};
use openrouter_rs::config::OpenRouterConfig;
use openrouter_rs::{Model, OpenRouterClient};

use super::llm_config::LlmConfig;
use crate::{Result, TRACING_TARGET_CLIENT};

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
    /// Creates a new OpenRouter client from a configuration.
    ///
    /// This method is the primary constructor when you have an [`LlmConfig`] instance.
    /// The configuration specifies the API key, rate limits, timeouts, and default
    /// model parameters.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use nvisy_openrouter::{LlmClient, LlmConfig};
    /// let config = LlmConfig::builder()
    ///     .with_api_key("your-api-key")
    ///     .with_default_model("openai/gpt-4")
    ///     .build()
    ///     .unwrap();
    ///
    /// let client = LlmClient::new(config).unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The underlying OpenRouter client cannot be initialized
    /// - The rate limiter configuration is invalid
    pub fn new(config: LlmConfig) -> Result<Self> {
        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            base_url = config.base_url(),
            rate_limit = config.rate_limit(),
            "Building OpenRouter client from configuration"
        );

        let mut builder = OpenRouterClient::builder();
        builder.api_key(config.api_key());
        builder.base_url(config.base_url());
        builder.config(OpenRouterConfig {
            default_model: config.effective_model().to_owned(),
            ..Default::default()
        });

        if let Some(referer) = config.http_referer() {
            builder.http_referer(referer);
            tracing::debug!(
                target: TRACING_TARGET_CLIENT,
                referer = referer,
                "Set HTTP referer"
            );
        }

        if let Some(title) = config.x_title() {
            builder.x_title(title);
            tracing::debug!(
                target: TRACING_TARGET_CLIENT,
                title = title,
                "Set X-Title"
            );
        }

        let client = builder.build()?;
        Self::with_client(client, config)
    }

    /// Creates a new OpenRouter client with a pre-configured OpenRouter client and custom configuration.
    ///
    /// This is useful when you need fine-grained control over the underlying OpenRouter client
    /// or when integrating with existing OpenRouter client instances.
    ///
    /// # Parameters
    ///
    /// - `client`: Pre-configured OpenRouter API client
    /// - `config`: Configuration for client behavior
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use nvisy_openrouter::{LlmClient, LlmConfig};
    /// # use openrouter_rs::OpenRouterClient;
    /// let openrouter_client = OpenRouterClient::builder()
    ///     .api_key("your-api-key")
    ///     .build()
    ///     .unwrap();
    ///
    /// let config = LlmConfig::builder()
    ///     .with_api_key("your-api-key")
    ///     .build()
    ///     .unwrap();
    ///
    /// let client = LlmClient::with_client(openrouter_client, config).unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the rate limiter cannot be initialized with the provided configuration.
    pub fn with_client(client: OpenRouterClient, config: LlmConfig) -> Result<Self> {
        let quota_limit = NonZeroU32::new(config.rate_limit())
            .expect("rate limit from config should be non-zero");

        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            rate_limit = config.rate_limit(),
            base_url = config.base_url(),
            "Initializing LlmClient with configuration"
        );

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

        tracing::info!(
            target: TRACING_TARGET_CLIENT,
            "LlmClient initialized successfully"
        );

        Ok(Self { inner })
    }

    /// Creates a new OpenRouter client from an API key.
    ///
    /// Uses default configuration optimized for general usage. This is the
    /// simplest way to create a client when you only need to provide an API key.
    ///
    /// # Parameters
    ///
    /// - `api_key`: Your OpenRouter API key
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use nvisy_openrouter::LlmClient;
    /// let client = LlmClient::from_api_key("your-api-key").unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The API key is invalid or empty
    /// - The client cannot be initialized
    pub fn from_api_key(api_key: impl Into<String>) -> Result<Self> {
        let config = LlmConfig::builder().with_api_key(api_key).build()?;
        Self::new(config)
    }

    /// Sends a request with rate limiting by executing the provided async function.
    ///
    /// This method provides a generic interface for sending any type of request
    /// to the OpenRouter API with automatic rate limiting and error handling.
    /// The rate limiter ensures that requests are throttled according to the
    /// configured rate limit.
    ///
    /// # Type Parameters
    ///
    /// - `F`: The async function type that creates the future
    /// - `Fut`: The future returned by the async function
    /// - `T`: The response type from the request
    ///
    /// # Parameters
    ///
    /// - `f`: An async function that takes references to the OpenRouterClient and LlmConfig,
    ///   and returns a `Result<T>`
    ///
    /// # Returns
    ///
    /// The result from executing the provided function.
    ///
    /// # Errors
    ///
    /// - [`Error::RateLimit`]: If rate limit is exceeded (this shouldn't happen with governor)
    /// - Any error returned by the provided function
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use nvisy_openrouter::{LlmClient, Result};
    /// # use openrouter_rs::api::chat::ChatCompletionRequest;
    /// # async fn example() -> Result<()> {
    /// let client = LlmClient::from_api_key("your-api-key")?;
    /// let request = ChatCompletionRequest::builder()
    ///     .model("openai/gpt-3.5-turbo")
    ///     .build()?;
    ///
    /// let response = client.send_request(|c, _config| {
    ///     let c = c.clone();
    ///     let req = request.clone();
    ///     async move {
    ///         c.send_chat_completion(&req).await.map_err(Into::into)
    ///     }
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_request<F, Fut, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&OpenRouterClient, &LlmConfig) -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        // Apply rate limiting before sending request
        self.inner.rate_limiter.until_ready().await;

        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            "Sending request to OpenRouter API"
        );

        let response = f(&self.inner.client, &self.inner.config).await?;

        tracing::info!(
            target: TRACING_TARGET_CLIENT,
            "Request completed successfully"
        );

        Ok(response)
    }

    /// Lists all available models from the OpenRouter API.
    ///
    /// Retrieves the complete list of models available through OpenRouter,
    /// including their capabilities, pricing, and metadata.
    ///
    /// # Returns
    ///
    /// A vector of [`Model`] instances with their metadata.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use nvisy_openrouter::{LlmClient, Result};
    /// # async fn example() -> Result<()> {
    /// let client = LlmClient::from_api_key("your-api-key")?;
    /// let models = client.list_models().await?;
    ///
    /// for model in models {
    ///     println!("Model: {} - {}", model.id, model.name);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// - [`Error::Api`]: If the API request fails
    /// - [`Error::RateLimit`]: If rate limit is exceeded
    pub async fn list_models(&self) -> Result<Vec<Model>> {
        self.send_request(|client, _config| {
            let client = client.clone();
            async move {
                tracing::debug!(
                    target: TRACING_TARGET_CLIENT,
                    "Fetching model list"
                );

                let models = client.list_models().await?;

                tracing::info!(
                    target: TRACING_TARGET_CLIENT,
                    count = models.len(),
                    "Retrieved model list successfully"
                );

                Ok(models)
            }
        })
        .await
    }

    /// Returns a reference to the client's configuration.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use nvisy_openrouter::LlmClient;
    /// let client = LlmClient::from_api_key("your-api-key").unwrap();
    /// let config = client.as_config();
    /// println!("Rate limit: {}", config.rate_limit());
    /// ```
    pub fn as_config(&self) -> &LlmConfig {
        &self.inner.config
    }

    /// Returns a reference to the underlying OpenRouter client.
    ///
    /// This provides direct access to the OpenRouter client for advanced use cases
    /// where you need to bypass the rate limiter or access client-specific methods.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use nvisy_openrouter::LlmClient;
    /// let client = LlmClient::from_api_key("your-api-key").unwrap();
    /// let inner_client = client.as_client();
    /// // Use inner_client directly for advanced operations
    /// ```
    pub fn as_client(&self) -> &OpenRouterClient {
        &self.inner.client
    }

    /// Gets the current rate limiter status.
    ///
    /// Returns a tuple of (available_tokens, total_capacity) indicating
    /// how many requests can be made immediately and the total burst capacity.
    ///
    /// # Returns
    ///
    /// A tuple `(available, capacity)` where:
    /// - `available`: Number of requests that can be made immediately
    /// - `capacity`: Total burst capacity of the rate limiter
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use nvisy_openrouter::LlmClient;
    /// let client = LlmClient::from_api_key("your-api-key").unwrap();
    /// let (available, capacity) = client.rate_limit_status();
    /// println!("Can make {} requests out of {} capacity", available, capacity);
    /// ```
    pub fn rate_limit(&self) -> Result<(), NonZeroU32> {
        let state = self.inner.rate_limiter.check();
        match state {
            Ok(_) => Ok(()), // Available
            Err(negative) => Err(negative.quota().burst_size()),
        }
    }
}

impl fmt::Debug for LlmClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LlmClient")
            .field("config", &self.inner.config)
            .field("rate_limit", &self.rate_limit())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() -> Result<()> {
        let config = LlmConfig::builder().with_api_key("test_key").build()?;
        assert_eq!(config.rate_limit(), 10);
        Ok(())
    }

    #[test]
    fn test_config_with_custom_values() -> Result<()> {
        let config = LlmConfig::builder()
            .with_api_key("test_key")
            .with_rate_limit(NonZeroU32::new(20).unwrap())
            .with_default_model("openai/gpt-4")
            .build()?;

        assert_eq!(config.rate_limit(), 20);
        assert_eq!(config.default_model().unwrap(), "openai/gpt-4");
        Ok(())
    }

    #[test]
    fn test_client_debug() {
        let config = LlmConfig::builder()
            .with_api_key("test_key")
            .build()
            .unwrap();
        let debug_str = format!("{:?}", config);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("LlmConfig"));
    }

    #[test]
    fn test_rate_limit_from_config() -> Result<()> {
        let config = LlmConfig::builder()
            .with_api_key("test_key")
            .with_rate_limit(NonZeroU32::new(15).unwrap())
            .build()?;

        assert_eq!(config.rate_limit(), 15);
        Ok(())
    }

    #[test]
    fn test_masked_api_key_in_debug() {
        let config = LlmConfig::builder()
            .with_api_key("secret_key_12345")
            .build()
            .unwrap();

        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("secr****"));
        assert!(!debug_str.contains("secret_key_12345"));
    }
}
