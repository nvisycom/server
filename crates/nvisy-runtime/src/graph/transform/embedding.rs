//! Embedding transformer configuration.

use serde::{Deserialize, Serialize};

use crate::provider::EmbeddingProviderParams;

/// Configuration for generating embeddings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Embedding provider parameters (includes credentials_id and model).
    #[serde(flatten)]
    pub provider: EmbeddingProviderParams,

    /// Whether to L2-normalize the output embeddings.
    #[serde(default)]
    pub normalize: bool,
}
