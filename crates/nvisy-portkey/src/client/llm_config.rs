//! Portkey client configuration and builder.
//!
//! This module provides the configuration types and builder pattern for creating
//! and customizing [`LlmClient`] instances.

use std::fmt;
use std::time::Duration;

use derive_builder::Builder;

use crate::{LlmClient, Result};

/// Default values for configuration options.
mod defaults {
    use std::time::Duration;

    /// Default request timeout in seconds.
    pub const REQUEST_TIMEOUT_SECS: u64 = 30;

    /// Default model for completions if none specified.
    pub const DEFAULT_MODEL: &str = "gpt-3.5-turbo";

    /// Default maximum tokens for responses.
    pub const DEFAULT_MAX_TOKENS: u32 = 1000;

    /// Default temperature for completions.
    pub const DEFAULT_TEMPERATURE: f64 = 0.7;

    /// Portkey API base URL.
    pub const BASE_URL: &str = "https://api.portkey.ai/v1";

    /// Returns the default request timeout.
    pub fn request_timeout() -> Duration {
        Duration::from_secs(REQUEST_TIMEOUT_SECS)
    }
}

/// Configuration for the Portkey API client.
///
/// This struct holds all the necessary configuration parameters for creating and using
/// a Portkey API client. It's split into two logical parts:
///
/// 1. **Portkey SDK Configuration**: API key, base URL, timeout, virtual key, trace ID, cache settings
/// 2. **LLM/Model Configuration**: Default model, temperature, tokens, penalties
///
/// The Portkey SDK configuration is passed to the underlying PortkeyClient, while the
/// LLM/model configuration is kept by our wrapper for request defaults.
#[derive(Clone, Builder)]
#[builder(
    name = "LlmBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate_config")
)]
pub struct LlmConfig {
    /// API key for authentication with the Portkey API.
    ///
    /// You can obtain your API key from the Portkey dashboard.
    api_key: String,

    /// Virtual key for routing requests through Portkey.
    ///
    /// Virtual keys define which AI provider and model to use.
    #[builder(default)]
    virtual_key: Option<String>,

    /// Timeout for API requests.
    ///
    /// Controls how long the client will wait for API responses before timing out.
    #[builder(default = "Self::default_request_timeout()")]
    request_timeout: Duration,

    /// Base URL for API requests.
    ///
    /// Defaults to the official Portkey API endpoint.
    #[builder(default = "Self::default_base_url()")]
    base_url: String,

    /// Trace ID for request tracking.
    ///
    /// Optional identifier for tracking requests through Portkey's observability features.
    #[builder(default)]
    trace_id: Option<String>,

    /// Cache namespace for Portkey's caching features.
    ///
    /// Optional namespace to scope cache entries.
    #[builder(default)]
    cache_namespace: Option<String>,

    /// Force cache refresh.
    ///
    /// When true, bypasses cache and fetches fresh responses.
    #[builder(default)]
    cache_force_refresh: Option<bool>,

    /// Default model to use for completions.
    ///
    /// Specifies which model to use when none is explicitly provided in the request.
    #[builder(default)]
    default_model: Option<String>,

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
}

impl LlmBuilder {
    /// Returns the default request timeout.
    fn default_request_timeout() -> Duration {
        defaults::request_timeout()
    }

    /// Returns the default base URL for the Portkey API.
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
    /// # use nvisy_portkey::LlmConfig;
    /// let config = LlmConfig::builder()
    ///     .with_api_key("your-api-key")
    ///     .with_virtual_key("your-virtual-key")
    ///     .with_default_model("gpt-4")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn builder() -> LlmBuilder {
        LlmBuilder::default()
    }

    /// Creates a new Portkey API client using this configuration.
    ///
    /// This is a convenience method that constructs a client from the configuration.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use nvisy_portkey::LlmConfig;
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

    /// Returns the virtual key, if set.
    pub fn virtual_key(&self) -> Option<&str> {
        self.virtual_key.as_deref()
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

    /// Returns the trace ID, if set.
    pub fn trace_id(&self) -> Option<&str> {
        self.trace_id.as_deref()
    }

    /// Returns the cache namespace, if set.
    pub fn cache_namespace(&self) -> Option<&str> {
        self.cache_namespace.as_deref()
    }

    /// Returns the cache force refresh setting, if set.
    pub fn cache_force_refresh(&self) -> Option<bool> {
        self.cache_force_refresh
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
    /// Creates a Portkey API client directly from the builder.
    ///
    /// This is a convenience method that builds the configuration and
    /// creates a client in one step. This is the recommended way to
    /// create a client.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use nvisy_portkey::LlmConfig;
    /// let client = LlmConfig::builder()
    ///     .with_api_key("your-api-key")
    ///     .with_virtual_key("your-virtual-key")
    ///     .with_default_model("gpt-4")
    ///     .build_client()
    ///     .unwrap();
    /// ```
    pub fn build_client(self) -> Result<LlmClient> {
        let config = self.build()?;
        LlmClient::new(config)
    }
}

impl fmt::Debug for LlmConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LlmConfig")
            .field("api_key", &self.masked_api_key())
            .field("virtual_key", &self.virtual_key)
            .field("request_timeout", &self.request_timeout)
            .field("default_model", &self.default_model)
            .field("base_url", &self.base_url)
            .field("default_max_tokens", &self.default_max_tokens)
            .field("default_temperature", &self.default_temperature)
            .field("default_presence_penalty", &self.default_presence_penalty)
            .field("default_frequency_penalty", &self.default_frequency_penalty)
            .field("default_top_p", &self.default_top_p)
            .field("trace_id", &self.trace_id)
            .field("cache_namespace", &self.cache_namespace)
            .field("cache_force_refresh", &self.cache_force_refresh)
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
        assert_eq!(config.base_url(), defaults::BASE_URL);
        assert_eq!(config.request_timeout(), defaults::request_timeout());

        Ok(())
    }

    #[test]
    fn test_config_builder_with_custom_values() -> Result<()> {
        let config = LlmConfig::builder()
            .with_api_key("test_key")
            .with_virtual_key("test_virtual_key")
            .with_base_url("https://custom.api.com")
            .with_request_timeout(Duration::from_secs(60))
            .with_default_model("gpt-4")
            .build()?;

        assert_eq!(config.api_key(), "test_key");
        assert_eq!(config.virtual_key().unwrap(), "test_virtual_key");
        assert_eq!(config.base_url(), "https://custom.api.com");
        assert_eq!(config.request_timeout(), Duration::from_secs(60));
        assert_eq!(config.default_model().unwrap(), "gpt-4");

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
