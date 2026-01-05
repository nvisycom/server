//! Ollama client implementation.
//!
//! This module provides the main client interface for Ollama API operations.
//! It wraps the `ollama-rs` crate and provides integration with nvisy-service.

use std::sync::Arc;

use nvisy_inference::InferenceService;
use ollama_rs::Ollama;

use super::OllamaConfig;
use crate::{Error, Result, TRACING_TARGET_CLIENT};

/// Inner client that holds the actual ollama-rs client.
struct OllamaClientInner {
    ollama: Ollama,
    config: OllamaConfig,
}

impl std::fmt::Debug for OllamaClientInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OllamaClientInner")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// Ollama client for interacting with Ollama API services.
///
/// This client wraps the `ollama-rs` crate and implements the
/// `InferenceProvider` trait from nvisy-service.
///
/// # Examples
///
/// ```rust,ignore
/// use nvisy_ollama::{OllamaClient, OllamaConfig};
///
/// let config = OllamaConfig::default()
///     .with_embedding_model("nomic-embed-text")
///     .with_vlm_model("llava");
/// let client = OllamaClient::new(config)?;
/// ```
#[derive(Clone, Debug)]
pub struct OllamaClient {
    inner: Arc<OllamaClientInner>,
}

impl OllamaClient {
    /// Create a new Ollama client with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    pub fn new(config: OllamaConfig) -> Result<Self> {
        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            host = %config.host,
            port = config.port,
            "Creating Ollama client"
        );

        config.validate().map_err(Error::invalid_config)?;

        let host_with_scheme = format!("http://{}", config.host);
        let ollama = Ollama::new(host_with_scheme, config.port);

        let inner = OllamaClientInner { ollama, config };
        let client = Self {
            inner: Arc::new(inner),
        };

        tracing::info!(
            target: TRACING_TARGET_CLIENT,
            "Ollama client created successfully"
        );

        Ok(client)
    }

    /// Create a new Ollama client with default configuration (localhost:11434).
    pub fn with_defaults() -> Result<Self> {
        Self::new(OllamaConfig::default())
    }

    /// Perform a health check against the Ollama service.
    pub async fn health_check(&self) -> Result<()> {
        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            "Performing health check"
        );

        self.inner.ollama.list_local_models().await?;

        tracing::debug!(
            target: TRACING_TARGET_CLIENT,
            "Health check successful"
        );

        Ok(())
    }

    /// Get the client configuration.
    pub fn config(&self) -> &OllamaConfig {
        &self.inner.config
    }

    /// Get a reference to the inner ollama-rs client.
    pub(crate) fn ollama(&self) -> &Ollama {
        &self.inner.ollama
    }

    /// Get the embedding model name.
    pub(crate) fn embedding_model(&self) -> &str {
        self.inner
            .config
            .embedding_model
            .as_deref()
            .expect("embedding_model must be configured")
    }

    /// Get the VLM model name.
    pub(crate) fn vlm_model(&self) -> &str {
        self.inner
            .config
            .vlm_model
            .as_deref()
            .expect("vlm_model must be configured")
    }

    /// Convert this client into an [`InferenceService`].
    ///
    /// Creates an inference service backed by this Ollama client.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use nvisy_ollama::{OllamaClient, OllamaConfig};
    ///
    /// let config = OllamaConfig::default()
    ///     .with_embedding_model("nomic-embed-text")
    ///     .with_vlm_model("llava");
    /// let client = OllamaClient::new(config)?;
    /// let service = client.into_service();
    /// ```
    pub fn into_service(self) -> InferenceService {
        InferenceService::from_provider(self)
    }
}
