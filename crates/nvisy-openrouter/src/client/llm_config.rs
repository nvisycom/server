//! Configuration types and utilities for OpenRouter client.
//!
//! This module provides comprehensive configuration options for customizing
//! the behavior of the OpenRouter client, including rate limiting, timeouts,
//! model preferences, and operational settings.

use std::num::NonZeroU32;
use std::time::Duration;

use derive_builder::Builder;

/// Default values for configuration options.
mod defaults {
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

/// Validates the LlmConfig before building.
///
/// This function validates configuration values and returns an error
/// for invalid configurations.
fn validate_config(builder: &LlmConfigBuilder) -> std::result::Result<(), String> {
    // Validate temperature range [0.0, 2.0]
    if let Some(Some(temp)) = builder.default_temperature {
        if !(0.0..=2.0).contains(&temp) {
            return Err(format!(
                "Temperature must be between 0.0 and 2.0, got {}",
                temp
            ));
        }
    }

    // Validate presence penalty range [-2.0, 2.0]
    if let Some(Some(penalty)) = builder.default_presence_penalty {
        if !(-2.0..=2.0).contains(&penalty) {
            return Err(format!(
                "Presence penalty must be between -2.0 and 2.0, got {}",
                penalty
            ));
        }
    }

    // Validate frequency penalty range [-2.0, 2.0]
    if let Some(Some(penalty)) = builder.default_frequency_penalty {
        if !(-2.0..=2.0).contains(&penalty) {
            return Err(format!(
                "Frequency penalty must be between -2.0 and 2.0, got {}",
                penalty
            ));
        }
    }

    // Validate top-p range (0.0, 1.0]
    if let Some(Some(top_p)) = builder.default_top_p {
        if !(0.001..=1.0).contains(&top_p) {
            return Err(format!(
                "Top-p must be between 0.001 and 1.0, got {}",
                top_p
            ));
        }
    }

    // Validate max tokens
    if let Some(Some(max_tokens)) = builder.default_max_tokens {
        if max_tokens == 0 {
            return Err("Max tokens must be greater than 0".to_string());
        }
    }

    // Validate base URL format
    if let Some(Some(base_url)) = &builder.base_url {
        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            return Err(format!(
                "Base URL must start with http:// or https://, got {}",
                base_url
            ));
        }
    }

    Ok(())
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
/// use std::num::NonZeroU32;
///
/// // Basic configuration with validation
/// let config = LlmConfig::builder()
///     .with_rate_limit(NonZeroU32::new(15).unwrap())
///     .with_default_model("openai/gpt-4o")
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, Builder)]
#[builder(
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "validate_config")
)]
pub struct LlmConfig {
    /// Maximum requests per second (default: 10)
    #[builder(default)]
    rate_limit: Option<NonZeroU32>,

    /// Default timeout for API requests (default: 30s)
    #[builder(default)]
    request_timeout: Option<Duration>,

    /// Default model to use for completions
    #[builder(default)]
    default_model: Option<String>,

    /// Default base URL for API requests
    #[builder(default)]
    base_url: Option<String>,

    /// Default maximum tokens for completions
    #[builder(default)]
    default_max_tokens: Option<u32>,

    /// Default temperature for completions (0.0-2.0)
    #[builder(default)]
    default_temperature: Option<f32>,

    /// Default presence penalty (-2.0 to 2.0)
    #[builder(default)]
    default_presence_penalty: Option<f32>,

    /// Default frequency penalty (-2.0 to 2.0)
    #[builder(default)]
    default_frequency_penalty: Option<f32>,

    /// Default top-p value (0.0-1.0)
    #[builder(default)]
    default_top_p: Option<f32>,

    /// Whether to enable streaming by default
    #[builder(default)]
    default_stream: bool,

    /// HTTP Referer header for OpenRouter
    #[builder(default)]
    http_referer: Option<String>,

    /// X-Title header for OpenRouter
    #[builder(default)]
    x_title: Option<String>,
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
            default_presence_penalty: None,
            default_frequency_penalty: None,
            default_top_p: None,
            default_stream: false,
            http_referer: None,
            x_title: None,
        }
    }
}

impl LlmConfig {
    /// Creates a new configuration builder.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::LlmConfig;
    /// use std::num::NonZeroU32;
    ///
    /// let config = LlmConfig::builder()
    ///     .with_rate_limit(NonZeroU32::new(20).unwrap())
    ///     .with_default_model("openai/gpt-4")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn builder() -> LlmConfigBuilder {
        LlmConfigBuilder::default()
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

    /// Returns the effective maximum tokens (considering defaults).
    pub fn effective_max_tokens(&self) -> u32 {
        self.default_max_tokens
            .unwrap_or(defaults::DEFAULT_MAX_TOKENS)
    }

    /// Returns the effective temperature (considering defaults).
    pub fn effective_temperature(&self) -> f32 {
        self.default_temperature
            .unwrap_or(defaults::DEFAULT_TEMPERATURE)
    }

    /// Returns the effective presence penalty (considering defaults).
    pub fn effective_presence_penalty(&self) -> f32 {
        self.default_presence_penalty.unwrap_or(0.0)
    }

    /// Returns the effective frequency penalty (considering defaults).
    pub fn effective_frequency_penalty(&self) -> f32 {
        self.default_frequency_penalty.unwrap_or(0.0)
    }

    /// Returns the effective top-p (considering defaults).
    pub fn effective_top_p(&self) -> f32 {
        self.default_top_p.unwrap_or(1.0)
    }

    /// Returns the effective stream setting (considering defaults).
    pub fn effective_stream(&self) -> bool {
        self.default_stream
    }

    /// Returns the default presence penalty setting.
    pub fn default_presence_penalty(&self) -> Option<f32> {
        self.default_presence_penalty
    }

    /// Returns the default frequency penalty setting.
    pub fn default_frequency_penalty(&self) -> Option<f32> {
        self.default_frequency_penalty
    }

    /// Returns the default top-p setting.
    pub fn default_top_p(&self) -> Option<f32> {
        self.default_top_p
    }

    /// Returns whether streaming is enabled by default.
    pub fn default_stream(&self) -> bool {
        self.default_stream
    }

    /// Returns the HTTP referer header setting.
    pub fn http_referer(&self) -> Option<&str> {
        self.http_referer.as_deref()
    }

    /// Returns the X-Title header setting.
    pub fn x_title(&self) -> Option<&str> {
        self.x_title.as_deref()
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
        assert!(!config.default_stream());
    }

    #[test]
    fn test_config_builder() {
        let config = LlmConfig::builder()
            .with_rate_limit(NonZeroU32::new(5).unwrap())
            .with_default_model("openai/gpt-4")
            .with_request_timeout(Duration::from_secs(45))
            .with_default_max_tokens(1000u32)
            .with_default_temperature(0.7)
            .build()
            .unwrap();

        assert_eq!(config.effective_rate_limit(), 5);
        assert_eq!(config.effective_model(), "openai/gpt-4");
        assert_eq!(config.effective_request_timeout(), Duration::from_secs(45));
        assert_eq!(config.effective_max_tokens(), 1000);
        assert_eq!(config.effective_temperature(), 0.7);
    }

    #[test]
    fn test_builder_validation() {
        // Valid configuration should build successfully
        let valid_config = LlmConfig::builder()
            .with_default_temperature(0.7)
            .with_default_presence_penalty(0.1)
            .with_default_frequency_penalty(-0.1)
            .with_default_top_p(0.9)
            .with_default_max_tokens(1000u32)
            .build()
            .unwrap();

        assert_eq!(valid_config.effective_temperature(), 0.7);

        // Invalid temperature should fail during build
        let invalid_temp_result = LlmConfig::builder().with_default_temperature(3.0).build();
        assert!(invalid_temp_result.is_err());

        // Invalid top-p should fail during build
        let invalid_top_p_result = LlmConfig::builder().with_default_top_p(0.0).build();
        assert!(invalid_top_p_result.is_err());

        // Invalid max tokens should fail during build
        let invalid_max_tokens_result = LlmConfig::builder().with_default_max_tokens(0u32).build();
        assert!(invalid_max_tokens_result.is_err());

        // Invalid base URL should fail during build
        let invalid_base_url_result = LlmConfig::builder().with_base_url("invalid-url").build();
        assert!(invalid_base_url_result.is_err());
    }

    #[test]
    fn test_effective_values() {
        let config = LlmConfig::builder()
            .with_rate_limit(NonZeroU32::new(20).unwrap())
            .with_default_model("custom/model")
            .build()
            .unwrap();

        assert_eq!(config.effective_rate_limit(), 20);
        assert_eq!(config.effective_model(), "custom/model");
        assert_eq!(config.effective_base_url(), defaults::BASE_URL);
        assert_eq!(config.effective_max_tokens(), defaults::DEFAULT_MAX_TOKENS);
    }

    #[test]
    fn test_getter_methods() {
        let config = LlmConfig::builder()
            .with_rate_limit(NonZeroU32::new(15).unwrap())
            .with_default_stream(true)
            .with_http_referer("https://example.com")
            .with_default_presence_penalty(0.5)
            .build()
            .unwrap();

        assert_eq!(config.effective_rate_limit(), 15);
        assert!(config.default_stream());
        assert_eq!(config.http_referer().unwrap(), "https://example.com");
        assert!(config.x_title().is_none());
        assert_eq!(config.default_presence_penalty().unwrap(), 0.5);
        assert_eq!(config.effective_presence_penalty(), 0.5);
    }
}
