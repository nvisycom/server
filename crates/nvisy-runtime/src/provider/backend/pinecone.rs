//! Pinecone vector database provider.

use nvisy_dal::provider::PineconeConfig;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::IntoProvider;
use crate::error::WorkflowResult;

/// Pinecone credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PineconeCredentials {
    /// Pinecone API key.
    pub api_key: String,
    /// Environment (e.g., "us-east-1-aws").
    pub environment: String,
}

/// Pinecone parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PineconeParams {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Index name.
    pub index: String,
    /// Namespace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// Vector dimensions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,
}

impl IntoProvider for PineconeParams {
    type Credentials = PineconeCredentials;
    type Output = PineconeConfig;

    fn into_provider(self, credentials: Self::Credentials) -> WorkflowResult<Self::Output> {
        let mut config =
            PineconeConfig::new(credentials.api_key, credentials.environment, self.index);

        if let Some(namespace) = self.namespace {
            config = config.with_namespace(namespace);
        }
        if let Some(dimensions) = self.dimensions {
            config = config.with_dimensions(dimensions);
        }

        Ok(config)
    }
}
