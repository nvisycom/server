//! OCR client implementation
//!
//! This module provides the main client interface for OCR operations using OLMo v2 models.
//! It handles authentication, request/response processing, and connection management.

use reqwest::{Client as HttpClient, ClientBuilder};

use super::{OlemCredentials, OlmConfig};
use crate::TRACING_TARGET_CLIENT;
use crate::error::{Error, Result};

/// OCR client for interacting with OLMo v2 OCR services.
///
/// The client handles authentication, request batching, and connection pooling
/// for optimal performance when processing documents.
///
/// # Examples
///
/// ```rust,ignore
/// use nvisy_olmocr2::{OlmClient, OlmConfig, OlemCredentials};
/// use std::time::Duration;
///
/// let config = OlmConfig::builder()
///     .with_base_url("https://api.olmo.ai/v2")?
///     .with_timeout(Duration::from_secs(30))
///     .build()?;
///
/// let credentials = OlemCredentials::api_key("your-api-key");
/// let client = OlmClient::new(config, credentials).await?;
/// ```
#[derive(Debug, Clone)]
pub struct OlmClient {
    http_client: HttpClient,
    config: OlmConfig,
    credentials: OlemCredentials,
}

impl OlmClient {
    /// Create a new OCR client with the given configuration and credentials.
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
    pub async fn new(config: OlmConfig, credentials: OlemCredentials) -> Result<Self> {
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

        tracing::info!(
            target: TRACING_TARGET_CLIENT,
            "OCR client created successfully"
        );

        Ok(client)
    }

    /// Create a new OCR client with default configuration.
    ///
    /// # Arguments
    ///
    /// * `base_url` - Base URL for the OCR API
    /// * `credentials` - Authentication credentials
    pub async fn with_defaults(
        base_url: impl AsRef<str>,
        credentials: OlemCredentials,
    ) -> Result<Self> {
        let config = OlmConfig::builder()
            .with_base_url(base_url.as_ref())?
            .build()?;

        Self::new(config, credentials).await
    }

    /// Perform a health check against the OCR service.
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

    /// Get the client configuration.
    pub fn config(&self) -> &OlmConfig {
        &self.config
    }

    /// Get the HTTP client for making requests.
    pub(crate) fn http_client(&self) -> &HttpClient {
        &self.http_client
    }

    /// Get the credentials type (for debugging/logging purposes only).
    pub fn credentials_type(&self) -> &'static str {
        match &self.credentials {
            OlemCredentials::ApiKey(_) => "api_key",
            OlemCredentials::BearerToken(_) => "bearer_token",
            OlemCredentials::Basic { .. } => "basic_auth",
            OlemCredentials::None => "none",
        }
    }

    /// Add authentication headers to a request.
    pub(crate) fn add_auth_headers(
        &self,
        mut request: reqwest::RequestBuilder,
    ) -> reqwest::RequestBuilder {
        match &self.credentials {
            OlemCredentials::ApiKey(key) => {
                request = request.header("X-API-Key", key);
            }
            OlemCredentials::BearerToken(token) => {
                request = request.header("Authorization", format!("Bearer {}", token));
            }
            OlemCredentials::Basic { username, password } => {
                request = request.basic_auth(username, Some(password));
            }
            OlemCredentials::None => {
                // No authentication headers needed
            }
        }
        request
    }
}
