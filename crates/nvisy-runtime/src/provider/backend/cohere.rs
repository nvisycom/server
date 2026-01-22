//! Cohere provider.

use nvisy_rig::provider::{
    CohereCompletionModel, CohereEmbeddingModel, CompletionProvider, EmbeddingProvider,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::IntoProvider;
use crate::error::{Error, Result};

/// Cohere credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CohereCredentials {
    /// API key.
    pub api_key: String,
}

/// Cohere completion parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CohereCompletionParams {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Model to use.
    pub model: CohereCompletionModel,
}

impl CohereCompletionParams {
    /// Creates a new Cohere completion params.
    pub fn new(credentials_id: Uuid, model: CohereCompletionModel) -> Self {
        Self {
            credentials_id,
            model,
        }
    }
}

#[async_trait::async_trait]
impl IntoProvider for CohereCompletionParams {
    type Credentials = CohereCredentials;
    type Output = CompletionProvider;

    async fn into_provider(self, credentials: Self::Credentials) -> Result<Self::Output> {
        let rig_creds = nvisy_rig::provider::CompletionCredentials::Cohere {
            api_key: credentials.api_key,
        };
        let model = nvisy_rig::provider::CompletionModel::Cohere(self.model);
        CompletionProvider::new(&rig_creds, &model).map_err(|e| Error::Internal(e.to_string()))
    }
}

/// Cohere embedding parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CohereEmbeddingParams {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Model to use.
    pub model: CohereEmbeddingModel,
}

impl CohereEmbeddingParams {
    /// Creates a new Cohere embedding params.
    pub fn new(credentials_id: Uuid, model: CohereEmbeddingModel) -> Self {
        Self {
            credentials_id,
            model,
        }
    }
}

#[async_trait::async_trait]
impl IntoProvider for CohereEmbeddingParams {
    type Credentials = CohereCredentials;
    type Output = EmbeddingProvider;

    async fn into_provider(self, credentials: Self::Credentials) -> Result<Self::Output> {
        let rig_creds = nvisy_rig::provider::EmbeddingCredentials::Cohere {
            api_key: credentials.api_key,
        };
        let model = nvisy_rig::provider::EmbeddingModel::Cohere(self.model);
        EmbeddingProvider::new(&rig_creds, &model).map_err(|e| Error::Internal(e.to_string()))
    }
}
