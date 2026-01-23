//! Backend provider implementations.
//!
//! Storage and vector database providers are re-exported from `nvisy_dal`.
//! AI providers are defined locally in this module.
//!
//! ## Storage backends (from nvisy_dal)
//! - `s3` - Amazon S3
//! - `gcs` - Google Cloud Storage
//! - `azblob` - Azure Blob Storage
//! - `postgres` - PostgreSQL
//! - `mysql` - MySQL
//!
//! ## Vector databases (from nvisy_dal)
//! - `qdrant` - Qdrant vector database
//! - `pinecone` - Pinecone vector database
//! - `milvus` - Milvus vector database
//! - `pgvector` - pgvector (PostgreSQL extension)
//!
//! ## AI providers (local)
//! - `openai` - OpenAI (completion + embedding)
//! - `anthropic` - Anthropic (completion only)
//! - `cohere` - Cohere (completion + embedding)
//! - `gemini` - Google Gemini (completion + embedding)
//! - `perplexity` - Perplexity (completion only)

use crate::error::Result;

// AI providers (local implementations)
mod anthropic;
mod cohere;
mod gemini;
mod openai;
mod perplexity;

// Re-export storage backend types from nvisy_dal
// AI provider exports
pub use anthropic::{AnthropicCompletionParams, AnthropicCredentials};
pub use cohere::{CohereCompletionParams, CohereCredentials, CohereEmbeddingParams};
pub use gemini::{GeminiCompletionParams, GeminiCredentials, GeminiEmbeddingParams};
pub use nvisy_dal::provider::{
    // Object storage
    AzblobCredentials,
    AzblobParams,
    AzblobProvider,
    GcsCredentials,
    GcsParams,
    GcsProvider,
    // Vector databases
    MilvusCredentials,
    MilvusParams,
    MilvusProvider,
    // Relational databases
    MysqlCredentials,
    MysqlParams,
    MysqlProvider,
    PgVectorCredentials,
    PgVectorParams,
    PgVectorProvider,
    PineconeCredentials,
    PineconeParams,
    PineconeProvider,
    PostgresCredentials,
    PostgresParams,
    PostgresProvider,
    QdrantCredentials,
    QdrantParams,
    QdrantProvider,
    S3Credentials,
    S3Params,
    S3Provider,
};
pub use openai::{OpenAiCompletionParams, OpenAiCredentials, OpenAiEmbeddingParams};
pub use perplexity::{PerplexityCompletionParams, PerplexityCredentials};

/// Trait for AI provider parameters that can be combined with credentials to create a provider.
///
/// This is distinct from `nvisy_dal::IntoProvider` which is for storage/vector providers.
#[async_trait::async_trait]
pub trait IntoAiProvider {
    /// The credentials type required by this provider.
    type Credentials: Send;
    /// The output type (provider instance).
    type Output;

    /// Combines params with credentials to create the provider.
    async fn into_provider(self, credentials: Self::Credentials) -> Result<Self::Output>;
}
