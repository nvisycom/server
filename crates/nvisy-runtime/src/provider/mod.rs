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

mod ai;
pub mod backend;
mod inputs;
mod outputs;
mod registry;
pub mod runtime;

pub use ai::{AiCredentials, CompletionProviderParams, EmbeddingProviderParams};
pub use backend::IntoProvider;
use backend::{
    AnthropicCredentials, AzblobCredentials, CohereCredentials, GcsCredentials, GeminiCredentials,
    MilvusCredentials, MysqlCredentials, OpenAiCredentials, PerplexityCredentials,
    PgVectorCredentials, PineconeCredentials, PostgresCredentials, QdrantCredentials,
    S3Credentials,
};
use derive_more::From;
pub use inputs::{InputProvider, InputProviderParams};
pub use outputs::{OutputProvider, OutputProviderParams};
pub use registry::CredentialsRegistry;
use serde::{Deserialize, Serialize};
use strum::IntoStaticStr;

/// Provider credentials (sensitive).
#[derive(Debug, Clone, From, Serialize, Deserialize, IntoStaticStr)]
#[serde(tag = "provider", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
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
    pub fn kind(&self) -> &'static str {
        self.into()
    }
}
