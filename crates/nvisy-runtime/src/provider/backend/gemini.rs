//! Google Gemini provider.

use nvisy_rig::provider::{
    CompletionProvider, EmbeddingProvider, GeminiCompletionModel, GeminiEmbeddingModel,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::IntoProvider;
use crate::error::{Error, Result};

/// Gemini credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiCredentials {
    /// API key.
    pub api_key: String,
}

/// Gemini completion parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeminiCompletionParams {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Model to use.
    pub model: GeminiCompletionModel,
}

impl GeminiCompletionParams {
    /// Creates a new Gemini completion params.
    pub fn new(credentials_id: Uuid, model: GeminiCompletionModel) -> Self {
        Self {
            credentials_id,
            model,
        }
    }
}

#[async_trait::async_trait]
impl IntoProvider for GeminiCompletionParams {
    type Credentials = GeminiCredentials;
    type Output = CompletionProvider;

    async fn into_provider(self, credentials: Self::Credentials) -> Result<Self::Output> {
        let rig_creds = nvisy_rig::provider::CompletionCredentials::Gemini {
            api_key: credentials.api_key,
        };
        let model = nvisy_rig::provider::CompletionModel::Gemini(self.model);
        CompletionProvider::new(&rig_creds, &model).map_err(|e| Error::Internal(e.to_string()))
    }
}

/// Gemini embedding parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeminiEmbeddingParams {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Model to use.
    pub model: GeminiEmbeddingModel,
}

impl GeminiEmbeddingParams {
    /// Creates a new Gemini embedding params.
    pub fn new(credentials_id: Uuid, model: GeminiEmbeddingModel) -> Self {
        Self {
            credentials_id,
            model,
        }
    }
}

#[async_trait::async_trait]
impl IntoProvider for GeminiEmbeddingParams {
    type Credentials = GeminiCredentials;
    type Output = EmbeddingProvider;

    async fn into_provider(self, credentials: Self::Credentials) -> Result<Self::Output> {
        let rig_creds = nvisy_rig::provider::EmbeddingCredentials::Gemini {
            api_key: credentials.api_key,
        };
        let model = nvisy_rig::provider::EmbeddingModel::Gemini(self.model);
        EmbeddingProvider::new(&rig_creds, &model).map_err(|e| Error::Internal(e.to_string()))
    }
}
