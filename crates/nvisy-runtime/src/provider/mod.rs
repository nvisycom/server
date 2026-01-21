//! Provider params, credentials, and registry.
//!
//! This module separates provider configuration into:
//! - [`ProviderCredentials`]: Sensitive credentials (stored per workspace)
//! - [`InputProviderParams`] / [`OutputProviderParams`]: Non-sensitive parameters (part of node definition)
//! - [`CredentialsRegistry`]: In-memory registry for credentials lookup
//!
//! # Module Structure
//!
//! - [`backend`]: Individual provider implementations (credentials + params)
//! - [`inputs`]: Input provider types and read operations
//! - [`outputs`]: Output provider types and write operations
//! - [`registry`]: Credentials registry for workflow execution

pub mod backend;
mod inputs;
mod outputs;
mod registry;
pub mod runtime;

use derive_more::From;
use serde::{Deserialize, Serialize};

pub use backend::{
    AzblobCredentials, AzblobParams, GcsCredentials, GcsParams, MilvusCredentials, MilvusParams,
    MysqlCredentials, MysqlParams, PgVectorCredentials, PgVectorParams, PineconeCredentials,
    PineconeParams, PostgresCredentials, PostgresParams, QdrantCredentials, QdrantParams,
    S3Credentials, S3Params,
};
pub use inputs::{InputProvider, InputProviderConfig, InputProviderParams};
pub use outputs::{OutputProvider, OutputProviderConfig, OutputProviderParams};
pub use registry::CredentialsRegistry;

/// Provider credentials (sensitive).
#[derive(Debug, Clone, From, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub enum ProviderCredentials {
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
    /// Qdrant credentials.
    Qdrant(QdrantCredentials),
    /// Pinecone credentials.
    Pinecone(PineconeCredentials),
    /// Milvus credentials.
    Milvus(MilvusCredentials),
    /// pgvector credentials.
    PgVector(PgVectorCredentials),
}

impl ProviderCredentials {
    /// Returns the provider kind as a string.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::S3(_) => "s3",
            Self::Gcs(_) => "gcs",
            Self::Azblob(_) => "azblob",
            Self::Postgres(_) => "postgres",
            Self::Mysql(_) => "mysql",
            Self::Qdrant(_) => "qdrant",
            Self::Pinecone(_) => "pinecone",
            Self::Milvus(_) => "milvus",
            Self::PgVector(_) => "pgvector",
        }
    }
}
