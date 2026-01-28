//! Embedding models and providers.

mod model;
mod provider;
mod rig_impl;

pub use model::{CohereEmbeddingModel, EmbeddingModel, GeminiEmbeddingModel, OpenAiEmbeddingModel};
pub use provider::EmbeddingProvider;
