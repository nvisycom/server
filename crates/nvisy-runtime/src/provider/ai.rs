//! AI provider types and implementations.

use derive_more::From;
use nvisy_rig::provider::{CompletionProvider, EmbeddingProvider};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ProviderCredentials;
use super::backend::{
    AnthropicCompletionParams, AnthropicCredentials, CohereCompletionParams, CohereCredentials,
    CohereEmbeddingParams, GeminiCompletionParams, GeminiCredentials, GeminiEmbeddingParams,
    IntoProvider, OpenAiCompletionParams, OpenAiCredentials, OpenAiEmbeddingParams,
    PerplexityCompletionParams, PerplexityCredentials,
};
use crate::error::{Error, Result};

/// Completion provider parameters.
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub enum CompletionProviderParams {
    /// OpenAI completion.
    OpenAi(OpenAiCompletionParams),
    /// Anthropic completion.
    Anthropic(AnthropicCompletionParams),
    /// Cohere completion.
    Cohere(CohereCompletionParams),
    /// Google Gemini completion.
    Gemini(GeminiCompletionParams),
    /// Perplexity completion.
    Perplexity(PerplexityCompletionParams),
}

impl CompletionProviderParams {
    /// Returns the credentials ID for this provider.
    pub fn credentials_id(&self) -> Uuid {
        match self {
            Self::OpenAi(p) => p.credentials_id,
            Self::Anthropic(p) => p.credentials_id,
            Self::Cohere(p) => p.credentials_id,
            Self::Gemini(p) => p.credentials_id,
            Self::Perplexity(p) => p.credentials_id,
        }
    }

    /// Returns the provider kind as a string.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::OpenAi(_) => "openai",
            Self::Anthropic(_) => "anthropic",
            Self::Cohere(_) => "cohere",
            Self::Gemini(_) => "gemini",
            Self::Perplexity(_) => "perplexity",
        }
    }
}

#[async_trait::async_trait]
impl IntoProvider for CompletionProviderParams {
    type Credentials = ProviderCredentials;
    type Output = CompletionProvider;

    async fn into_provider(self, credentials: Self::Credentials) -> Result<Self::Output> {
        match (self, credentials) {
            (Self::OpenAi(p), ProviderCredentials::OpenAi(c)) => p.into_provider(c).await,
            (Self::Anthropic(p), ProviderCredentials::Anthropic(c)) => p.into_provider(c).await,
            (Self::Cohere(p), ProviderCredentials::Cohere(c)) => p.into_provider(c).await,
            (Self::Gemini(p), ProviderCredentials::Gemini(c)) => p.into_provider(c).await,
            (Self::Perplexity(p), ProviderCredentials::Perplexity(c)) => p.into_provider(c).await,
            (params, creds) => Err(Error::Internal(format!(
                "credentials type mismatch: expected '{}', got '{}'",
                params.kind(),
                creds.kind()
            ))),
        }
    }
}

/// Embedding provider parameters.
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub enum EmbeddingProviderParams {
    /// OpenAI embedding.
    OpenAi(OpenAiEmbeddingParams),
    /// Cohere embedding.
    Cohere(CohereEmbeddingParams),
    /// Google Gemini embedding.
    Gemini(GeminiEmbeddingParams),
}

impl EmbeddingProviderParams {
    /// Returns the credentials ID for this provider.
    pub fn credentials_id(&self) -> Uuid {
        match self {
            Self::OpenAi(p) => p.credentials_id,
            Self::Cohere(p) => p.credentials_id,
            Self::Gemini(p) => p.credentials_id,
        }
    }

    /// Returns the provider kind as a string.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::OpenAi(_) => "openai",
            Self::Cohere(_) => "cohere",
            Self::Gemini(_) => "gemini",
        }
    }

    /// Returns the embedding dimensions for this provider's model.
    pub fn dimensions(&self) -> usize {
        match self {
            Self::OpenAi(p) => p.model.dimensions(),
            Self::Cohere(p) => p.model.dimensions(),
            Self::Gemini(p) => p.model.dimensions(),
        }
    }
}

#[async_trait::async_trait]
impl IntoProvider for EmbeddingProviderParams {
    type Credentials = ProviderCredentials;
    type Output = EmbeddingProvider;

    async fn into_provider(self, credentials: Self::Credentials) -> Result<Self::Output> {
        match (self, credentials) {
            (Self::OpenAi(p), ProviderCredentials::OpenAi(c)) => p.into_provider(c).await,
            (Self::Cohere(p), ProviderCredentials::Cohere(c)) => p.into_provider(c).await,
            (Self::Gemini(p), ProviderCredentials::Gemini(c)) => p.into_provider(c).await,
            (params, creds) => Err(Error::Internal(format!(
                "credentials type mismatch: expected '{}', got '{}'",
                params.kind(),
                creds.kind()
            ))),
        }
    }
}

/// AI provider credentials (sensitive).
#[derive(Debug, Clone, From, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub enum AiCredentials {
    /// OpenAI credentials.
    OpenAi(OpenAiCredentials),
    /// Anthropic credentials.
    Anthropic(AnthropicCredentials),
    /// Cohere credentials.
    Cohere(CohereCredentials),
    /// Gemini credentials.
    Gemini(GeminiCredentials),
    /// Perplexity credentials.
    Perplexity(PerplexityCredentials),
}

impl AiCredentials {
    /// Returns the provider kind as a string.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::OpenAi(_) => "openai",
            Self::Anthropic(_) => "anthropic",
            Self::Cohere(_) => "cohere",
            Self::Gemini(_) => "gemini",
            Self::Perplexity(_) => "perplexity",
        }
    }
}
