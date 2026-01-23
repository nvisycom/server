//! AI provider types and implementations.
//!
//! Re-exports types from nvisy_rig and provides wrapper enums for provider params.

use derive_more::From;
use nvisy_core::Provider;
use nvisy_rig::provider::{
    AnthropicModel, CohereCompletionModel, CohereEmbeddingModel, CompletionCredentials,
    CompletionModel, CompletionProvider, EmbeddingCredentials, EmbeddingModel, EmbeddingProvider,
    GeminiCompletionModel, GeminiEmbeddingModel, OpenAiCompletionModel, OpenAiEmbeddingModel,
    PerplexityModel,
};
use serde::{Deserialize, Serialize};
use strum::IntoStaticStr;
use uuid::Uuid;

use crate::error::{Error, Result};

// =============================================================================
// Completion Provider Params
// =============================================================================

/// Completion provider parameters with credentials reference.
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize, IntoStaticStr)]
#[serde(tag = "provider", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum CompletionProviderParams {
    /// OpenAI completion.
    OpenAi {
        credentials_id: Uuid,
        model: OpenAiCompletionModel,
    },
    /// Anthropic completion.
    Anthropic {
        credentials_id: Uuid,
        model: AnthropicModel,
    },
    /// Cohere completion.
    Cohere {
        credentials_id: Uuid,
        model: CohereCompletionModel,
    },
    /// Google Gemini completion.
    Gemini {
        credentials_id: Uuid,
        model: GeminiCompletionModel,
    },
    /// Perplexity completion.
    Perplexity {
        credentials_id: Uuid,
        model: PerplexityModel,
    },
}

impl CompletionProviderParams {
    /// Returns the credentials ID.
    pub fn credentials_id(&self) -> Uuid {
        match self {
            Self::OpenAi { credentials_id, .. }
            | Self::Anthropic { credentials_id, .. }
            | Self::Cohere { credentials_id, .. }
            | Self::Gemini { credentials_id, .. }
            | Self::Perplexity { credentials_id, .. } => *credentials_id,
        }
    }

    /// Returns the provider kind as a string.
    pub fn kind(&self) -> &'static str {
        self.into()
    }

    /// Creates a completion provider from params and credentials.
    pub async fn into_provider(
        self,
        credentials: CompletionCredentials,
    ) -> Result<CompletionProvider> {
        let model = match self {
            Self::OpenAi { model, .. } => CompletionModel::OpenAi(model),
            Self::Anthropic { model, .. } => CompletionModel::Anthropic(model),
            Self::Cohere { model, .. } => CompletionModel::Cohere(model),
            Self::Gemini { model, .. } => CompletionModel::Gemini(model),
            Self::Perplexity { model, .. } => CompletionModel::Perplexity(model),
        };

        CompletionProvider::connect(model, credentials)
            .await
            .map_err(|e| Error::Internal(e.to_string()))
    }
}

// =============================================================================
// Embedding Provider Params
// =============================================================================

/// Embedding provider parameters with credentials reference.
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize, IntoStaticStr)]
#[serde(tag = "provider", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum EmbeddingProviderParams {
    /// OpenAI embedding.
    OpenAi {
        credentials_id: Uuid,
        model: OpenAiEmbeddingModel,
    },
    /// Cohere embedding.
    Cohere {
        credentials_id: Uuid,
        model: CohereEmbeddingModel,
    },
    /// Google Gemini embedding.
    Gemini {
        credentials_id: Uuid,
        model: GeminiEmbeddingModel,
    },
}

impl EmbeddingProviderParams {
    /// Returns the credentials ID.
    pub fn credentials_id(&self) -> Uuid {
        match self {
            Self::OpenAi { credentials_id, .. }
            | Self::Cohere { credentials_id, .. }
            | Self::Gemini { credentials_id, .. } => *credentials_id,
        }
    }

    /// Returns the provider kind as a string.
    pub fn kind(&self) -> &'static str {
        self.into()
    }

    /// Returns the embedding dimensions for this model.
    pub fn dimensions(&self) -> usize {
        match self {
            Self::OpenAi { model, .. } => model.dimensions(),
            Self::Cohere { model, .. } => model.dimensions(),
            Self::Gemini { model, .. } => model.dimensions(),
        }
    }

    /// Creates an embedding provider from params and credentials.
    pub async fn into_provider(
        self,
        credentials: EmbeddingCredentials,
    ) -> Result<EmbeddingProvider> {
        let model = match self {
            Self::OpenAi { model, .. } => EmbeddingModel::OpenAi(model),
            Self::Cohere { model, .. } => EmbeddingModel::Cohere(model),
            Self::Gemini { model, .. } => EmbeddingModel::Gemini(model),
        };

        EmbeddingProvider::connect(model, credentials)
            .await
            .map_err(|e| Error::Internal(e.to_string()))
    }
}
