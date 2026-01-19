//! Configuration for the rig service.

#[cfg(feature = "config")]
use clap::Args;
use serde::{Deserialize, Serialize};

use crate::provider::EmbeddingProvider;

/// Configuration for AI services (chat and RAG).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(Args))]
pub struct RigConfig {
    /// Ollama base URL for embeddings.
    #[cfg_attr(
        feature = "config",
        arg(
            long,
            env = "OLLAMA_BASE_URL",
            default_value = "http://localhost:11434"
        )
    )]
    pub ollama_base_url: String,

    /// Ollama embedding model name.
    #[cfg_attr(
        feature = "config",
        arg(
            long,
            env = "OLLAMA_EMBEDDING_MODEL",
            default_value = "nomic-embed-text"
        )
    )]
    pub ollama_embedding_model: String,
}

impl Default for RigConfig {
    fn default() -> Self {
        Self {
            ollama_base_url: "http://localhost:11434".to_string(),
            ollama_embedding_model: "nomic-embed-text".to_string(),
        }
    }
}

impl RigConfig {
    /// Creates an embedding provider from this configuration.
    pub(crate) fn embedding_provider(&self) -> EmbeddingProvider {
        EmbeddingProvider::ollama(&self.ollama_base_url, &self.ollama_embedding_model)
    }
}
