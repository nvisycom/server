//! Backend provider implementations.
//!
//! Each provider file contains credentials and params for a specific backend:
//!
//! ## Storage backends
//! - `s3` - Amazon S3
//! - `gcs` - Google Cloud Storage
//! - `azblob` - Azure Blob Storage
//! - `postgres` - PostgreSQL
//! - `mysql` - MySQL
//!
//! ## Vector databases
//! - `qdrant` - Qdrant vector database
//! - `pinecone` - Pinecone vector database
//! - `milvus` - Milvus vector database
//! - `pgvector` - pgvector (PostgreSQL extension)
//!
//! ## AI providers
//! - `openai` - OpenAI (completion + embedding)
//! - `anthropic` - Anthropic (completion only)
//! - `cohere` - Cohere (completion + embedding)
//! - `gemini` - Google Gemini (completion + embedding)
//! - `perplexity` - Perplexity (completion only)

use crate::error::Result;

// Storage backends
mod azblob;
mod gcs;
mod mysql;
mod postgres;
mod s3;

// Vector databases
mod milvus;
mod pgvector;
mod pinecone;
mod qdrant;

// AI providers
mod anthropic;
mod cohere;
mod gemini;
mod openai;
mod perplexity;

// Storage backend exports
pub use azblob::{AzblobCredentials, AzblobParams};
pub use gcs::{GcsCredentials, GcsParams};
pub use mysql::{MysqlCredentials, MysqlParams};
pub use postgres::{PostgresCredentials, PostgresParams};
pub use s3::{S3Credentials, S3Params};

// Vector database exports
pub use milvus::{MilvusCredentials, MilvusParams};
pub use pgvector::{PgVectorCredentials, PgVectorParams};
pub use pinecone::{PineconeCredentials, PineconeParams};
pub use qdrant::{QdrantCredentials, QdrantParams};

// AI provider exports
pub use anthropic::{AnthropicCompletionParams, AnthropicCredentials};
pub use cohere::{CohereCompletionParams, CohereCredentials, CohereEmbeddingParams};
pub use gemini::{GeminiCompletionParams, GeminiCredentials, GeminiEmbeddingParams};
pub use openai::{OpenAiCompletionParams, OpenAiCredentials, OpenAiEmbeddingParams};
pub use perplexity::{PerplexityCompletionParams, PerplexityCredentials};

/// Trait for provider parameters that can be combined with credentials to create a provider.
#[async_trait::async_trait]
pub trait IntoProvider {
    /// The credentials type required by this provider.
    type Credentials: Send;
    /// The output type (provider instance).
    type Output;

    /// Combines params with credentials to create the provider.
    async fn into_provider(self, credentials: Self::Credentials) -> Result<Self::Output>;
}
