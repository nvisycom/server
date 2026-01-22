//! Embedding transformer.

use nvisy_dal::AnyDataValue;
use serde::{Deserialize, Serialize};

use super::Transform;
use crate::error::Result;
use crate::provider::{CredentialsRegistry, EmbeddingProviderParams};

/// Embedding transformer for generating vector embeddings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Embedding {
    /// Embedding provider parameters (includes credentials_id and model).
    #[serde(flatten)]
    pub provider: EmbeddingProviderParams,

    /// Whether to L2-normalize the output embeddings.
    #[serde(default)]
    pub normalize: bool,
}

impl Transform for Embedding {
    async fn transform(
        &self,
        input: Vec<AnyDataValue>,
        _registry: &CredentialsRegistry,
    ) -> Result<Vec<AnyDataValue>> {
        // TODO: Implement embedding generation using provider
        // For now, pass through unchanged
        Ok(input)
    }
}
