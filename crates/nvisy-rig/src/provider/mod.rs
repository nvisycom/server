//! Multi-provider management for AI inference.

mod completion;
mod credentials;
mod embedding;
pub mod splitting;

pub use completion::{
    AnthropicModel, CohereCompletionModel, CompletionModel, CompletionProvider,
    GeminiCompletionModel, OpenAiCompletionModel, PerplexityModel,
};
pub use credentials::{ApiKeyCredentials, Credentials};
pub use embedding::{
    CohereEmbeddingModel, EmbeddingModel, EmbeddingProvider, GeminiEmbeddingModel,
    OpenAiEmbeddingModel,
};
pub use splitting::{Chunk, ChunkMetadata, OwnedChunk, TextSplitter};
