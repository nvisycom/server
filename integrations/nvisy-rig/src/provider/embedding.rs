//! Embedding provider abstraction.
//!
//! Wraps different embedding model providers into a unified enum,
//! eliminating the need for generic parameters throughout the codebase.

use nvisy_postgres::types::EMBEDDING_DIMENSIONS;
use rig::client::Nothing;
use rig::embeddings::{Embedding, EmbeddingError, EmbeddingModel};
use rig::providers::ollama;

/// Embedding provider that wraps different model implementations.
///
/// This enum provides a concrete type for embedding operations,
/// removing the need for generic `M: EmbeddingModel` parameters.
///
/// All providers use [`EMBEDDING_DIMENSIONS`] to ensure consistency with the
/// `document_chunks` table schema.
#[derive(Clone)]
pub enum EmbeddingProvider {
    /// Ollama embedding model.
    Ollama {
        client: ollama::Client,
        model: String,
    },
}

impl EmbeddingProvider {
    /// Creates a new Ollama embedding provider.
    pub fn ollama(base_url: &str, model: &str) -> Self {
        let client = ollama::Client::builder()
            .api_key(Nothing)
            .base_url(base_url)
            .build()
            .expect("Failed to create Ollama client");

        Self::Ollama {
            client,
            model: model.to_string(),
        }
    }

    /// Returns the model name.
    pub fn model_name(&self) -> &str {
        match self {
            Self::Ollama { model, .. } => model,
        }
    }

    /// Returns the number of dimensions.
    ///
    /// This always returns [`EMBEDDING_DIMENSIONS`] to ensure consistency with the database schema.
    pub fn ndims(&self) -> usize {
        EMBEDDING_DIMENSIONS
    }

    /// Embed a single text document.
    pub async fn embed_text(&self, text: &str) -> Result<Embedding, EmbeddingError> {
        match self {
            Self::Ollama { client, model } => {
                let embedding_model =
                    ollama::EmbeddingModel::new(client.clone(), model, EMBEDDING_DIMENSIONS);
                embedding_model.embed_text(text).await
            }
        }
    }

    /// Embed multiple text documents.
    pub async fn embed_texts(
        &self,
        texts: impl IntoIterator<Item = String> + Send,
    ) -> Result<Vec<Embedding>, EmbeddingError> {
        match self {
            Self::Ollama { client, model } => {
                let embedding_model =
                    ollama::EmbeddingModel::new(client.clone(), model, EMBEDDING_DIMENSIONS);
                embedding_model.embed_texts(texts).await
            }
        }
    }
}

impl std::fmt::Debug for EmbeddingProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ollama { model, .. } => f
                .debug_struct("EmbeddingProvider::Ollama")
                .field("model", model)
                .field("ndims", &EMBEDDING_DIMENSIONS)
                .finish(),
        }
    }
}
