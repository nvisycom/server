//! Provider params, credentials, and registry.
//!
//! This module separates provider configuration into:
//! - [`ProviderCredentials`]: Sensitive credentials (stored per workspace)
//! - [`AiCredentials`]: AI provider credentials (stored per workspace)
//! - [`InputProviderParams`] / [`OutputProviderParams`]: Non-sensitive parameters (part of node definition)
//! - [`CompletionProviderParams`] / [`EmbeddingProviderParams`]: AI provider parameters
//! - [`CredentialsRegistry`]: In-memory registry for credentials lookup
//!
//! # Module Structure
//!
//! - [`backend`]: Individual provider implementations (credentials + params)
//! - `inputs`: Input provider types and read operations
//! - `outputs`: Output provider types and write operations
//! - `ai`: AI provider types (completion + embedding)
//! - `registry`: Credentials registry for workflow execution

mod ai;
pub mod backend;
mod inputs;
mod outputs;
mod registry;
pub mod runtime;

// Storage backend exports
pub use backend::{
    AzblobCredentials, AzblobParams, GcsCredentials, GcsParams, MysqlCredentials, MysqlParams,
    PostgresCredentials, PostgresParams, S3Credentials, S3Params,
};

// Vector database exports
pub use backend::{
    MilvusCredentials, MilvusParams, PgVectorCredentials, PgVectorParams, PineconeCredentials,
    PineconeParams, QdrantCredentials, QdrantParams,
};

// AI provider exports
pub use backend::{
    AnthropicCompletionParams, AnthropicCredentials, CohereCompletionParams, CohereCredentials,
    CohereEmbeddingParams, GeminiCompletionParams, GeminiCredentials, GeminiEmbeddingParams,
    OpenAiCompletionParams, OpenAiCredentials, OpenAiEmbeddingParams, PerplexityCompletionParams,
    PerplexityCredentials,
};

use derive_more::From;
pub use inputs::{InputProvider, InputProviderParams};
pub use outputs::{OutputProvider, OutputProviderParams};
pub use registry::CredentialsRegistry;
use serde::{Deserialize, Serialize};

// AI provider enum exports
pub use ai::{AiCredentials, CompletionProviderParams, EmbeddingProviderParams};
pub use backend::IntoProvider;

/// Provider credentials (sensitive).
#[derive(Debug, Clone, From, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub enum ProviderCredentials {
    // Storage backends
    /// Amazon S3 credentials.
    S3(S3Credentials),
    /// Google Cloud Storage credentials.
    Gcs(GcsCredentials),
    /// Azure Blob Storage credentials.
    Azblob(AzblobCredentials),
    /// PostgreSQL credentials.
    Postgres(PostgresCredentials),
    /// MySQL credentials.
    Mysql(MysqlCredentials),

    // Vector databases
    /// Qdrant credentials.
    Qdrant(QdrantCredentials),
    /// Pinecone credentials.
    Pinecone(PineconeCredentials),
    /// Milvus credentials.
    Milvus(MilvusCredentials),
    /// pgvector credentials.
    PgVector(PgVectorCredentials),

    // AI providers
    /// OpenAI credentials.
    OpenAi(OpenAiCredentials),
    /// Anthropic credentials.
    Anthropic(AnthropicCredentials),
    /// Cohere credentials.
    Cohere(CohereCredentials),
    /// Google Gemini credentials.
    Gemini(GeminiCredentials),
    /// Perplexity credentials.
    Perplexity(PerplexityCredentials),
}

impl ProviderCredentials {
    /// Returns the provider kind as a string.
    pub const fn kind(&self) -> &'static str {
        match self {
            // Storage backends
            Self::S3(_) => "s3",
            Self::Gcs(_) => "gcs",
            Self::Azblob(_) => "azblob",
            Self::Postgres(_) => "postgres",
            Self::Mysql(_) => "mysql",
            // Vector databases
            Self::Qdrant(_) => "qdrant",
            Self::Pinecone(_) => "pinecone",
            Self::Milvus(_) => "milvus",
            Self::PgVector(_) => "pgvector",
            // AI providers
            Self::OpenAi(_) => "openai",
            Self::Anthropic(_) => "anthropic",
            Self::Cohere(_) => "cohere",
            Self::Gemini(_) => "gemini",
            Self::Perplexity(_) => "perplexity",
        }
    }
}
