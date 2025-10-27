//! Configuration types and utilities for OpenRouter client.
//!
//! This module provides comprehensive configuration options for customizing
//! the behavior of the OpenRouter client, including rate limiting, timeouts,
//! model preferences, and operational settings.

use std::num::NonZeroU32;
use std::time::Duration;

use super::error::{Error, Result};

/// Default values for configuration options.
pub mod defaults {
    /// Default rate limit (requests per second).
    pub const RATE_LIMIT: u32 = 10;

    /// Default request timeout in seconds.
    pub const REQUEST_TIMEOUT_SECS: u64 = 30;

    /// Default model for completions if none specified.
    pub const DEFAULT_MODEL: &str = "openai/gpt-3.5-turbo";

    /// Default maximum tokens for responses.
    pub const DEFAULT_MAX_TOKENS: u32 = 1000;

    /// Default temperature for completions.
    pub const DEFAULT_TEMPERATURE: f32 = 0.7;

    /// OpenRouter API base URL.
    pub const BASE_URL: &str = "https://openrouter.ai/api/v1";
}

/// Configuration for OpenRouter client behavior.
///
/// This configuration allows customization of rate limiting, timeouts, model preferences,
/// and other client behaviors. Use the builder methods to create configurations tailored
/// to your specific use case.
///
/// # Examples
///
/// ```rust
/// use nvisy_openrouter::LlmConfig;
/// use std::time::Duration;
///
/// // Basic configuration
/// let config = LlmConfig::new()
///     .with_rate_limit(15)
///     .with_default_model("openai/gpt-4o")
///     .with_tracing(true);
/// ```
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// Maximum requests per second (default: 10)
    pub rate_limit: Option<NonZeroU32>,

    /// Default timeout for API requests (default: 30s)
    pub request_timeout: Option<Duration>,

    /// Default model to use for completions
    pub default_model: Option<String>,

    /// Default base URL for API requests
    pub base_url: Option<String>,

    /// Default maximum tokens for completions
    pub default_max_tokens: Option<u32>,

    /// Default temperature for completions (0.0-2.0)
    pub default_temperature: Option<f32>,

    /// Whether to enable request/response tracing (default: true)
    pub enable_tracing: bool,

    /// Default presence penalty (-2.0 to 2.0)
    pub default_presence_penalty: Option<f32>,

    /// Default frequency penalty (-2.0 to 2.0)
    pub default_frequency_penalty: Option<f32>,

    /// Default top-p value (0.0-1.0)
    pub default_top_p: Option<f32>,

    /// Whether to enable streaming by default
    pub default_stream: bool,

    /// User identifier for usage tracking
    pub user_id: Option<String>,

    /// Custom headers to include with requests
    pub custom_headers: Vec<(String, String)>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            rate_limit: NonZeroU32::new(defaults::RATE_LIMIT),
            request_timeout: Some(Duration::from_secs(defaults::REQUEST_TIMEOUT_SECS)),
            default_model: None,
            base_url: None,
            default_max_tokens: None,
            default_temperature: None,
            enable_tracing: true,
            default_presence_penalty: None,
            default_frequency_penalty: None,
            default_top_p: None,
            default_stream: false,
            user_id: None,
            custom_headers: Vec::new(),
        }
    }
}

impl LlmConfig {
    /// Creates a new configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the rate limit (requests per second).
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum requests per second (must be > 0)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::LlmConfig;
    ///
    /// let config = LlmConfig::new().with_rate_limit(20);
    /// ```
    pub fn with_rate_limit(mut self, limit: u32) -> Self {
        self.rate_limit = NonZeroU32::new(limit);
        self
    }

    /// Sets the request timeout.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum time to wait for API responses
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::LlmConfig;
    /// use std::time::Duration;
    ///
    /// let config = LlmConfig::new()
    ///     .with_request_timeout(Duration::from_secs(45));
    /// ```
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = Some(timeout);
        self
    }

    /// Sets the default model for completions.
    ///
    /// # Arguments
    ///
    /// * `model` - Model ID (e.g., "openai/gpt-4o", "anthropic/claude-3-sonnet")
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::LlmConfig;
    ///
    /// let config = LlmConfig::new()
    ///     .with_default_model("anthropic/claude-3-sonnet");
    /// ```
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = Some(model.into());
        self
    }

    /// Sets the base URL for API requests.
    ///
    /// # Arguments
    ///
    /// * `url` - Base URL for the OpenRouter API
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::LlmConfig;
    ///
    /// let config = LlmConfig::new()
    ///     .with_base_url("https://openrouter.ai/api/v1");
    /// ```
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Sets the default maximum tokens for completions.
    ///
    /// # Arguments
    ///
    /// * `max_tokens` - Maximum number of tokens to generate
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::LlmConfig;
    ///
    /// let config = LlmConfig::new()
    ///     .with_default_max_tokens(2000);
    /// ```
    pub fn with_default_max_tokens(mut self, max_tokens: u32) -> Self {
        self.default_max_tokens = Some(max_tokens);
        self
    }

    /// Sets the default temperature for completions.
    ///
    /// # Arguments
    ///
    /// * `temperature` - Sampling temperature (0.0-2.0)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::LlmConfig;
    ///
    /// let config = LlmConfig::new()
    ///     .with_default_temperature(0.8);
    /// ```
    pub fn with_default_temperature(mut self, temperature: f32) -> Self {
        self.default_temperature = Some(temperature);
        self
    }

    /// Enables or disables request/response tracing.
    ///
    /// # Arguments
    ///
    /// * `enable` - Whether to enable tracing
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::LlmConfig;
    ///
    /// let config = LlmConfig::new()
    ///     .with_tracing(false);
    /// ```
    pub fn with_tracing(mut self, enable: bool) -> Self {
        self.enable_tracing = enable;
        self
    }

    /// Sets the default presence penalty.
    ///
    /// # Arguments
    ///
    /// * `penalty` - Presence penalty (-2.0 to 2.0)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::LlmConfig;
    ///
    /// let config = LlmConfig::new()
    ///     .with_default_presence_penalty(0.1);
    /// ```
    pub fn with_default_presence_penalty(mut self, penalty: f32) -> Self {
        self.default_presence_penalty = Some(penalty);
        self
    }

    /// Sets the default frequency penalty.
    ///
    /// # Arguments
    ///
    /// * `penalty` - Frequency penalty (-2.0 to 2.0)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::LlmConfig;
    ///
    /// let config = LlmConfig::new()
    ///     .with_default_frequency_penalty(0.1);
    /// ```
    pub fn with_default_frequency_penalty(mut self, penalty: f32) -> Self {
        self.default_frequency_penalty = Some(penalty);
        self
    }

    /// Sets the default top-p value.
    ///
    /// # Arguments
    ///
    /// * `top_p` - Top-p nucleus sampling (0.0-1.0)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::LlmConfig;
    ///
    /// let config = LlmConfig::new()
    ///     .with_default_top_p(0.9);
    /// ```
    pub fn with_default_top_p(mut self, top_p: f32) -> Self {
        self.default_top_p = Some(top_p);
        self
    }

    /// Sets whether streaming is enabled by default.
    ///
    /// # Arguments
    ///
    /// * `stream` - Whether to enable streaming by default
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::LlmConfig;
    ///
    /// let config = LlmConfig::new()
    ///     .with_default_stream(true);
    /// ```
    pub fn with_default_stream(mut self, stream: bool) -> Self {
        self.default_stream = stream;
        self
    }

    /// Sets the user ID for usage tracking.
    ///
    /// # Arguments
    ///
    /// * `user_id` - User identifier for analytics and abuse prevention
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::LlmConfig;
    ///
    /// let config = LlmConfig::new()
    ///     .with_user_id("user_12345");
    /// ```
    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Adds a custom header to be included with requests.
    ///
    /// # Arguments
    ///
    /// * `name` - Header name
    /// * `value` - Header value
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::LlmConfig;
    ///
    /// let config = LlmConfig::new()
    ///     .with_custom_header("X-Custom-Header", "custom-value");
    /// ```
    pub fn with_custom_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_headers.push((name.into(), value.into()));
        self
    }

    /// Validates the configuration and returns any validation errors.
    ///
    /// # Returns
    ///
    /// - `Ok(())` if the configuration is valid
    /// - `Err(OpenRouterError)` with details about validation failures
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::LlmConfig;
    ///
    /// let config = LlmConfig::new()
    ///     .with_default_temperature(0.7);
    ///
    /// match config.build() {
    ///     Ok(config) => println!("Configuration built successfully"),
    ///     Err(e) => println!("Configuration error: {}", e),
    /// }
    /// ```
    /// Builds the configuration with validation and clamping of float values.
    ///
    /// This method clamps float values to valid ranges and returns an error
    /// for invalid configurations that cannot be auto-corrected.
    pub fn build(mut self) -> Result<Self> {
        // Clamp temperature to valid range [0.0, 2.0]
        if let Some(temp) = self.default_temperature {
            self.default_temperature = Some(temp.clamp(0.0, 2.0));
        }

        // Clamp presence penalty to valid range [-2.0, 2.0]
        if let Some(penalty) = self.default_presence_penalty {
            self.default_presence_penalty = Some(penalty.clamp(-2.0, 2.0));
        }

        // Clamp frequency penalty to valid range [-2.0, 2.0]
        if let Some(penalty) = self.default_frequency_penalty {
            self.default_frequency_penalty = Some(penalty.clamp(-2.0, 2.0));
        }

        // Clamp top-p to valid range (0.0, 1.0]
        if let Some(top_p) = self.default_top_p {
            self.default_top_p = Some(top_p.clamp(0.001, 1.0));
        }

        // Validate max tokens (cannot be auto-corrected)
        if let Some(max_tokens) = self.default_max_tokens {
            if max_tokens == 0 {
                return Err(Error::config(
                    "Max tokens must be greater than 0".to_string(),
                ));
            }
        }

        // Validate base URL format (cannot be auto-corrected)
        if let Some(base_url) = &self.base_url {
            if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
                return Err(Error::config(format!(
                    "Base URL must start with http:// or https://, got {}",
                    base_url
                )));
            }
        }

        Ok(self)
    }

    /// Returns the effective rate limit (considering defaults).
    pub fn effective_rate_limit(&self) -> u32 {
        self.rate_limit
            .map(|n| n.get())
            .unwrap_or(defaults::RATE_LIMIT)
    }

    /// Returns the effective request timeout (considering defaults).
    pub fn effective_request_timeout(&self) -> Duration {
        self.request_timeout
            .unwrap_or_else(|| Duration::from_secs(defaults::REQUEST_TIMEOUT_SECS))
    }

    /// Returns the effective model (considering defaults).
    pub fn effective_model(&self) -> &str {
        self.default_model
            .as_deref()
            .unwrap_or(defaults::DEFAULT_MODEL)
    }

    /// Returns the effective base URL (considering defaults).
    pub fn effective_base_url(&self) -> &str {
        self.base_url.as_deref().unwrap_or(defaults::BASE_URL)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LlmConfig::default();
        assert_eq!(config.effective_rate_limit(), defaults::RATE_LIMIT);
        assert_eq!(
            config.effective_request_timeout(),
            Duration::from_secs(defaults::REQUEST_TIMEOUT_SECS)
        );
        assert!(config.enable_tracing);
        assert!(!config.default_stream);
    }

    #[test]
    fn test_config_builder() {
        let config = LlmConfig::new()
            .with_rate_limit(5)
            .with_default_model("openai/gpt-4")
            .with_request_timeout(Duration::from_secs(45))
            .with_default_max_tokens(1000)
            .with_default_temperature(0.7)
            .with_tracing(false);

        assert_eq!(config.rate_limit.unwrap().get(), 5);
        assert_eq!(config.default_model.as_ref().unwrap(), "openai/gpt-4");
        assert_eq!(config.request_timeout.unwrap(), Duration::from_secs(45));
        assert_eq!(config.default_max_tokens.unwrap(), 1000);
        assert_eq!(config.default_temperature.unwrap(), 0.7);
        assert!(!config.enable_tracing);
    }

    #[test]
    fn test_build() {
        // Valid configuration - float values should be clamped
        let valid_config = LlmConfig::new()
            .with_default_temperature(0.7)
            .with_default_presence_penalty(0.1)
            .with_default_frequency_penalty(-0.1)
            .with_default_top_p(0.9)
            .with_default_max_tokens(1000);
        assert!(valid_config.build().is_ok());

        // Temperature should be clamped to valid range
        let clamped_temp = LlmConfig::new().with_default_temperature(3.0);
        let result = clamped_temp.build().unwrap();
        assert_eq!(result.default_temperature, Some(2.0));

        // Top-p should be clamped to valid range
        let clamped_top_p = LlmConfig::new().with_default_top_p(0.0);
        let result = clamped_top_p.build().unwrap();
        assert_eq!(result.default_top_p, Some(0.001));

        // Invalid max tokens should return error
        let invalid_max_tokens = LlmConfig::new().with_default_max_tokens(0);
        assert!(invalid_max_tokens.build().is_err());

        // Invalid base URL should return error
        let invalid_base_url = LlmConfig::new().with_base_url("invalid-url");
        assert!(invalid_base_url.build().is_err());
    }

    #[test]
    fn test_custom_headers() {
        let config = LlmConfig::new()
            .with_custom_header("X-App-Name", "test-app")
            .with_custom_header("X-Version", "1.0.0");

        assert_eq!(config.custom_headers.len(), 2);
        assert_eq!(
            config.custom_headers[0],
            ("X-App-Name".to_string(), "test-app".to_string())
        );
        assert_eq!(
            config.custom_headers[1],
            ("X-Version".to_string(), "1.0.0".to_string())
        );
    }
}
