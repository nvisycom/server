//! OpenAI provider.

use nvisy_core::IntoProvider;
use nvisy_rig::provider::{
    CompletionProvider, EmbeddingProvider, OpenAiCompletionModel, OpenAiEmbeddingModel,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::IntoAiProvider;
use crate::error::{Error, Result};

/// OpenAI credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiCredentials {
    /// API key.
    pub api_key: String,
}

/// OpenAI completion parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenAiCompletionParams {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Model to use.
    pub model: OpenAiCompletionModel,
}

impl OpenAiCompletionParams {
    /// Creates a new OpenAI completion params.
    pub fn new(credentials_id: Uuid, model: OpenAiCompletionModel) -> Self {
        Self {
            credentials_id,
            model,
        }
    }
}

#[async_trait::async_trait]
impl IntoAiProvider for OpenAiCompletionParams {
    type Credentials = OpenAiCredentials;
    type Output = CompletionProvider;

    async fn into_provider(self, credentials: Self::Credentials) -> Result<Self::Output> {
        let rig_creds = nvisy_rig::provider::CompletionCredentials::OpenAi {
            api_key: credentials.api_key,
        };
        let model = nvisy_rig::provider::CompletionModel::OpenAi(self.model);
        CompletionProvider::create(model, rig_creds)
            .await
            .map_err(|e| Error::Internal(e.to_string()))
    }
}

/// OpenAI embedding parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenAiEmbeddingParams {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Model to use.
    pub model: OpenAiEmbeddingModel,
}

impl OpenAiEmbeddingParams {
    /// Creates a new OpenAI embedding params.
    pub fn new(credentials_id: Uuid, model: OpenAiEmbeddingModel) -> Self {
        Self {
            credentials_id,
            model,
        }
    }
}

#[async_trait::async_trait]
impl IntoAiProvider for OpenAiEmbeddingParams {
    type Credentials = OpenAiCredentials;
    type Output = EmbeddingProvider;

    async fn into_provider(self, credentials: Self::Credentials) -> Result<Self::Output> {
        let rig_creds = nvisy_rig::provider::EmbeddingCredentials::OpenAi {
            api_key: credentials.api_key,
        };
        let model = nvisy_rig::provider::EmbeddingModel::OpenAi(self.model);
        EmbeddingProvider::create(model, rig_creds)
            .await
            .map_err(|e| Error::Internal(e.to_string()))
    }
}
