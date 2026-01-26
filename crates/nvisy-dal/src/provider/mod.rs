//! Provider implementations for external services.
//!
//! Each provider module exports credentials and params types
//! along with the main provider struct.
//!
//! Data types for input/output are in the `core` module:
//! - `Record` for PostgreSQL rows
//! - `Object` for S3 objects
//! - `Embedding` for Pinecone vectors
//!
//! Context types for pagination are in the `core` module:
//! - `RelationalContext` for relational databases
//! - `ObjectContext` for object storage
//! - `VectorContext` for vector databases
//!
//! Available providers:
//! - `postgres`: PostgreSQL relational database
//! - `s3`: AWS S3 / MinIO object storage
//! - `pinecone`: Pinecone vector database

use derive_more::From;
use serde::{Deserialize, Serialize};

mod pinecone;
mod postgres;
mod s3;

pub use self::pinecone::{PineconeCredentials, PineconeParams, PineconeProvider};
pub use self::postgres::{PostgresCredentials, PostgresParams, PostgresProvider};
pub use self::s3::{S3Credentials, S3Params, S3Provider};

/// Type-erased credentials for any provider.
#[derive(Debug, Clone, From, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum AnyCredentials {
    /// PostgreSQL credentials.
    Postgres(PostgresCredentials),
    /// S3 credentials.
    S3(S3Credentials),
    /// Pinecone credentials.
    Pinecone(PineconeCredentials),
}

/// Type-erased parameters for any provider.
#[derive(Debug, Clone, From, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum AnyParams {
    /// PostgreSQL parameters.
    Postgres(PostgresParams),
    /// S3 parameters.
    S3(S3Params),
    /// Pinecone parameters.
    Pinecone(PineconeParams),
}

/// Type-erased provider instance.
#[derive(Debug, From)]
pub enum AnyProvider {
    /// PostgreSQL provider.
    Postgres(PostgresProvider),
    /// S3 provider.
    S3(S3Provider),
    /// Pinecone provider.
    Pinecone(PineconeProvider),
}
