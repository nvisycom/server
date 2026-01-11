//! Provider configuration types.

use serde::{Deserialize, Serialize};

/// Supported AI providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    /// OpenAI (GPT-4, embeddings, etc.)
    OpenAi,
    /// Anthropic (Claude models)
    Anthropic,
    /// Cohere (Command, embeddings)
    Cohere,
    /// Google Gemini
    Gemini,
    /// Perplexity
    Perplexity,
}

impl ProviderKind {
    /// Returns the provider name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::Cohere => "cohere",
            Self::Gemini => "gemini",
            Self::Perplexity => "perplexity",
        }
    }

    /// Default completion model for this provider.
    pub fn default_completion_model(&self) -> &'static str {
        match self {
            Self::OpenAi => "gpt-4o",
            Self::Anthropic => "claude-sonnet-4-20250514",
            Self::Cohere => "command-r-plus",
            Self::Gemini => "gemini-2.0-flash",
            Self::Perplexity => "sonar",
        }
    }

    /// Default embedding model for this provider.
    pub fn default_embedding_model(&self) -> &'static str {
        match self {
            Self::OpenAi => "text-embedding-3-small",
            Self::Anthropic => "text-embedding-3-small", // Uses OpenAI
            Self::Cohere => "embed-english-v3.0",
            Self::Gemini => "text-embedding-004",
            Self::Perplexity => "text-embedding-3-small", // Uses OpenAI
        }
    }
}

impl std::fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Configuration for a single provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Unique identifier for this provider instance.
    pub id: String,

    /// The provider type.
    pub kind: ProviderKind,

    /// API key for authentication.
    pub api_key: String,

    /// Optional base URL override.
    #[serde(default)]
    pub base_url: Option<String>,

    /// Model configuration.
    #[serde(default)]
    pub models: ModelConfig,
}

impl ProviderConfig {
    /// Creates a new provider configuration.
    pub fn new(id: impl Into<String>, kind: ProviderKind, api_key: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            api_key: api_key.into(),
            base_url: None,
            models: ModelConfig::default_for(kind),
        }
    }

    /// Sets the base URL.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Sets the model configuration.
    pub fn with_models(mut self, models: ModelConfig) -> Self {
        self.models = models;
        self
    }
}

/// Model configuration for a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Model for completions/chat.
    pub completion: String,

    /// Model for embeddings.
    pub embedding: String,

    /// Model for vision tasks.
    #[serde(default)]
    pub vision: Option<String>,

    /// Maximum tokens for completions.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,

    /// Temperature for completions (0.0 - 2.0).
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_max_tokens() -> usize {
    4096
}

fn default_temperature() -> f32 {
    0.7
}

impl ModelConfig {
    /// Creates default model config for a provider.
    pub fn default_for(kind: ProviderKind) -> Self {
        Self {
            completion: kind.default_completion_model().to_string(),
            embedding: kind.default_embedding_model().to_string(),
            vision: None,
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
        }
    }
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self::default_for(ProviderKind::OpenAi)
    }
}
