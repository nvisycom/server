//! Configuration types and utilities for OpenRouter client.
//!
//! This module provides comprehensive configuration options for customizing
//! the behavior of the OpenRouter client, including rate limiting, timeouts,
//! model preferences, and operational settings.

use std::num::NonZeroU32;
use std::time::Duration;

use derive_builder::Builder;

use crate::error::{Error, Result};

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
/// use std::num::NonZeroU32;
///
/// // Basic configuration
/// let config = LlmConfig::builder()
///     .with_rate_limit(NonZeroU32::new(15).unwrap())
///     .with_default_model("openai/gpt-4o")
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, Builder)]
#[builder(pattern = "owned", setter(into, strip_option, prefix = "with"))]
pub struct LlmConfig {
    /// Maximum requests per second (default: 10)
    #[builder(default)]
    pub rate_limit: Option<NonZeroU32>,

    /// Default timeout for API requests (default: 30s)
    #[builder(default)]
    pub request_timeout: Option<Duration>,

    /// Default model to use for completions
    #[builder(default)]
    pub default_model: Option<String>,

    /// Default base URL for API requests
    #[builder(default)]
    pub base_url: Option<String>,

    /// Default maximum tokens for completions
    #[builder(default)]
    pub default_max_tokens: Option<u32>,

    /// Default temperature for completions (0.0-2.0)
    #[builder(default)]
    pub default_temperature: Option<f32>,

    /// Default presence penalty (-2.0 to 2.0)
    #[builder(default)]
    pub default_presence_penalty: Option<f32>,

    /// Default frequency penalty (-2.0 to 2.0)
    #[builder(default)]
    pub default_frequency_penalty: Option<f32>,

    /// Default top-p value (0.0-1.0)
    #[builder(default)]
    pub default_top_p: Option<f32>,

    /// Whether to enable streaming by default
    #[builder(default)]
    pub default_stream: bool,

    /// HTTP Referer header for OpenRouter
    #[builder(default)]
    pub http_referer: Option<String>,

    /// X-Title header for OpenRouter
    #[builder(default)]
    pub x_title: Option<String>,
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

    /// Validates and normalizes the configuration values.
    ///
    /// This method clamps float values to valid ranges and returns an error
    /// for invalid configurations that cannot be auto-corrected.
    ///
    /// # Returns
    ///
    /// - `Ok(())` if the configuration is valid
    /// - `Err(Error)` with details about validation failures
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::LlmConfig;
    ///
    /// let mut config = LlmConfig::builder()
    ///     .with_default_temperature(0.7)
    ///     .build()
    ///     .unwrap();
    ///
    /// config.validate()?;
    /// # Ok::<(), nvisy_openrouter::Error>(())
    /// ```
    pub fn validate(&mut self) -> Result<()> {
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

        Ok(())
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
        assert!(!config.default_stream);
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

        assert_eq!(config.rate_limit.unwrap().get(), 5);
        assert_eq!(config.default_model.as_ref().unwrap(), "openai/gpt-4");
        assert_eq!(config.request_timeout.unwrap(), Duration::from_secs(45));
        assert_eq!(config.default_max_tokens.unwrap(), 1000);
        assert_eq!(config.default_temperature.unwrap(), 0.7);
    }

    #[test]
    fn test_validate() {
        // Valid configuration - float values should be clamped
        let mut valid_config = LlmConfig::builder()
            .with_default_temperature(0.7)
            .with_default_presence_penalty(0.1)
            .with_default_frequency_penalty(-0.1)
            .with_default_top_p(0.9)
            .with_default_max_tokens(1000u32)
            .build()
            .unwrap();
        assert!(valid_config.validate().is_ok());

        // Temperature should be clamped to valid range
        let mut clamped_temp = LlmConfig::builder()
            .with_default_temperature(3.0)
            .build()
            .unwrap();
        clamped_temp.validate().unwrap();
        assert_eq!(clamped_temp.default_temperature, Some(2.0));

        // Top-p should be clamped to valid range
        let mut clamped_top_p = LlmConfig::builder()
            .with_default_top_p(0.0)
            .build()
            .unwrap();
        clamped_top_p.validate().unwrap();
        assert_eq!(clamped_top_p.default_top_p, Some(0.001));

        // Invalid max tokens should return error
        let mut invalid_max_tokens = LlmConfig::builder()
            .with_default_max_tokens(0u32)
            .build()
            .unwrap();
        assert!(invalid_max_tokens.validate().is_err());

        // Invalid base URL should return error
        let mut invalid_base_url = LlmConfig::builder()
            .with_base_url("invalid-url")
            .build()
            .unwrap();
        assert!(invalid_base_url.validate().is_err());
    }
}
