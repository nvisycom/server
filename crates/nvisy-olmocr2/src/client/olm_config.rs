//! OCR client configuration
//!
//! This module provides configuration structures and builders for the OCR client.

use std::time::Duration;

use derive_builder::Builder;
use url::Url;

use crate::error::{Error, Result};

/// Configuration for the OCR client
///
/// Contains all the settings needed to configure the OCR client behavior,
/// including timeouts, retry settings, and API endpoints.
#[derive(Debug, Clone, Builder)]
#[builder(
    name = "OlmBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(validate = "Self::validate_config")
)]
pub struct OlmConfig {
    /// Base URL for the OCR API
    #[builder(setter(custom), default = "OlmConfig::default_base_url()")]
    pub base_url: Url,
    /// Request timeout duration
    #[builder(default = "Duration::from_secs(30)")]
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
    #[builder(default = "OlmConfig::default_user_agent()")]
    pub user_agent: String,
    /// Enable request/response logging
    #[builder(default = "false")]
    pub enable_logging: bool,
}

impl Default for OlmConfig {
    fn default() -> Self {
        Self {
            base_url: Self::default_base_url(),
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            max_retries: 3,
            max_concurrent_requests: 10,
            user_agent: Self::default_user_agent(),
            enable_logging: false,
        }
    }
}

impl OlmConfig {
    /// Create a new configuration builder
    pub fn builder() -> OlmBuilder {
        OlmBuilder::default()
    }

    fn default_base_url() -> Url {
        "https://api.olmo.ai/v2".parse().expect("Valid default URL")
    }

    fn default_user_agent() -> String {
        format!("nvisy-olmocr2/{}", env!("CARGO_PKG_VERSION"))
    }
}

impl OlmBuilder {
    /// Set the base URL for the OCR API
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
        let config = OlmConfig::builder()
            .with_timeout(Duration::from_secs(60))
            .with_max_retries(5)
            .with_enable_logging(true)
            .build()
            .expect("Valid config");

        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.max_retries, 5);
        assert!(config.enable_logging);
    }

    #[test]
    fn test_default_config() {
        let config = OlmConfig::default();

        assert_eq!(config.base_url.as_str(), "https://api.olmo.ai/v2/");
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.max_concurrent_requests, 10);
        assert!(!config.enable_logging);
    }

    #[test]
    fn test_custom_base_url() {
        let config = OlmConfig::builder()
            .with_base_url("https://custom-ocr.example.com/v1")
            .expect("Valid URL")
            .build()
            .expect("Valid config");

        assert_eq!(
            config.base_url.as_str(),
            "https://custom-ocr.example.com/v1"
        );
    }

    #[test]
    fn test_invalid_base_url() {
        let result = OlmConfig::builder().with_base_url("not-a-valid-url");

        assert!(result.is_err());
    }

    #[test]
    fn test_validation_zero_timeout() {
        let result = OlmConfig::builder()
            .with_timeout(Duration::from_secs(0))
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_validation_zero_concurrent_requests() {
        let result = OlmConfig::builder().with_max_concurrent_requests(0).build();

        assert!(result.is_err());
    }
}
