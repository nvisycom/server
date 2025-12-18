//! Ollama client configuration
//!
//! This module provides configuration structures and builders for the Ollama client.

use std::time::Duration;

use derive_builder::Builder;
use url::Url;

use crate::error::{Error, Result};

/// Configuration for the Ollama client
///
/// Contains all the settings needed to configure the Ollama client behavior,
/// including timeouts, retry settings, and API endpoints.
#[derive(Debug, Clone, Builder)]
#[builder(
    name = "OllamaBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate_config")
)]
pub struct OllamaConfig {
    /// Base URL for the Ollama API
    #[builder(setter(custom), default = "OllamaConfig::default_base_url()")]
    pub base_url: Url,
    /// Request timeout duration
    #[builder(default = "Duration::from_secs(60)")]
    pub timeout: Duration,
    /// Connection timeout duration
    #[builder(default = "Duration::from_secs(10)")]
    pub connect_timeout: Duration,
    /// Maximum number of retry attempts
    #[builder(default = "3")]
    pub max_retries: u32,
    /// Maximum concurrent requests
    #[builder(default = "10")]
    pub max_concurrent_requests: usize,
    /// User agent string for requests
    #[builder(default = "OllamaConfig::default_user_agent()")]
    pub user_agent: String,
    /// Enable request/response logging
    #[builder(default = "false")]
    pub enable_logging: bool,
    /// Enable streaming responses (for chat and generation)
    #[builder(default = "true")]
    pub enable_streaming: bool,
    /// Keep alive duration for persistent connections
    #[builder(default = "Some(Duration::from_secs(30))")]
    pub keep_alive: Option<Duration>,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: Self::default_base_url(),
            timeout: Duration::from_secs(60),
            connect_timeout: Duration::from_secs(10),
            max_retries: 3,
            max_concurrent_requests: 10,
            user_agent: Self::default_user_agent(),
            enable_logging: false,
            enable_streaming: true,
            keep_alive: Some(Duration::from_secs(30)),
        }
    }
}

impl OllamaConfig {
    /// Create a new configuration builder
    pub fn builder() -> OllamaBuilder {
        OllamaBuilder::default()
    }

    fn default_base_url() -> Url {
        "http://localhost:11434".parse().expect("Valid default URL")
    }

    fn default_user_agent() -> String {
        format!("nvisy-ollama/{}", env!("CARGO_PKG_VERSION"))
    }
}

impl OllamaBuilder {
    /// Set the base URL for the Ollama API
    pub fn with_base_url(mut self, url: &str) -> Result<Self> {
        self.base_url =
            Some(url.parse().map_err(|e| {
                Error::invalid_config(format!("Invalid base URL '{}': {}", url, e))
            })?);
        Ok(self)
    }

    fn validate_config(&self) -> std::result::Result<(), String> {
        // Validate timeout values
        if let Some(timeout) = &self.timeout {
            if timeout.as_secs() == 0 {
                return Err("Timeout must be greater than 0".to_string());
            }
        }

        if let Some(connect_timeout) = &self.connect_timeout {
            if connect_timeout.as_secs() == 0 {
                return Err("Connect timeout must be greater than 0".to_string());
            }
        }

        // Validate concurrent requests limit
        if let Some(max_concurrent) = &self.max_concurrent_requests {
            if *max_concurrent == 0 {
                return Err("Max concurrent requests must be greater than 0".to_string());
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = OllamaConfig::builder()
            .with_timeout(Duration::from_secs(120))
            .with_max_retries(5)
            .with_enable_logging(true)
            .with_enable_streaming(false)
            .build()
            .expect("Valid config");

        assert_eq!(config.timeout, Duration::from_secs(120));
        assert_eq!(config.max_retries, 5);
        assert!(config.enable_logging);
        assert!(!config.enable_streaming);
    }

    #[test]
    fn test_default_config() {
        let config = OllamaConfig::default();

        assert_eq!(config.base_url.as_str(), "http://localhost:11434/");
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.max_concurrent_requests, 10);
        assert!(!config.enable_logging);
        assert!(config.enable_streaming);
        assert_eq!(config.keep_alive, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_custom_base_url() {
        let config = OllamaConfig::builder()
            .with_base_url("http://remote-ollama:11434")
            .expect("Valid URL")
            .build()
            .expect("Valid config");

        assert_eq!(config.base_url.as_str(), "http://remote-ollama:11434/");
    }

    #[test]
    fn test_invalid_base_url() {
        let result = OllamaConfig::builder().with_base_url("not-a-valid-url");

        assert!(result.is_err());
    }

    #[test]
    fn test_validation_zero_timeout() {
        let result = OllamaConfig::builder()
            .with_timeout(Duration::from_secs(0))
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_validation_zero_concurrent_requests() {
        let result = OllamaConfig::builder()
            .with_max_concurrent_requests(0)
            .build();

        assert!(result.is_err());
    }
}
