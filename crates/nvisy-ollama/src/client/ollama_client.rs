//! Ollama client implementation
//!
//! This module provides the main client interface for Ollama API operations.
//! It handles authentication, request/response processing, and connection management.

use reqwest::{Client as HttpClient, ClientBuilder};

use super::{OllamaConfig, OllamaCredentials};
use crate::{Error, Result, TRACING_TARGET_CLIENT};

/// Ollama client for interacting with Ollama API services
///
/// The client handles authentication, request routing, and connection pooling
/// for optimal performance when working with language models.
///
/// # Examples
///
/// ```rust
/// use nvisy_ollama::client::{OllamaClient, OllamaConfig, OllamaCredentials};
/// use std::time::Duration;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = OllamaConfig::builder()
///     .with_base_url("http://localhost:11434")?
///     .with_timeout(Duration::from_secs(30))
///     .build()?;
///
/// let credentials = OllamaCredentials::none();
/// let client = OllamaClient::new(config, credentials).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct OllamaClient {
    http_client: HttpClient,
    config: OllamaConfig,
    credentials: OllamaCredentials,
}

impl OllamaClient {
    /// Create a new Ollama client with the given configuration and credentials
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
    pub async fn new(config: OllamaConfig, credentials: OllamaCredentials) -> Result<Self> {
        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            base_url = %config.base_url,
            "Creating Ollama client"
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
            "Ollama client created successfully"
        );

        Ok(client)
    }

    /// Create a new Ollama client with default configuration
    ///
    /// # Arguments
    ///
    /// * `base_url` - Base URL for the Ollama API
    /// * `credentials` - Authentication credentials
    pub async fn with_defaults(
        base_url: impl AsRef<str>,
        credentials: OllamaCredentials,
    ) -> Result<Self> {
        let config = OllamaConfig::builder()
            .with_base_url(base_url.as_ref())?
            .build()?;

        Self::new(config, credentials).await
    }

    /// Perform a health check against the Ollama service
    ///
    /// This method verifies that the service is accessible and responding.
    pub async fn health_check(&self) -> Result<()> {
        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            "Performing health check"
        );

        let url = self
            .config
            .base_url
            .join("/api/tags")
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
    pub fn config(&self) -> &OllamaConfig {
        &self.config
    }

    /// Get the client credentials (for debugging/logging purposes only)
    pub fn credentials_type(&self) -> &'static str {
        match &self.credentials {
            OllamaCredentials::ApiKey(_) => "api_key",
            OllamaCredentials::BearerToken(_) => "bearer_token",
            OllamaCredentials::Basic { .. } => "basic_auth",
            OllamaCredentials::None => "none",
        }
    }

    /// Add authentication headers to a request
    fn add_auth_headers(&self, mut request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.credentials {
            OllamaCredentials::ApiKey(key) => {
                request = request.header("Authorization", format!("Bearer {}", key));
            }
            OllamaCredentials::BearerToken(token) => {
                request = request.header("Authorization", format!("Bearer {}", token));
            }
            OllamaCredentials::Basic { username, password } => {
                request = request.basic_auth(username, Some(password));
            }
            OllamaCredentials::None => {
                // No authentication headers needed for local Ollama instances
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

    // TODO: Add Ollama-specific API methods
    // - list_models()
    // - generate()
    // - chat()
    // - embeddings()
    // - create_model()
    // - delete_model()
    // - pull_model()
    // - push_model()
}
