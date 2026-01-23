//! Configuration for the rig service.

#[cfg(feature = "config")]
use clap::Args;
use serde::{Deserialize, Serialize};

#[cfg(feature = "ollama")]
use crate::provider::{EmbeddingProvider, OllamaEmbeddingModel};

/// Configuration for AI services (chat and RAG).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "config", derive(Args))]
pub struct RigConfig {
    /// Ollama base URL for embeddings.
    #[cfg(feature = "ollama")]
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
    #[cfg(feature = "ollama")]
    #[cfg_attr(
        feature = "config",
        arg(
            long,
            env = "OLLAMA_EMBEDDING_MODEL",
            default_value = "nomic-embed-text"
        )
    )]
    pub ollama_embedding_model: String,

    /// Ollama embedding model dimensions.
    #[cfg(feature = "ollama")]
    #[cfg_attr(
        feature = "config",
        arg(long, env = "OLLAMA_EMBEDDING_DIMENSIONS", default_value = "768")
    )]
    pub ollama_embedding_dimensions: usize,
}

impl Default for RigConfig {
    fn default() -> Self {
        Self {
            #[cfg(feature = "ollama")]
            ollama_base_url: "http://localhost:11434".to_string(),
            #[cfg(feature = "ollama")]
            ollama_embedding_model: "nomic-embed-text".to_string(),
            #[cfg(feature = "ollama")]
            ollama_embedding_dimensions: 768,
        }
    }
}

#[cfg(feature = "ollama")]
impl RigConfig {
    /// Creates an Ollama embedding provider from this configuration.
    pub(crate) fn embedding_provider(&self) -> nvisy_core::Result<EmbeddingProvider> {
        let model = OllamaEmbeddingModel::new(
            &self.ollama_embedding_model,
            self.ollama_embedding_dimensions,
        );
        EmbeddingProvider::ollama(&self.ollama_base_url, model)
    }
}
