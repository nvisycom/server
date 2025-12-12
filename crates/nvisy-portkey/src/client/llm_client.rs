//! Portkey API client implementation.
//!
//! This module provides the main client for interacting with Portkey's AI Gateway,
//! enabling unified access to 200+ AI providers.

use std::fmt;

use portkey_sdk::builder::AuthMethod;
use portkey_sdk::{PortkeyClient, PortkeyConfig};

use super::llm_config::LlmConfig;
use crate::{Result, TRACING_TARGET_CLIENT};

/// Portkey API client with comprehensive configuration.
///
/// This client provides a high-level interface to the Portkey AI Gateway with
/// error handling and observability features.
///
/// # Features
///
/// - **Unified AI Access**: Connect to 200+ AI providers through a single interface
/// - **Error Handling**: Comprehensive error types with recovery strategies
/// - **Observability**: Structured logging and request tracking
/// - **Configuration**: Flexible configuration with sensible defaults
/// - **Caching**: Built-in caching support via Portkey's gateway
///
/// # Clone Semantics
///
/// This client is cheap to clone as the underlying `PortkeyClient` uses Arc internally.
#[derive(Clone)]
pub struct LlmClient {
    client: PortkeyClient,
    config: LlmConfig,
}

impl LlmClient {
    /// Creates a new Portkey client from a configuration.
    ///
    /// This method is the primary constructor when you have an [`LlmConfig`] instance.
    /// The configuration specifies the API key, virtual keys, timeouts, and default
    /// model parameters.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use nvisy_portkey::{LlmClient, LlmConfig};
    /// let config = LlmConfig::builder()
    ///     .with_api_key("your-api-key")
    ///     .with_virtual_key("your-virtual-key")
    ///     .with_default_model("gpt-4")
    ///     .build()
    ///     .unwrap();
    ///
    /// let client = LlmClient::new(config).unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying Portkey client cannot be initialized.
    pub fn new(config: LlmConfig) -> Result<Self> {
        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            base_url = config.base_url(),
            "Building Portkey client from configuration"
        );

        let mut builder = PortkeyConfig::builder()
            .with_api_key(config.api_key())
            .with_base_url(config.base_url())
            .with_timeout(config.request_timeout());

        if let Some(virtual_key) = config.virtual_key() {
            builder = builder.with_auth_method(AuthMethod::virtual_key(virtual_key));
            tracing::debug!(
                target: TRACING_TARGET_CLIENT,
                "Set virtual key for authentication"
            );
        }

        if let Some(trace_id) = config.trace_id() {
            builder = builder.with_trace_id(trace_id);
            tracing::debug!(
                target: TRACING_TARGET_CLIENT,
                trace_id = trace_id,
                "Set trace ID for request tracking"
            );
        }

        if let Some(cache_namespace) = config.cache_namespace() {
            builder = builder.with_cache_namespace(cache_namespace);
            tracing::debug!(
                target: TRACING_TARGET_CLIENT,
                cache_namespace = cache_namespace,
                "Set cache namespace"
            );
        }

        if let Some(cache_force_refresh) = config.cache_force_refresh() {
            builder = builder.with_cache_force_refresh(cache_force_refresh);
            tracing::debug!(
                target: TRACING_TARGET_CLIENT,
                cache_force_refresh = cache_force_refresh,
                "Set cache force refresh"
            );
        }

        let client = builder.build_client()?;
        Self::with_client(client, config)
    }

    /// Creates a new Portkey client with a pre-configured Portkey client and custom configuration.
    ///
    /// This is useful when you need fine-grained control over the underlying Portkey client
    /// or when integrating with existing Portkey client instances.
    ///
    /// # Parameters
    ///
    /// - `client`: Pre-configured Portkey API client
    /// - `config`: Configuration for client behavior
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use nvisy_portkey::{LlmClient, LlmConfig};
    /// # use portkey_sdk::{PortkeyClient, PortkeyConfig};
    /// let portkey_client = PortkeyConfig::builder()
    ///     .with_api_key("your-api-key")
    ///     .build_client()
    ///     .unwrap();
    ///
    /// let config = LlmConfig::builder()
    ///     .with_api_key("your-api-key")
    ///     .build()
    ///     .unwrap();
    ///
    /// let client = LlmClient::with_client(portkey_client, config).unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// This method currently does not return errors but the signature allows for future validation.
    pub fn with_client(client: PortkeyClient, config: LlmConfig) -> Result<Self> {
        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            base_url = config.base_url(),
            "Initializing LlmClient with configuration"
        );

        tracing::info!(
            target: TRACING_TARGET_CLIENT,
            "LlmClient initialized successfully"
        );

        Ok(Self { client, config })
    }

    /// Creates a new Portkey client from an API key.
    ///
    /// Uses default configuration optimized for general usage. This is the
    /// simplest way to create a client when you only need to provide an API key.
    ///
    /// # Parameters
    ///
    /// - `api_key`: Your Portkey API key
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use nvisy_portkey::LlmClient;
    /// let client = LlmClient::from_api_key("your-api-key").unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The API key is invalid or empty
    /// - The client cannot be initialized
    pub fn from_api_key(api_key: impl Into<String>) -> Result<Self> {
        let config = LlmConfig::builder().with_api_key(api_key).build()?;
        Self::new(config)
    }

    /// Creates a new Portkey client from an API key and virtual key.
    ///
    /// This is a convenience method for the common pattern of using both
    /// an API key and a virtual key for routing.
    ///
    /// # Parameters
    ///
    /// - `api_key`: Your Portkey API key
    /// - `virtual_key`: Your Portkey virtual key for routing
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use nvisy_portkey::LlmClient;
    /// let client = LlmClient::from_keys("your-api-key", "your-virtual-key").unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Either key is invalid or empty
    /// - The client cannot be initialized
    pub fn from_keys(api_key: impl Into<String>, virtual_key: impl Into<String>) -> Result<Self> {
        let config = LlmConfig::builder()
            .with_api_key(api_key)
            .with_virtual_key(virtual_key)
            .build()?;
        Self::new(config)
    }

    /// Returns a reference to the client's configuration.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use nvisy_portkey::LlmClient;
    /// let client = LlmClient::from_api_key("your-api-key").unwrap();
    /// let config = client.as_config();
    /// println!("Base URL: {}", config.base_url());
    /// ```
    pub fn as_config(&self) -> &LlmConfig {
        &self.config
    }

    /// Returns a reference to the underlying Portkey client.
    ///
    /// This provides direct access to the Portkey client for advanced use cases
    /// where you need to access client-specific methods.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use nvisy_portkey::LlmClient;
    /// let client = LlmClient::from_api_key("your-api-key").unwrap();
    /// let inner_client = client.as_client();
    /// // Use inner_client directly for advanced operations
    /// ```
    pub fn as_client(&self) -> &PortkeyClient {
        &self.client
    }
}

impl fmt::Debug for LlmClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LlmClient")
            .field("config", &self.config)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() -> Result<()> {
        let config = LlmConfig::builder().with_api_key("test_key").build()?;
        assert_eq!(config.base_url(), "https://api.portkey.ai/v1");
        Ok(())
    }

    #[test]
    fn test_config_with_custom_values() -> Result<()> {
        let config = LlmConfig::builder()
            .with_api_key("test_key")
            .with_virtual_key("test_virtual_key")
            .with_default_model("gpt-4")
            .build()?;

        assert_eq!(config.virtual_key().unwrap(), "test_virtual_key");
        assert_eq!(config.default_model().unwrap(), "gpt-4");
        Ok(())
    }

    #[test]
    fn test_client_debug() {
        let config = LlmConfig::builder()
            .with_api_key("test_key")
            .build()
            .unwrap();
        let debug_str = format!("{:?}", config);
        assert!(!debug_str.is_empty());
        assert!(debug_str.contains("LlmConfig"));
    }

    #[test]
    fn test_masked_api_key_in_debug() {
        let config = LlmConfig::builder()
            .with_api_key("secret_key_12345")
            .build()
            .unwrap();

        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("secr****"));
        assert!(!debug_str.contains("secret_key_12345"));
    }
}
