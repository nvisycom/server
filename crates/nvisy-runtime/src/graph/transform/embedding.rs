//! Embedding transformer configuration.

use nvisy_rig::provider::EmbeddingModel;
use serde::{Deserialize, Serialize};

/// Configuration for generating embeddings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Model to use for embedding generation.
    #[serde(flatten)]
    pub model: EmbeddingModel,

    /// Whether to L2-normalize the output embeddings.
    #[serde(default)]
    pub normalize: bool,
}
