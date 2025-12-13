//! OCR client module
//!
//! This module provides the main client interface for OCR operations using OLMo v2 models.
//! It handles authentication, request/response processing, and connection management.

use std::time::Duration;

use reqwest::{Client as HttpClient, ClientBuilder};
use url::Url;

use crate::{Error, Result, TRACING_TARGET_CLIENT};

/// OCR client for interacting with OLMo v2 OCR services
///
/// The client handles authentication, request batching, and connection pooling
/// for optimal performance when processing documents.
///
/// # Examples
///
/// ```rust
/// use nvisy_olmocr2::client::{OcrClient, OcrConfig, OcrCredentials};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = OcrConfig::builder()
///     .base_url("https://api.olmo.ai/v2")
///     .timeout(Duration::from_secs(30))
///     .build()?;
///
/// let credentials = OcrCredentials::api_key("your-api-key");
/// let client = OcrClient::new(config, credentials).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct OcrClient {
    http_client: HttpClient,
    config: OcrConfig,
    credentials: OcrCredentials,
}

/// Configuration for the OCR client
///
/// Contains all the settings needed to configure the OCR client behavior,
/// including timeouts, retry settings, and API endpoints.
#[derive(Debug, Clone)]
pub struct OcrConfig {
    /// Base URL for the OCR API
    pub base_url: Url,
    /// Request timeout duration
    pub timeout: Duration,
    /// Connection timeout duration
    pub connect_timeout: Duration,
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Maximum concurrent requests
    pub max_concurrent_requests: usize,
    /// User agent string for requests
    pub user_agent: String,
    /// Enable request/response logging
    pub enable_logging: bool,
}

/// Authentication credentials for the OCR service
///
/// Supports different authentication methods including API keys,
/// bearer tokens, and basic authentication.
#[derive(Debug, Clone)]
pub enum OcrCredentials {
    /// API key authentication
    ApiKey(String),
    /// Bearer token authentication
    BearerToken(String),
    /// Basic authentication with username and password
    Basic { username: String, password: String },
    /// No authentication (for testing/development)
    None,
}

impl OcrClient {
    /// Create a new OCR client with the given configuration and credentials
    ///
    /// # Arguments
    ///
    /// * `config` - Client configuration
    /// * `credentials` - Authentication credentials
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created or if the
    /// configuration is invalid.
    pub async fn new(config: OcrConfig, credentials: OcrCredentials) -> Result<Self> {
        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            base_url = %config.base_url,
            "Creating OCR client"
        );

        let http_client = ClientBuilder::new()
            .timeout(config.timeout)
            .connect_timeout(config.connect_timeout)
            .user_agent(&config.user_agent)
            .build()
            .map_err(Error::Http)?;

        let client = Self {
            http_client,
            config,
            credentials,
        };

        // Verify connection by making a health check
        client.health_check().await?;

        tracing::info!(
            target: TRACING_TARGET_CLIENT,
            "OCR client created successfully"
        );

        Ok(client)
    }

    /// Create a new OCR client with default configuration
    ///
    /// # Arguments
    ///
    /// * `base_url` - Base URL for the OCR API
    /// * `credentials` - Authentication credentials
    pub async fn with_defaults(
        base_url: impl AsRef<str>,
        credentials: OcrCredentials,
    ) -> Result<Self> {
        let config = OcrConfig::builder().base_url(base_url.as_ref())?.build();

        Self::new(config, credentials).await
    }

    /// Perform a health check against the OCR service
    ///
    /// This method verifies that the service is accessible and the credentials are valid.
    pub async fn health_check(&self) -> Result<()> {
        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            "Performing health check"
        );

        let url = self
            .config
            .base_url
            .join("/health")
            .map_err(|e| Error::invalid_config(format!("Invalid health check URL: {}", e)))?;

        let mut request = self.http_client.get(url);
        request = self.add_auth_headers(request);

        let response = request.send().await.map_err(Error::Http)?;

        if response.status().is_success() {
            tracing::debug!(
                target: TRACING_TARGET_CLIENT,
                status = response.status().as_u16(),
                "Health check successful"
            );
            Ok(())
        } else {
            let status = response.status().as_u16();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            tracing::error!(
                target: TRACING_TARGET_CLIENT,
                status,
                message,
                "Health check failed"
            );

            Err(Error::api_error(status, message))
        }
    }

    /// Get the client configuration
    pub fn config(&self) -> &OcrConfig {
        &self.config
    }

    /// Get the client credentials (for debugging/logging purposes only)
    pub fn credentials_type(&self) -> &'static str {
        match &self.credentials {
            OcrCredentials::ApiKey(_) => "api_key",
            OcrCredentials::BearerToken(_) => "bearer_token",
            OcrCredentials::Basic { .. } => "basic_auth",
            OcrCredentials::None => "none",
        }
    }

    /// Add authentication headers to a request
    fn add_auth_headers(&self, mut request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.credentials {
            OcrCredentials::ApiKey(key) => {
                request = request.header("X-API-Key", key);
            }
            OcrCredentials::BearerToken(token) => {
                request = request.header("Authorization", format!("Bearer {}", token));
            }
            OcrCredentials::Basic { username, password } => {
                request = request.basic_auth(username, Some(password));
            }
            OcrCredentials::None => {
                // No authentication headers needed
            }
        }
        request
    }

    /// Create a new request builder with base configuration
    pub(crate) fn request(
        &self,
        method: reqwest::Method,
        path: &str,
    ) -> Result<reqwest::RequestBuilder> {
        let url = self
            .config
            .base_url
            .join(path)
            .map_err(|e| Error::invalid_config(format!("Invalid request URL: {}", e)))?;

        let request = self.http_client.request(method, url);
        let request = self.add_auth_headers(request);

        Ok(request)
    }
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.olmo.ai/v2".parse().expect("Valid default URL"),
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            max_retries: 3,
            max_concurrent_requests: 10,
            user_agent: format!("nvisy-olmocr2/{}", env!("CARGO_PKG_VERSION")),
            enable_logging: false,
        }
    }
}

impl OcrConfig {
    /// Create a new configuration builder
    pub fn builder() -> OcrConfigBuilder {
        OcrConfigBuilder::new()
    }
}

/// Builder for OCR client configuration
#[derive(Debug)]
pub struct OcrConfigBuilder {
    config: OcrConfig,
}

impl OcrConfigBuilder {
    /// Create a new configuration builder with defaults
    pub fn new() -> Self {
        Self {
            config: OcrConfig::default(),
        }
    }

    /// Set the base URL for the OCR API
    pub fn base_url(mut self, url: &str) -> Result<Self> {
        self.config.base_url = url
            .parse()
            .map_err(|e| Error::invalid_config(format!("Invalid base URL '{}': {}", url, e)))?;
        Ok(self)
    }

    /// Set the request timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    /// Set the connection timeout
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.config.connect_timeout = timeout;
        self
    }

    /// Set the maximum number of retry attempts
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.config.max_retries = retries;
        self
    }

    /// Set the maximum number of concurrent requests
    pub fn max_concurrent_requests(mut self, max: usize) -> Self {
        self.config.max_concurrent_requests = max;
        self
    }

    /// Set a custom user agent string
    pub fn user_agent(mut self, agent: impl Into<String>) -> Self {
        self.config.user_agent = agent.into();
        self
    }

    /// Enable or disable request/response logging
    pub fn enable_logging(mut self, enable: bool) -> Self {
        self.config.enable_logging = enable;
        self
    }

    /// Build the configuration
    pub fn build(self) -> OcrConfig {
        self.config
    }
}

impl Default for OcrConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl OcrCredentials {
    /// Create API key credentials
    pub fn api_key(key: impl Into<String>) -> Self {
        Self::ApiKey(key.into())
    }

    /// Create bearer token credentials
    pub fn bearer_token(token: impl Into<String>) -> Self {
        Self::BearerToken(token.into())
    }

    /// Create basic authentication credentials
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self::Basic {
            username: username.into(),
            password: password.into(),
        }
    }

    /// Create credentials with no authentication
    pub fn none() -> Self {
        Self::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = OcrConfig::builder()
            .timeout(Duration::from_secs(60))
            .max_retries(5)
            .enable_logging(true)
            .build();

        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.max_retries, 5);
        assert!(config.enable_logging);
    }

    #[test]
    fn test_credentials() {
        let api_key = OcrCredentials::api_key("test-key");
        let bearer = OcrCredentials::bearer_token("test-token");
        let basic = OcrCredentials::basic("user", "pass");
        let none = OcrCredentials::none();

        match api_key {
            OcrCredentials::ApiKey(key) => assert_eq!(key, "test-key"),
            _ => panic!("Expected API key credentials"),
        }

        match bearer {
            OcrCredentials::BearerToken(token) => assert_eq!(token, "test-token"),
            _ => panic!("Expected bearer token credentials"),
        }

        match basic {
            OcrCredentials::Basic { username, password } => {
                assert_eq!(username, "user");
                assert_eq!(password, "pass");
            }
            _ => panic!("Expected basic credentials"),
        }

        match none {
            OcrCredentials::None => {}
            _ => panic!("Expected no credentials"),
        }
    }
}
