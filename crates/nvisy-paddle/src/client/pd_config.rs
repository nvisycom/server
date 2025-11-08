//! Configuration for PaddleX HTTP client.

use std::time::Duration;

use url::Url;

use crate::{Error, Result};

/// Configuration for the PaddleX HTTP client.
///
/// This struct holds all the configuration needed to connect to and interact with
/// PaddleX services (PaddleOCR-VL, etc.).
///
/// # Examples
///
/// ```ignore
/// use nvisy_paddle::PdConfig;
/// use std::time::Duration;
///
/// // Basic configuration
/// let config = PdConfig::new("http://localhost:8080");
///
/// // Advanced configuration
/// let config = PdConfig::builder()
///     .base_url("http://paddlex-service:8080")
///     .timeout(Duration::from_secs(60))
///     .api_key("my-secret-key")
///     .max_retries(3)
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct PdConfig {
    /// Base URL of the PaddleX service
    base_url: Url,

    /// API key for authentication (if required)
    api_key: Option<String>,

    /// Request timeout duration
    timeout: Duration,

    /// Maximum number of retry attempts for retryable errors
    max_retries: u32,

    /// Base delay for exponential backoff
    retry_backoff: Duration,

    /// User agent string for HTTP requests
    user_agent: String,

    /// Whether to verify SSL certificates
    verify_ssl: bool,

    /// Custom HTTP headers to include in all requests
    custom_headers: Vec<(String, String)>,
}

impl PdConfig {
    /// Create a new configuration with the given base URL and default settings.
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL of the PaddleX service (e.g., "http://localhost:8080")
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use nvisy_paddle::PdConfig;
    ///
    /// let config = PdConfig::new("http://localhost:8080");
    /// ```
    pub fn new(base_url: impl AsRef<str>) -> Result<Self> {
        let base_url = Url::parse(base_url.as_ref()).map_err(|e| {
            Error::config(format!("Invalid base URL '{}': {}", base_url.as_ref(), e))
        })?;

        Ok(Self {
            base_url,
            api_key: None,
            timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_backoff: Duration::from_millis(500),
            user_agent: format!("nvisy-paddle/{}", env!("CARGO_PKG_VERSION")),
            verify_ssl: true,
            custom_headers: Vec::new(),
        })
    }

    /// Create a new configuration builder.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use nvisy_paddle::PdConfig;
    /// use std::time::Duration;
    ///
    /// let config = PdConfig::builder()
    ///     .base_url("http://localhost:8080")
    ///     .timeout(Duration::from_secs(60))
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn builder() -> PdConfigBuilder {
        PdConfigBuilder::default()
    }

    /// Get the base URL of the PaddleX service.
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Get the API key (if configured).
    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    /// Get the request timeout.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Get the maximum number of retry attempts.
    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }

    /// Get the retry backoff duration.
    pub fn retry_backoff(&self) -> Duration {
        self.retry_backoff
    }

    /// Get the user agent string.
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// Get whether SSL verification is enabled.
    pub fn verify_ssl(&self) -> bool {
        self.verify_ssl
    }

    /// Get custom headers.
    pub fn custom_headers(&self) -> &[(String, String)] {
        &self.custom_headers
    }

    /// Set the API key.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Set the request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the maximum number of retries.
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set the retry backoff duration.
    pub fn with_retry_backoff(mut self, backoff: Duration) -> Self {
        self.retry_backoff = backoff;
        self
    }

    /// Set a custom user agent.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    /// Set whether to verify SSL certificates.
    pub fn with_verify_ssl(mut self, verify: bool) -> Self {
        self.verify_ssl = verify;
        self
    }

    /// Add a custom header to all requests.
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_headers.push((key.into(), value.into()));
        self
    }
}

impl Default for PdConfig {
    fn default() -> Self {
        Self::new("http://localhost:8080").expect("Default URL should be valid")
    }
}

/// Builder for [`PdConfig`].
///
/// Provides a fluent interface for constructing PaddleX client configuration.
#[derive(Debug, Default)]
pub struct PdConfigBuilder {
    base_url: Option<String>,
    api_key: Option<String>,
    timeout: Option<Duration>,
    max_retries: Option<u32>,
    retry_backoff: Option<Duration>,
    user_agent: Option<String>,
    verify_ssl: Option<bool>,
    custom_headers: Vec<(String, String)>,
}

impl PdConfigBuilder {
    /// Set the base URL of the PaddleX service.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Set the API key for authentication.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the maximum number of retry attempts.
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = Some(retries);
        self
    }

    /// Set the retry backoff duration.
    pub fn retry_backoff(mut self, backoff: Duration) -> Self {
        self.retry_backoff = Some(backoff);
        self
    }

    /// Set a custom user agent.
    pub fn user_agent(mut self, agent: impl Into<String>) -> Self {
        self.user_agent = Some(agent.into());
        self
    }

    /// Set whether to verify SSL certificates.
    pub fn verify_ssl(mut self, verify: bool) -> Self {
        self.verify_ssl = Some(verify);
        self
    }

    /// Add a custom header to all requests.
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_headers.push((key.into(), value.into()));
        self
    }

    /// Build the configuration.
    ///
    /// Returns an error if the base URL is not set or is invalid.
    pub fn build(self) -> Result<PdConfig> {
        let base_url = self
            .base_url
            .ok_or_else(|| Error::config("Base URL is required"))?;

        let mut config = PdConfig::new(base_url)?;

        if let Some(api_key) = self.api_key {
            config = config.with_api_key(api_key);
        }

        if let Some(timeout) = self.timeout {
            config = config.with_timeout(timeout);
        }

        if let Some(max_retries) = self.max_retries {
            config = config.with_max_retries(max_retries);
        }

        if let Some(retry_backoff) = self.retry_backoff {
            config = config.with_retry_backoff(retry_backoff);
        }

        if let Some(user_agent) = self.user_agent {
            config = config.with_user_agent(user_agent);
        }

        if let Some(verify_ssl) = self.verify_ssl {
            config = config.with_verify_ssl(verify_ssl);
        }

        for (key, value) in self.custom_headers {
            config = config.with_header(key, value);
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_config() {
        let config = PdConfig::new("http://localhost:8080").unwrap();
        assert_eq!(config.base_url().as_str(), "http://localhost:8080/");
        assert_eq!(config.api_key(), None);
        assert_eq!(config.timeout(), Duration::from_secs(30));
        assert_eq!(config.max_retries(), 3);
        assert!(config.verify_ssl());
    }

    #[test]
    fn test_invalid_url() {
        let result = PdConfig::new("not a valid url");
        assert!(result.is_err());
        assert!(result.unwrap_err().is_client_error());
    }

    #[test]
    fn test_builder() {
        let config = PdConfig::builder()
            .base_url("https://api.paddlex.com")
            .api_key("test-key")
            .timeout(Duration::from_secs(60))
            .max_retries(5)
            .verify_ssl(false)
            .header("X-Custom", "value")
            .build()
            .unwrap();

        assert_eq!(config.base_url().scheme(), "https");
        assert_eq!(config.api_key(), Some("test-key"));
        assert_eq!(config.timeout(), Duration::from_secs(60));
        assert_eq!(config.max_retries(), 5);
        assert!(!config.verify_ssl());
        assert_eq!(config.custom_headers().len(), 1);
    }

    #[test]
    fn test_builder_missing_url() {
        let result = PdConfig::builder()
            .api_key("test-key")
            .timeout(Duration::from_secs(30))
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_fluent_api() {
        let config = PdConfig::new("http://localhost:8080")
            .unwrap()
            .with_api_key("my-key")
            .with_timeout(Duration::from_secs(45))
            .with_max_retries(10)
            .with_header("Authorization", "Bearer token");

        assert_eq!(config.api_key(), Some("my-key"));
        assert_eq!(config.timeout(), Duration::from_secs(45));
        assert_eq!(config.max_retries(), 10);
        assert_eq!(config.custom_headers().len(), 1);
    }

    #[test]
    fn test_default_config() {
        let config = PdConfig::default();
        assert_eq!(config.base_url().as_str(), "http://localhost:8080/");
        assert_eq!(config.timeout(), Duration::from_secs(30));
    }
}
