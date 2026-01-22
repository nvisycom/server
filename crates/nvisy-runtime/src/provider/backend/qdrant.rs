//! Qdrant vector database provider.

use nvisy_dal::provider::QdrantConfig;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::IntoProvider;
use crate::error::WorkflowResult;

/// Qdrant credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantCredentials {
    /// Qdrant server URL.
    pub url: String,
    /// API key for authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// Qdrant parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QdrantParams {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Collection name.
    pub collection: String,
    /// Vector dimensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,
}

impl IntoProvider for QdrantParams {
    type Credentials = QdrantCredentials;
    type Output = QdrantConfig;

    fn into_provider(self, credentials: Self::Credentials) -> WorkflowResult<Self::Output> {
        let mut config = QdrantConfig::new(credentials.url).with_collection(self.collection);

        if let Some(api_key) = credentials.api_key {
            config = config.with_api_key(api_key);
        }
        if let Some(dimensions) = self.dimensions {
            config = config.with_dimensions(dimensions);
        }

        Ok(config)
    }
}
