//! Multi-provider management for AI inference.

mod completion;
mod embedding;
pub mod splitting;

pub use completion::{
    AnthropicModel, CohereCompletionModel, CompletionCredentials, CompletionModel,
    GeminiCompletionModel, OpenAiCompletionModel, PerplexityModel,
};
#[cfg(feature = "ollama")]
pub use embedding::OllamaEmbeddingModel;
pub use embedding::{
    CohereEmbeddingModel, EmbeddingCredentials, EmbeddingModel, EmbeddingProvider,
    GeminiEmbeddingModel, OpenAiEmbeddingModel,
};
pub use splitting::{Chunk, ChunkMetadata, OwnedChunk, TextSplitter, estimate_tokens};
