//! rig-core trait implementations for EmbeddingProvider.

use rig::embeddings::{Embedding, EmbeddingError, EmbeddingModel as RigEmbeddingModel};
#[cfg(feature = "ollama")]
use rig::providers::ollama;

use super::provider::{DEFAULT_MAX_DOCUMENTS, EmbeddingProvider, EmbeddingService};

impl RigEmbeddingModel for EmbeddingProvider {
    type Client = ();

    const MAX_DOCUMENTS: usize = DEFAULT_MAX_DOCUMENTS;

    fn make(_client: &Self::Client, _model: impl Into<String>, _dims: Option<usize>) -> Self {
        // This is a no-op since EmbeddingProvider is constructed via its own methods
        panic!("EmbeddingProvider should be constructed via EmbeddingProvider::new()")
    }

    fn ndims(&self) -> usize {
        match self.inner() {
            EmbeddingService::OpenAi { model, .. } => model.ndims(),
            EmbeddingService::Cohere { model, .. } => model.ndims(),
            EmbeddingService::Gemini { model, .. } => model.ndims(),
            #[cfg(feature = "ollama")]
            EmbeddingService::Ollama { ndims, .. } => *ndims,
        }
    }

    async fn embed_texts(
        &self,
        texts: impl IntoIterator<Item = String> + Send,
    ) -> std::result::Result<Vec<Embedding>, EmbeddingError> {
        match self.inner() {
            EmbeddingService::OpenAi { model, .. } => model.embed_texts(texts).await,
            EmbeddingService::Cohere { model, .. } => model.embed_texts(texts).await,
            EmbeddingService::Gemini { model, .. } => model.embed_texts(texts).await,
            #[cfg(feature = "ollama")]
            EmbeddingService::Ollama {
                client,
                model_name,
                ndims,
            } => {
                let model = ollama::EmbeddingModel::new(client.clone(), model_name, *ndims);
                model.embed_texts(texts).await
            }
        }
    }
}
