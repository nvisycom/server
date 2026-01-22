//! Perplexity provider.

use nvisy_rig::provider::{CompletionProvider, PerplexityModel};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::IntoProvider;
use crate::error::{Error, Result};

/// Perplexity credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerplexityCredentials {
    /// API key.
    pub api_key: String,
}

/// Perplexity completion parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerplexityCompletionParams {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Model to use.
    pub model: PerplexityModel,
}

impl PerplexityCompletionParams {
    /// Creates a new Perplexity completion params.
    pub fn new(credentials_id: Uuid, model: PerplexityModel) -> Self {
        Self {
            credentials_id,
            model,
        }
    }
}

#[async_trait::async_trait]
impl IntoProvider for PerplexityCompletionParams {
    type Credentials = PerplexityCredentials;
    type Output = CompletionProvider;

    async fn into_provider(self, credentials: Self::Credentials) -> Result<Self::Output> {
        let rig_creds = nvisy_rig::provider::CompletionCredentials::Perplexity {
            api_key: credentials.api_key,
        };
        let model = nvisy_rig::provider::CompletionModel::Perplexity(self.model);
        CompletionProvider::new(&rig_creds, &model).map_err(|e| Error::Internal(e.to_string()))
    }
}
