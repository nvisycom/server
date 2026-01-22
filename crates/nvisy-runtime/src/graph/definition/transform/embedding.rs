//! Embedding transform definition.

use serde::{Deserialize, Serialize};

use crate::provider::EmbeddingProviderParams;

/// Embedding transform for generating vector embeddings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Embedding {
    /// Embedding provider parameters (includes credentials_id and model).
    #[serde(flatten)]
    pub provider: EmbeddingProviderParams,

    /// Whether to L2-normalize the output embeddings.
    #[serde(default)]
    pub normalize: bool,
}
