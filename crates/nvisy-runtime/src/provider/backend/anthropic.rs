//! Anthropic provider.

use nvisy_core::IntoProvider;
use nvisy_rig::provider::{AnthropicModel, CompletionProvider};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::IntoAiProvider;
use crate::error::{Error, Result};

/// Anthropic credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicCredentials {
    /// API key.
    pub api_key: String,
}

/// Anthropic completion parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnthropicCompletionParams {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Model to use.
    pub model: AnthropicModel,
}

impl AnthropicCompletionParams {
    /// Creates a new Anthropic completion params.
    pub fn new(credentials_id: Uuid, model: AnthropicModel) -> Self {
        Self {
            credentials_id,
            model,
        }
    }
}

#[async_trait::async_trait]
impl IntoAiProvider for AnthropicCompletionParams {
    type Credentials = AnthropicCredentials;
    type Output = CompletionProvider;

    async fn into_provider(self, credentials: Self::Credentials) -> Result<Self::Output> {
        let rig_creds = nvisy_rig::provider::CompletionCredentials::Anthropic {
            api_key: credentials.api_key,
        };
        let model = nvisy_rig::provider::CompletionModel::Anthropic(self.model);
        CompletionProvider::create(model, rig_creds)
            .await
            .map_err(|e| Error::Internal(e.to_string()))
    }
}
