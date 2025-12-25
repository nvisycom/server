//! Qdrant client configuration.

use std::time::Duration;

#[cfg(feature = "config")]
use clap::Args;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Configuration for Qdrant client connections.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(Args))]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct QdrantConfig {
    /// Qdrant cluster endpoint (e.g., "http://localhost:6334")
    #[cfg_attr(
        feature = "config",
        arg(
            long,
            env = "QDRANT_CLUSTER_ENDPOINT",
            default_value = "http://localhost:6334"
        )
    )]
    pub url: String,

    /// API key for authentication
    #[cfg_attr(feature = "config", arg(long, env = "QDRANT_API_KEY"))]
    pub api_key: String,

    /// Connection timeout in seconds (optional)
    #[cfg_attr(feature = "config", arg(long, env = "QDRANT_CONNECT_TIMEOUT_SECS"))]
    pub connect_timeout_secs: Option<u64>,

    /// Request timeout in seconds (optional)
    #[cfg_attr(feature = "config", arg(long, env = "QDRANT_TIMEOUT_SECS"))]
    pub timeout_secs: Option<u64>,

    /// Maximum number of concurrent connections
    #[cfg_attr(feature = "config", arg(long, env = "QDRANT_POOL_SIZE"))]
    pub pool_size: Option<usize>,

    /// Enable keep-alive while idle
    #[cfg_attr(feature = "config", arg(long, env = "QDRANT_KEEP_ALIVE"))]
    pub keep_alive: Option<bool>,
}

impl QdrantConfig {
    /// Create a new Qdrant configuration with the given URL and API key.
    ///
    /// # Arguments
    ///
    /// * `url` - The Qdrant cluster endpoint (e.g., "http://localhost:6334")
    /// * `api_key` - The API key for authentication
    pub fn new(url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            api_key: api_key.into(),
            connect_timeout_secs: None,
            timeout_secs: None,
            pool_size: None,
            keep_alive: None,
        }
    }

    /// Returns the connection timeout as a Duration, if set.
    #[inline]
    pub fn connect_timeout(&self) -> Option<Duration> {
        self.connect_timeout_secs.map(Duration::from_secs)
    }

    /// Returns the request timeout as a Duration, if set.
    #[inline]
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout_secs.map(Duration::from_secs)
    }

    /// Returns the user-agent string for the Qdrant client.
    #[inline]
    pub fn user_agent(&self) -> String {
        format!(
            "nvisy-qdrant/{} ({})",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS
        )
    }

    /// Set the API key for authentication.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = api_key.into();
        self
    }

    /// Set the connection timeout in seconds.
    pub fn with_connect_timeout_secs(mut self, secs: u64) -> Self {
        self.connect_timeout_secs = Some(secs);
        self
    }

    /// Set the request timeout in seconds.
    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Set the connection pool size.
    pub fn with_pool_size(mut self, size: usize) -> Self {
        self.pool_size = Some(size);
        self
    }

    /// Enable or disable keep-alive while idle.
    pub fn with_keep_alive(mut self, enable: bool) -> Self {
        self.keep_alive = Some(enable);
        self
    }

    /// Get the base URL without path.
    pub fn base_url(&self) -> &str {
        &self.url
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<()> {
        if self.url.is_empty() {
            return Err(Error::configuration().with_message("URL cannot be empty"));
        }

        if !self.url.starts_with("http://") && !self.url.starts_with("https://") {
            return Err(
                Error::configuration().with_message("URL must start with http:// or https://")
            );
        }

        if self.api_key.is_empty() {
            return Err(Error::configuration().with_message("API key cannot be empty"));
        }

        if self.pool_size.is_some_and(|pool_size| pool_size == 0) {
            return Err(Error::configuration().with_message("Pool size must be greater than zero"));
        }

        Ok(())
    }
}

impl Default for QdrantConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:6334".to_string(),
            api_key: String::new(),
            connect_timeout_secs: None,
            timeout_secs: None,
            pool_size: None,
            keep_alive: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = QdrantConfig::new("http://localhost:6334", "test-api-key");
        assert_eq!(config.url, "http://localhost:6334");
        assert_eq!(config.api_key, "test-api-key");
        assert_eq!(config.connect_timeout(), None);
        assert_eq!(config.timeout(), None);
    }

    #[test]
    fn test_config_validation() {
        let valid_config = QdrantConfig::new("http://localhost:6334", "key");
        assert!(valid_config.validate().is_ok());

        // Empty URL
        let empty_url = QdrantConfig::new("", "key");
        assert!(empty_url.validate().is_err());

        // Invalid URL format
        let invalid_url = QdrantConfig::new("not-a-url", "key");
        assert!(invalid_url.validate().is_err());

        // Invalid URL scheme
        let ftp_url = QdrantConfig::new("ftp://localhost", "key");
        assert!(ftp_url.validate().is_err());

        // Empty API key
        let empty_api_key = QdrantConfig::new("http://localhost:6334", "");
        assert!(empty_api_key.validate().is_err());

        // Invalid pool size
        let mut invalid_pool = QdrantConfig::new("http://localhost:6334", "key");
        invalid_pool.pool_size = Some(0);
        assert!(invalid_pool.validate().is_err());
    }

    #[test]
    fn test_duration_helpers() {
        let config = QdrantConfig::new("http://localhost:6334", "key");
        assert_eq!(config.connect_timeout(), None);
        assert_eq!(config.timeout(), None);

        let config = config.with_connect_timeout_secs(10).with_timeout_secs(30);
        assert_eq!(config.connect_timeout(), Some(Duration::from_secs(10)));
        assert_eq!(config.timeout(), Some(Duration::from_secs(30)));
    }
}
