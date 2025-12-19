//! Context management for embedding operations.
//!
//! This module provides context types for managing embedding service configuration,
//! model information, and request-specific settings.

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::request::EncodingFormat;

/// Context for embedding operations.
///
/// This struct contains configuration and metadata for embedding services,
/// including model settings, authentication information, and operational parameters.
///
/// # Examples
///
/// ```rust,ignore
/// use nvisy_core::emb::EmbeddingContext;
///
/// let context = EmbeddingContext::builder()
///     .model("text-embedding-ada-002")
///     .max_retries(3)
///     .timeout(Duration::from_secs(30))
///     .build();
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingContext {
    /// Unique identifier for this context.
    pub context_id: Uuid,

    /// The embedding service provider name.
    pub provider: String,

    /// The default model to use for embeddings.
    pub model: String,

    /// Available models for this service.
    pub available_models: Vec<ModelInfo>,

    /// Service endpoint configuration.
    pub endpoint: ServiceEndpoint,

    /// Authentication configuration.
    pub auth: AuthConfig,

    /// Retry configuration.
    pub retry_config: RetryConfig,

    /// Timeout settings.
    pub timeout_config: TimeoutConfig,

    /// Rate limiting settings.
    pub rate_limit_config: Option<RateLimitConfig>,

    /// Default encoding format for embeddings.
    pub default_encoding_format: EncodingFormat,

    /// Maximum number of inputs per batch request.
    pub max_batch_size: usize,

    /// Maximum token length per input.
    pub max_input_tokens: Option<u32>,

    /// Additional provider-specific configuration.
    pub provider_config: HashMap<String, serde_json::Value>,

    /// Context metadata.
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Information about an available embedding model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    /// The model identifier.
    pub id: String,

    /// Human-readable name for the model.
    pub name: String,

    /// Description of the model's capabilities.
    pub description: Option<String>,

    /// Embedding dimensions produced by this model.
    pub dimensions: u32,

    /// Maximum input tokens for this model.
    pub max_input_tokens: u32,

    /// Supported input types for this model.
    pub supported_input_types: Vec<InputType>,

    /// Whether this model supports custom dimensions.
    pub supports_custom_dimensions: bool,

    /// Model-specific metadata.
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Supported input types for embedding models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    /// Text input support.
    Text,
    /// Image input support.
    Image,
    /// Document input support.
    Document,
    /// Audio input support.
    Audio,
    /// Video input support.
    Video,
}

/// Service endpoint configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceEndpoint {
    /// Base URL for the embedding service.
    pub base_url: String,

    /// API version to use.
    pub api_version: Option<String>,

    /// Additional headers to include in requests.
    pub headers: HashMap<String, String>,

    /// Whether to use TLS for connections.
    pub use_tls: bool,
}

/// Authentication configuration for embedding services.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Authentication method.
    pub method: AuthMethod,

    /// API key for key-based authentication.
    pub api_key: Option<String>,

    /// Bearer token for token-based authentication.
    pub bearer_token: Option<String>,

    /// Additional authentication parameters.
    pub additional_params: HashMap<String, String>,
}

/// Authentication methods for embedding services.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    /// No authentication required.
    None,
    /// API key in header.
    ApiKey,
    /// Bearer token in Authorization header.
    BearerToken,
    /// Custom authentication method.
    Custom,
}

/// Retry configuration for embedding operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_retries: u32,

    /// Base delay between retry attempts.
    pub base_delay: Duration,

    /// Maximum delay between retry attempts.
    pub max_delay: Duration,

    /// Backoff multiplier for exponential backoff.
    pub backoff_multiplier: f64,

    /// Whether to add jitter to retry delays.
    pub jitter: bool,
}

/// Timeout configuration for embedding operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeoutConfig {
    /// Overall request timeout.
    pub request_timeout: Duration,

    /// Connection timeout.
    pub connect_timeout: Duration,

    /// Read timeout for response data.
    pub read_timeout: Duration,
}

/// Rate limiting configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per minute.
    pub requests_per_minute: u32,

    /// Maximum tokens per minute.
    pub tokens_per_minute: Option<u32>,

    /// Maximum concurrent requests.
    pub max_concurrent_requests: u32,
}

impl Default for EmbeddingContext {
    fn default() -> Self {
        Self {
            context_id: Uuid::new_v4(),
            provider: "unknown".to_string(),
            model: "default".to_string(),
            available_models: Vec::new(),
            endpoint: ServiceEndpoint::default(),
            auth: AuthConfig::default(),
            retry_config: RetryConfig::default(),
            timeout_config: TimeoutConfig::default(),
            rate_limit_config: None,
            default_encoding_format: EncodingFormat::Float,
            max_batch_size: 100,
            max_input_tokens: None,
            provider_config: HashMap::new(),
            metadata: HashMap::new(),
        }
    }
}

impl Default for ServiceEndpoint {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8080".to_string(),
            api_version: None,
            headers: HashMap::new(),
            use_tls: true,
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            method: AuthMethod::None,
            api_key: None,
            bearer_token: None,
            additional_params: HashMap::new(),
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(30),
        }
    }
}

impl EmbeddingContext {
    /// Creates a new context builder.
    pub fn builder() -> EmbeddingContextBuilder {
        EmbeddingContextBuilder::new()
    }

    /// Returns the model information for the default model.
    pub fn default_model_info(&self) -> Option<&ModelInfo> {
        self.available_models
            .iter()
            .find(|model| model.id == self.model)
    }

    /// Returns the model information for the specified model.
    pub fn get_model_info(&self, model_id: &str) -> Option<&ModelInfo> {
        self.available_models
            .iter()
            .find(|model| model.id == model_id)
    }

    /// Checks if a model is available in this context.
    pub fn has_model(&self, model_id: &str) -> bool {
        self.available_models
            .iter()
            .any(|model| model.id == model_id)
    }

    /// Returns all models that support the specified input type.
    pub fn models_supporting_input_type(&self, input_type: InputType) -> Vec<&ModelInfo> {
        self.available_models
            .iter()
            .filter(|model| model.supported_input_types.contains(&input_type))
            .collect()
    }

    /// Validates the context configuration.
    pub fn validate(&self) -> Result<(), String> {
        if self.provider.is_empty() {
            return Err("Provider must be specified".to_string());
        }

        if self.model.is_empty() {
            return Err("Default model must be specified".to_string());
        }

        if self.endpoint.base_url.is_empty() {
            return Err("Service endpoint base URL must be specified".to_string());
        }

        if self.max_batch_size == 0 {
            return Err("Max batch size must be greater than 0".to_string());
        }

        if self.retry_config.max_retries > 10 {
            return Err("Max retries should not exceed 10".to_string());
        }

        if self.retry_config.backoff_multiplier <= 1.0 {
            return Err("Backoff multiplier must be greater than 1.0".to_string());
        }

        // Validate that default model exists in available models
        if !self.available_models.is_empty() && !self.has_model(&self.model) {
            return Err("Default model not found in available models".to_string());
        }

        Ok(())
    }
}

impl ModelInfo {
    /// Creates a new model info entry.
    pub fn new(
        id: String,
        name: String,
        dimensions: u32,
        max_input_tokens: u32,
        supported_input_types: Vec<InputType>,
    ) -> Self {
        Self {
            id,
            name,
            description: None,
            dimensions,
            max_input_tokens,
            supported_input_types,
            supports_custom_dimensions: false,
            metadata: HashMap::new(),
        }
    }

    /// Checks if this model supports the specified input type.
    pub fn supports_input_type(&self, input_type: InputType) -> bool {
        self.supported_input_types.contains(&input_type)
    }
}

/// Builder for creating embedding contexts.
#[derive(Debug, Clone)]
pub struct EmbeddingContextBuilder {
    context: EmbeddingContext,
}

impl EmbeddingContextBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self {
            context: EmbeddingContext::default(),
        }
    }

    /// Sets the provider name.
    pub fn provider(mut self, provider: impl Into<String>) -> Self {
        self.context.provider = provider.into();
        self
    }

    /// Sets the default model.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.context.model = model.into();
        self
    }

    /// Adds an available model.
    pub fn add_model(mut self, model: ModelInfo) -> Self {
        self.context.available_models.push(model);
        self
    }

    /// Sets the service endpoint.
    pub fn endpoint(mut self, endpoint: ServiceEndpoint) -> Self {
        self.context.endpoint = endpoint;
        self
    }

    /// Sets the base URL.
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.context.endpoint.base_url = base_url.into();
        self
    }

    /// Sets the authentication configuration.
    pub fn auth(mut self, auth: AuthConfig) -> Self {
        self.context.auth = auth;
        self
    }

    /// Sets an API key for authentication.
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.context.auth.method = AuthMethod::ApiKey;
        self.context.auth.api_key = Some(api_key.into());
        self
    }

    /// Sets retry configuration.
    pub fn retry_config(mut self, retry_config: RetryConfig) -> Self {
        self.context.retry_config = retry_config;
        self
    }

    /// Sets maximum retry attempts.
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.context.retry_config.max_retries = max_retries;
        self
    }

    /// Sets timeout configuration.
    pub fn timeout_config(mut self, timeout_config: TimeoutConfig) -> Self {
        self.context.timeout_config = timeout_config;
        self
    }

    /// Sets request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.context.timeout_config.request_timeout = timeout;
        self
    }

    /// Sets rate limit configuration.
    pub fn rate_limit_config(mut self, rate_limit_config: RateLimitConfig) -> Self {
        self.context.rate_limit_config = Some(rate_limit_config);
        self
    }

    /// Sets the maximum batch size.
    pub fn max_batch_size(mut self, max_batch_size: usize) -> Self {
        self.context.max_batch_size = max_batch_size;
        self
    }

    /// Adds provider-specific configuration.
    pub fn provider_config(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.context.provider_config.insert(key.into(), value);
        self
    }

    /// Adds metadata.
    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.context.metadata.insert(key.into(), value);
        self
    }

    /// Builds the embedding context.
    pub fn build(self) -> Result<EmbeddingContext, String> {
        self.context.validate()?;
        Ok(self.context)
    }
}

impl Default for EmbeddingContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}
