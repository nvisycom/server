//! OpenRouter client configuration and builder.
//!
//! This module provides the configuration types and builder pattern for creating
//! and customizing [`LlmClient`] instances.

use std::num::NonZeroU32;
use std::time::Duration;

use derive_builder::Builder;

use crate::{LlmClient, Result};

/// Default values for configuration options.
mod defaults {
    use std::time::Duration;

    /// Default rate limit (requests per second).
    pub const RATE_LIMIT: u32 = 10;

    /// Default request timeout in seconds.
    pub const REQUEST_TIMEOUT_SECS: u64 = 30;

    /// Default model for completions if none specified.
    pub const DEFAULT_MODEL: &str = "openai/gpt-3.5-turbo";

    /// Default maximum tokens for responses.
    pub const DEFAULT_MAX_TOKENS: u32 = 1000;

    /// Default temperature for completions.
    pub const DEFAULT_TEMPERATURE: f64 = 0.7;

    /// OpenRouter API base URL.
    pub const BASE_URL: &str = "https://openrouter.ai/api/v1";

    /// Returns the default request timeout.
    pub fn request_timeout() -> Duration {
        Duration::from_secs(REQUEST_TIMEOUT_SECS)
    }
}

/// Configuration for the OpenRouter API client.
///
/// This struct holds all the necessary configuration parameters for creating and using
/// an OpenRouter API client, including authentication credentials, rate limiting, timeouts,
/// model preferences, and operational settings.
#[derive(Clone, Builder)]
#[builder(
    name = "LlmBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate_config")
)]
pub struct LlmConfig {
    /// API key for authentication with the OpenRouter API.
    ///
    /// You can obtain your API key from the OpenRouter dashboard.
    api_key: String,

    /// Maximum requests per second.
    ///
    /// Controls the rate at which requests are sent to the OpenRouter API.
    #[builder(default = "Self::default_rate_limit()")]
    rate_limit: NonZeroU32,

    /// Timeout for API requests.
    ///
    /// Controls how long the client will wait for API responses before timing out.
    #[builder(default = "Self::default_request_timeout()")]
    request_timeout: Duration,

    /// Default model to use for completions.
    ///
    /// Specifies which model to use when none is explicitly provided in the request.
    #[builder(default)]
    default_model: Option<String>,

    /// Base URL for API requests.
    ///
    /// Defaults to the official OpenRouter API endpoint.
    #[builder(default = "Self::default_base_url()")]
    base_url: String,

    /// Default maximum tokens for completions.
    ///
    /// Limits the length of model responses.
    #[builder(default)]
    default_max_tokens: Option<u32>,

    /// Default temperature for completions (0.0-2.0).
    ///
    /// Controls randomness in model outputs. Higher values make output more random.
    #[builder(default)]
    default_temperature: Option<f64>,

    /// Default presence penalty (-2.0 to 2.0).
    ///
    /// Penalizes tokens based on whether they appear in the text so far.
    #[builder(default)]
    default_presence_penalty: Option<f64>,

    /// Default frequency penalty (-2.0 to 2.0).
    ///
    /// Penalizes tokens based on their frequency in the text so far.
    #[builder(default)]
    default_frequency_penalty: Option<f64>,

    /// Default top-p value (0.0-1.0).
    ///
    /// Controls diversity via nucleus sampling. Lower values make output more focused.
    #[builder(default)]
    default_top_p: Option<f64>,

    /// HTTP Referer header for OpenRouter.
    ///
    /// Optional header to identify your application.
    #[builder(default)]
    http_referer: Option<String>,

    /// X-Title header for OpenRouter.
    ///
    /// Optional header to provide a human-readable title for your application.
    #[builder(default)]
    x_title: Option<String>,
}

impl LlmBuilder {
    /// Returns the default rate limit.
    fn default_rate_limit() -> NonZeroU32 {
        NonZeroU32::new(defaults::RATE_LIMIT).expect("default rate limit is non-zero")
    }

    /// Returns the default request timeout.
    fn default_request_timeout() -> Duration {
        defaults::request_timeout()
    }

    /// Returns the default base URL for the OpenRouter API.
    fn default_base_url() -> String {
        defaults::BASE_URL.to_string()
    }

    /// Validates the configuration before building.
    fn validate_config(&self) -> std::result::Result<(), String> {
        // Validate API key is not empty
        if let Some(ref api_key) = self.api_key
            && api_key.trim().is_empty()
        {
            return Err("API key cannot be empty".to_string());
        }

        // Validate temperature range [0.0, 2.0]
        if let Some(Some(temp)) = self.default_temperature
            && !(0.0..=2.0).contains(&temp)
        {
            return Err(format!(
                "Temperature must be between 0.0 and 2.0, got {}",
                temp
            ));
        }

        // Validate presence penalty range [-2.0, 2.0]
        if let Some(Some(penalty)) = self.default_presence_penalty
            && !(-2.0..=2.0).contains(&penalty)
        {
            return Err(format!(
                "Presence penalty must be between -2.0 and 2.0, got {}",
                penalty
            ));
        }

        // Validate frequency penalty range [-2.0, 2.0]
        if let Some(Some(penalty)) = self.default_frequency_penalty
            && !(-2.0..=2.0).contains(&penalty)
        {
            return Err(format!(
                "Frequency penalty must be between -2.0 and 2.0, got {}",
                penalty
            ));
        }

        // Validate top-p range (0.0, 1.0]
        if let Some(Some(top_p)) = self.default_top_p
            && !(0.001..=1.0).contains(&top_p)
        {
            return Err(format!(
                "Top-p must be between 0.001 and 1.0, got {}",
                top_p
            ));
        }

        // Validate max tokens
        if let Some(Some(max_tokens)) = self.default_max_tokens
            && max_tokens == 0
        {
            return Err("Max tokens must be greater than 0".to_string());
        }

        // Validate base URL format
        if let Some(base_url) = &self.base_url
            && !base_url.starts_with("http://")
            && !base_url.starts_with("https://")
        {
            return Err(format!(
                "Base URL must start with http:// or https://, got {}",
                base_url
            ));
        }

        // Validate timeout is reasonable
        if let Some(timeout) = self.request_timeout {
            if timeout.is_zero() {
                return Err("Timeout must be greater than 0".to_string());
            }
            if timeout > Duration::from_secs(300) {
                return Err("Timeout cannot exceed 300 seconds (5 minutes)".to_string());
            }
        }

        // Validate rate limit
        if let Some(rate_limit) = self.rate_limit
            && rate_limit.get() > 1000
        {
            return Err(format!(
                "Rate limit seems unreasonably high: {} requests/second",
                rate_limit
            ));
        }

        Ok(())
    }
}

impl LlmConfig {
    /// Creates a new configuration builder.
    ///
    /// This is the recommended way to construct an `LlmConfig`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use nvisy_openrouter::LlmConfig;
    /// let config = LlmConfig::builder()
    ///     .with_api_key("your-api-key")
    ///     .with_default_model("openai/gpt-4")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn builder() -> LlmBuilder {
        LlmBuilder::default()
    }

    /// Creates a new OpenRouter API client using this configuration.
    ///
    /// This is a convenience method that constructs a client from the configuration.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use nvisy_openrouter::LlmConfig;
    /// let config = LlmConfig::builder()
    ///     .with_api_key("your-api-key")
    ///     .build()
    ///     .unwrap();
    ///
    /// let client = config.build_client().unwrap();
    /// ```
    pub fn build_client(self) -> Result<LlmClient> {
        LlmClient::new(self)
    }

    /// Returns the API key.
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Returns a masked version of the API key for safe display/logging.
    ///
    /// Shows the first 4 characters followed by "****", or just "****"
    /// if the key is shorter than 4 characters.
    pub fn masked_api_key(&self) -> String {
        if self.api_key.len() > 4 {
            format!("{}****", &self.api_key[..4])
        } else {
            "****".to_string()
        }
    }

    /// Returns the rate limit.
    pub fn rate_limit(&self) -> u32 {
        self.rate_limit.get()
    }

    /// Returns the request timeout duration.
    pub fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    /// Returns the base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Returns the default model, if set.
    pub fn default_model(&self) -> Option<&str> {
        self.default_model.as_deref()
    }

    /// Returns the default maximum tokens, if set.
    pub fn default_max_tokens(&self) -> Option<u32> {
        self.default_max_tokens
    }

    /// Returns the default temperature, if set.
    pub fn default_temperature(&self) -> Option<f64> {
        self.default_temperature
    }

    /// Returns the default presence penalty, if set.
    pub fn default_presence_penalty(&self) -> Option<f64> {
        self.default_presence_penalty
    }

    /// Returns the default frequency penalty, if set.
    pub fn default_frequency_penalty(&self) -> Option<f64> {
        self.default_frequency_penalty
    }

    /// Returns the default top-p, if set.
    pub fn default_top_p(&self) -> Option<f64> {
        self.default_top_p
    }

    /// Returns the HTTP referer header, if set.
    pub fn http_referer(&self) -> Option<&str> {
        self.http_referer.as_deref()
    }

    /// Returns the X-Title header, if set.
    pub fn x_title(&self) -> Option<&str> {
        self.x_title.as_deref()
    }

    /// Returns the effective model (considering defaults).
    ///
    /// Returns the configured model or the default model if none is set.
    pub fn effective_model(&self) -> &str {
        self.default_model
            .as_deref()
            .unwrap_or(defaults::DEFAULT_MODEL)
    }

    /// Returns the effective maximum tokens (considering defaults).
    ///
    /// Returns the configured max tokens or the default if none is set.
    pub fn effective_max_tokens(&self) -> u32 {
        self.default_max_tokens
            .unwrap_or(defaults::DEFAULT_MAX_TOKENS)
    }

    /// Returns the effective temperature (considering defaults).
    ///
    /// Returns the configured temperature or the default if none is set.
    pub fn effective_temperature(&self) -> f64 {
        self.default_temperature
            .unwrap_or(defaults::DEFAULT_TEMPERATURE)
    }

    /// Returns the effective presence penalty (considering defaults).
    ///
    /// Returns the configured presence penalty or 0.0 if none is set.
    pub fn effective_presence_penalty(&self) -> f64 {
        self.default_presence_penalty.unwrap_or(0.0)
    }

    /// Returns the effective frequency penalty (considering defaults).
    ///
    /// Returns the configured frequency penalty or 0.0 if none is set.
    pub fn effective_frequency_penalty(&self) -> f64 {
        self.default_frequency_penalty.unwrap_or(0.0)
    }

    /// Returns the effective top-p (considering defaults).
    ///
    /// Returns the configured top-p or 1.0 if none is set.
    pub fn effective_top_p(&self) -> f64 {
        self.default_top_p.unwrap_or(1.0)
    }
}

impl LlmBuilder {
    /// Creates an OpenRouter API client directly from the builder.
    ///
    /// This is a convenience method that builds the configuration and
    /// creates a client in one step. This is the recommended way to
    /// create a client.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use nvisy_openrouter::LlmConfig;
    /// let client = LlmConfig::builder()
    ///     .with_api_key("your-api-key")
    ///     .with_default_model("openai/gpt-4")
    ///     .build_client()
    ///     .unwrap();
    /// ```
    pub fn build_client(self) -> Result<LlmClient> {
        let config = self.build()?;
        LlmClient::new(config)
    }
}

impl std::fmt::Debug for LlmConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmConfig")
            .field("api_key", &self.masked_api_key())
            .field("rate_limit", &self.rate_limit)
            .field("request_timeout", &self.request_timeout)
            .field("default_model", &self.default_model)
            .field("base_url", &self.base_url)
            .field("default_max_tokens", &self.default_max_tokens)
            .field("default_temperature", &self.default_temperature)
            .field("default_presence_penalty", &self.default_presence_penalty)
            .field("default_frequency_penalty", &self.default_frequency_penalty)
            .field("default_top_p", &self.default_top_p)
            .field("http_referer", &self.http_referer)
            .field("x_title", &self.x_title)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() -> Result<()> {
        let config = LlmConfig::builder().with_api_key("test_key").build()?;

        assert_eq!(config.api_key(), "test_key");
        assert_eq!(config.rate_limit(), defaults::RATE_LIMIT);
        assert_eq!(config.base_url(), defaults::BASE_URL);
        assert_eq!(config.request_timeout(), defaults::request_timeout());

        Ok(())
    }

    #[test]
    fn test_config_builder_with_custom_values() -> Result<()> {
        let config = LlmConfig::builder()
            .with_api_key("test_key")
            .with_rate_limit(NonZeroU32::new(25).unwrap())
            .with_base_url("https://custom.api.com")
            .with_request_timeout(Duration::from_secs(60))
            .with_default_model("openai/gpt-4")
            .build()?;

        assert_eq!(config.api_key(), "test_key");
        assert_eq!(config.rate_limit(), 25);
        assert_eq!(config.base_url(), "https://custom.api.com");
        assert_eq!(config.request_timeout(), Duration::from_secs(60));
        assert_eq!(config.default_model().unwrap(), "openai/gpt-4");

        Ok(())
    }

    #[test]
    fn test_config_validation_empty_api_key() {
        let result = LlmConfig::builder().with_api_key("").build();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("API key cannot be empty")
        );
    }

    #[test]
    fn test_config_validation_temperature() {
        let result = LlmConfig::builder()
            .with_api_key("test_key")
            .with_default_temperature(3.0)
            .build();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Temperature must be between")
        );
    }

    #[test]
    fn test_config_validation_zero_timeout() {
        let result = LlmConfig::builder()
            .with_api_key("test_key")
            .with_request_timeout(Duration::from_secs(0))
            .build();

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Timeout must be greater than 0")
        );
    }

    #[test]
    fn test_config_validation_excessive_timeout() {
        let result = LlmConfig::builder()
            .with_api_key("test_key")
            .with_request_timeout(Duration::from_secs(400))
            .build();

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Timeout cannot exceed")
        );
    }

    #[test]
    fn test_config_validation_invalid_base_url() {
        let result = LlmConfig::builder()
            .with_api_key("test_key")
            .with_base_url("invalid-url")
            .build();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Base URL must start with")
        );
    }

    #[test]
    fn test_config_validation_top_p() {
        let result = LlmConfig::builder()
            .with_api_key("test_key")
            .with_default_top_p(0.0)
            .build();
        assert!(result.is_err());

        let result = LlmConfig::builder()
            .with_api_key("test_key")
            .with_default_top_p(1.5)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_config_validation_max_tokens() {
        let result = LlmConfig::builder()
            .with_api_key("test_key")
            .with_default_max_tokens(0u32)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_config_validation_penalties() {
        // Valid penalties
        let config = LlmConfig::builder()
            .with_api_key("test_key")
            .with_default_presence_penalty(0.5)
            .with_default_frequency_penalty(-0.5)
            .build()
            .unwrap();

        assert_eq!(config.default_presence_penalty().unwrap(), 0.5);
        assert_eq!(config.default_frequency_penalty().unwrap(), -0.5);

        // Invalid presence penalty
        let result = LlmConfig::builder()
            .with_api_key("test_key")
            .with_default_presence_penalty(3.0)
            .build();
        assert!(result.is_err());

        // Invalid frequency penalty
        let result = LlmConfig::builder()
            .with_api_key("test_key")
            .with_default_frequency_penalty(-3.0)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_config_builder_with_all_options() -> Result<()> {
        let config = LlmConfig::builder()
            .with_api_key("test_key_comprehensive")
            .with_rate_limit(NonZeroU32::new(30).unwrap())
            .with_base_url("https://api.custom-domain.com/v2")
            .with_request_timeout(Duration::from_secs(120))
            .with_default_model("anthropic/claude-3-opus")
            .with_default_max_tokens(2000u32)
            .with_default_temperature(0.8)
            .with_default_presence_penalty(0.1)
            .with_default_frequency_penalty(0.2)
            .with_default_top_p(0.95)
            .with_http_referer("https://myapp.com")
            .with_x_title("My Application")
            .build()?;

        assert_eq!(config.api_key(), "test_key_comprehensive");
        assert_eq!(config.rate_limit(), 30);
        assert_eq!(config.base_url(), "https://api.custom-domain.com/v2");
        assert_eq!(config.request_timeout(), Duration::from_secs(120));
        assert_eq!(config.default_model().unwrap(), "anthropic/claude-3-opus");
        assert_eq!(config.default_max_tokens().unwrap(), 2000);
        assert_eq!(config.default_temperature().unwrap(), 0.8);
        assert_eq!(config.default_presence_penalty().unwrap(), 0.1);
        assert_eq!(config.default_frequency_penalty().unwrap(), 0.2);
        assert_eq!(config.default_top_p().unwrap(), 0.95);
        assert_eq!(config.http_referer().unwrap(), "https://myapp.com");
        assert_eq!(config.x_title().unwrap(), "My Application");

        Ok(())
    }

    #[test]
    fn test_config_builder_defaults() -> Result<()> {
        let config = LlmConfig::builder().with_api_key("test_key").build()?;

        assert_eq!(config.api_key(), "test_key");
        assert_eq!(config.rate_limit(), defaults::RATE_LIMIT);
        assert_eq!(config.base_url(), defaults::BASE_URL);
        assert_eq!(config.request_timeout(), defaults::request_timeout());
        assert_eq!(config.default_model(), None);

        Ok(())
    }

    #[test]
    fn test_effective_values() {
        let config = LlmConfig::builder()
            .with_api_key("test_key")
            .with_rate_limit(NonZeroU32::new(20).unwrap())
            .with_default_model("custom/model")
            .build()
            .unwrap();

        assert_eq!(config.rate_limit(), 20);
        assert_eq!(config.effective_model(), "custom/model");
        assert_eq!(config.effective_max_tokens(), defaults::DEFAULT_MAX_TOKENS);
        assert_eq!(
            config.effective_temperature(),
            defaults::DEFAULT_TEMPERATURE
        );
    }

    #[test]
    fn test_getter_methods() {
        let config = LlmConfig::builder()
            .with_api_key("test_key")
            .with_rate_limit(NonZeroU32::new(15).unwrap())
            .with_http_referer("https://example.com")
            .with_default_presence_penalty(0.5)
            .build()
            .unwrap();

        assert_eq!(config.rate_limit(), 15);
        assert_eq!(config.http_referer().unwrap(), "https://example.com");
        assert!(config.x_title().is_none());
        assert_eq!(config.default_presence_penalty().unwrap(), 0.5);
        assert_eq!(config.effective_presence_penalty(), 0.5);
    }

    #[test]
    fn test_config_debug_impl() {
        let config = LlmConfig::builder()
            .with_api_key("test_api_key_1234567890")
            .with_default_model("openai/gpt-4")
            .build()
            .unwrap();

        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("LlmConfig"));
        assert!(debug_str.contains("rate_limit"));
        assert!(debug_str.contains("base_url"));
        // Should show masked key, not full key
        assert!(debug_str.contains("test****"));
        assert!(!debug_str.contains("test_api_key_1234567890"));
    }

    #[test]
    fn test_masked_api_key() {
        let config = LlmConfig::builder()
            .with_api_key("test_key_1234567890")
            .build()
            .unwrap();

        assert_eq!(config.masked_api_key(), "test****");

        let short_config = LlmConfig::builder().with_api_key("key").build().unwrap();

        assert_eq!(short_config.masked_api_key(), "****");
    }
}
