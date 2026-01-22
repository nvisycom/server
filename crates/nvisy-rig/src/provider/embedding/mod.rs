//! Embedding models and providers.

mod credentials;
mod model;
mod provider;
mod rig_impl;

pub use credentials::EmbeddingCredentials;
#[cfg(feature = "ollama")]
pub use model::OllamaEmbeddingModel;
pub use model::{CohereEmbeddingModel, EmbeddingModel, GeminiEmbeddingModel, OpenAiEmbeddingModel};
pub use provider::EmbeddingProvider;
