//! Embedding transform definition.

use nvisy_core::Provider;
use nvisy_rig::provider::{Credentials, EmbeddingModel, EmbeddingProvider};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Error, Result};

/// Embedding transform for generating vector embeddings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Embedding {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,

    /// Embedding model to use.
    #[serde(flatten)]
    pub model: EmbeddingModel,

    /// Whether to L2-normalize the output embeddings.
    #[serde(default)]
    pub normalize: bool,
}

impl Embedding {
    /// Creates an embedding provider from these parameters and credentials.
    pub async fn into_provider(self, credentials: Credentials) -> Result<EmbeddingProvider> {
        // Validate that credentials support embedding
        credentials
            .require_embedding_support()
            .map_err(|e| Error::Internal(e.to_string()))?;

        EmbeddingProvider::connect(self.model, credentials)
            .await
            .map_err(|e| Error::Internal(e.to_string()))
    }
}
