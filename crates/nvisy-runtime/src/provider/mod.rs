//! Provider params, credentials, and registry.
//!
//! This module separates provider configuration into:
//! - [`ProviderCredentials`]: All credentials (storage + AI, stored per workspace)
//! - [`InputProviderConfig`] / [`OutputProviderConfig`]: Config with credentials reference + params
//! - [`InputProviderParams`] / [`OutputProviderParams`]: Non-sensitive parameters (part of node definition)
//! - [`CompletionProviderParams`] / [`EmbeddingProviderParams`]: AI provider parameters
//! - [`CredentialsRegistry`]: In-memory registry for credentials lookup

mod ai;
mod inputs;
mod outputs;
mod registry;
pub mod runtime;

pub use ai::{CompletionProviderParams, EmbeddingProviderParams};
use derive_more::From;
pub use inputs::{InputProvider, InputProviderConfig, InputProviderParams};
// Re-export dal credentials
pub use nvisy_dal::provider::{
    AzblobCredentials, GcsCredentials, MysqlCredentials, PgVectorCredentials, PineconeCredentials,
    PostgresCredentials, QdrantCredentials, S3Credentials,
};
// Re-export rig types
pub use nvisy_rig::provider::{
    AnthropicModel, CohereCompletionModel, CohereEmbeddingModel, CompletionCredentials,
    EmbeddingCredentials, GeminiCompletionModel, GeminiEmbeddingModel, OpenAiCompletionModel,
    OpenAiEmbeddingModel, PerplexityModel,
};
pub use outputs::{OutputProvider, OutputProviderConfig, OutputProviderParams};
pub use registry::CredentialsRegistry;
use serde::{Deserialize, Serialize};
use strum::IntoStaticStr;

use crate::error::{Error, Result};

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
    /// pgvector credentials.
    PgVector(PgVectorCredentials),

    // AI providers (completion)
    /// Completion provider credentials.
    Completion(CompletionCredentials),
    /// Embedding provider credentials.
    Embedding(EmbeddingCredentials),
}

impl ProviderCredentials {
    /// Returns the provider kind as a string.
    pub fn kind(&self) -> &'static str {
        self.into()
    }

    /// Converts to completion credentials if applicable.
    pub fn into_completion_credentials(self) -> Result<CompletionCredentials> {
        match self {
            Self::Completion(c) => Ok(c),
            other => Err(Error::Internal(format!(
                "expected completion credentials, got '{}'",
                other.kind()
            ))),
        }
    }

    /// Converts to embedding credentials if applicable.
    pub fn into_embedding_credentials(self) -> Result<EmbeddingCredentials> {
        match self {
            Self::Embedding(c) => Ok(c),
            other => Err(Error::Internal(format!(
                "expected embedding credentials, got '{}'",
                other.kind()
            ))),
        }
    }
}
