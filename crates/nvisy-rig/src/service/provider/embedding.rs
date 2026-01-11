//! Embedding provider abstraction.
//!
//! Wraps different embedding model providers into a unified enum,
//! eliminating the need for generic parameters throughout the codebase.

use rig::embeddings::{Embedding, EmbeddingError, EmbeddingModel};
use rig::providers::ollama;

/// Embedding provider that wraps different model implementations.
///
/// This enum provides a concrete type for embedding operations,
/// removing the need for generic `M: EmbeddingModel` parameters.
///
/// Implements [`EmbeddingModel`] so it can be used directly with rig's
/// APIs like `VectorStoreIndex` and `EmbeddingsBuilder`.
#[derive(Clone)]
pub enum EmbeddingProvider {
    /// Ollama embedding model.
    Ollama(ollama::EmbeddingModel),
}

impl EmbeddingProvider {
    /// Creates a new Ollama embedding provider.
    pub fn ollama(base_url: &str, model: &str) -> Self {
        let client = ollama::Client::from_url(base_url);
        Self::Ollama(client.embedding_model(model))
    }

    /// Creates a new Ollama embedding provider with custom dimensions.
    pub fn ollama_with_ndims(base_url: &str, model: &str, ndims: usize) -> Self {
        let client = ollama::Client::from_url(base_url);
        Self::Ollama(client.embedding_model_with_ndims(model, ndims))
    }
}

impl EmbeddingModel for EmbeddingProvider {
    const MAX_DOCUMENTS: usize = 1024;

    fn ndims(&self) -> usize {
        match self {
            Self::Ollama(model) => model.ndims(),
        }
    }

    async fn embed_texts(
        &self,
        texts: impl IntoIterator<Item = String> + Send,
    ) -> Result<Vec<Embedding>, EmbeddingError> {
        match self {
            Self::Ollama(model) => model.embed_texts(texts).await,
        }
    }
}

impl std::fmt::Debug for EmbeddingProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ollama(model) => f
                .debug_struct("EmbeddingProvider::Ollama")
                .field("model", &model.model)
                .finish(),
        }
    }
}
