//! Provider implementations for external services.
//!
//! Each provider module exports credentials and params types
//! along with the main provider struct.
//!
//! Data types for input/output are in the `core` module:
//! - `Record` for PostgreSQL rows
//! - `Object` for S3 objects
//! - `Embedding` for vector databases
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
//! - `qdrant`: Qdrant vector database
//! - `milvus`: Milvus vector database
//! - `weaviate`: Weaviate vector database

use derive_more::From;
use serde::{Deserialize, Serialize};
use strum::AsRefStr;

mod milvus;
mod pinecone;
mod postgres;
mod qdrant;
mod s3;
mod weaviate;

pub use self::milvus::{MilvusCredentials, MilvusParams, MilvusProvider};
pub use self::pinecone::{PineconeCredentials, PineconeParams, PineconeProvider};
pub use self::postgres::{PostgresCredentials, PostgresParams, PostgresProvider};
pub use self::qdrant::{QdrantCredentials, QdrantParams, QdrantProvider};
pub use self::s3::{S3Credentials, S3Params, S3Provider};
pub use self::weaviate::{WeaviateCredentials, WeaviateParams, WeaviateProvider};

/// Type-erased credentials for any provider.
#[derive(Debug, Clone, From, AsRefStr, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AnyCredentials {
    /// PostgreSQL credentials.
    Postgres(PostgresCredentials),
    /// S3 credentials.
    S3(S3Credentials),
    /// Pinecone credentials.
    Pinecone(PineconeCredentials),
    /// Qdrant credentials.
    Qdrant(QdrantCredentials),
    /// Milvus credentials.
    Milvus(MilvusCredentials),
    /// Weaviate credentials.
    Weaviate(WeaviateCredentials),
}

/// Type-erased parameters for any provider.
#[derive(Debug, Clone, From, AsRefStr, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AnyParams {
    /// PostgreSQL parameters.
    Postgres(PostgresParams),
    /// S3 parameters.
    S3(S3Params),
    /// Pinecone parameters.
    Pinecone(PineconeParams),
    /// Qdrant parameters.
    Qdrant(QdrantParams),
    /// Milvus parameters.
    Milvus(MilvusParams),
    /// Weaviate parameters.
    Weaviate(WeaviateParams),
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
    /// Qdrant provider.
    Qdrant(QdrantProvider),
    /// Milvus provider.
    Milvus(MilvusProvider),
    /// Weaviate provider.
    Weaviate(WeaviateProvider),
}

#[async_trait::async_trait]
impl crate::Provider for AnyProvider {
    type Credentials = AnyCredentials;
    type Params = AnyParams;

    async fn connect(
        params: Self::Params,
        credentials: Self::Credentials,
    ) -> nvisy_core::Result<Self> {
        match (params, credentials) {
            (AnyParams::Postgres(params), AnyCredentials::Postgres(credentials)) => {
                let provider = PostgresProvider::connect(params, credentials).await?;
                Ok(Self::Postgres(provider))
            }
            (AnyParams::S3(params), AnyCredentials::S3(credentials)) => {
                let provider = S3Provider::connect(params, credentials).await?;
                Ok(Self::S3(provider))
            }
            (AnyParams::Pinecone(params), AnyCredentials::Pinecone(credentials)) => {
                let provider = PineconeProvider::connect(params, credentials).await?;
                Ok(Self::Pinecone(provider))
            }
            (AnyParams::Qdrant(params), AnyCredentials::Qdrant(credentials)) => {
                let provider = QdrantProvider::connect(params, credentials).await?;
                Ok(Self::Qdrant(provider))
            }
            (AnyParams::Milvus(params), AnyCredentials::Milvus(credentials)) => {
                let provider = MilvusProvider::connect(params, credentials).await?;
                Ok(Self::Milvus(provider))
            }
            (AnyParams::Weaviate(params), AnyCredentials::Weaviate(credentials)) => {
                let provider = WeaviateProvider::connect(params, credentials).await?;
                Ok(Self::Weaviate(provider))
            }
            (params, credentials) => Err(nvisy_core::Error::new(
                nvisy_core::ErrorKind::InvalidInput,
            )
            .with_message(format!(
                "mismatched provider types: params={}, credentials={}",
                params.as_ref(),
                credentials.as_ref()
            ))),
        }
    }

    async fn disconnect(self) -> nvisy_core::Result<()> {
        match self {
            Self::Postgres(provider) => provider.disconnect().await,
            Self::S3(provider) => provider.disconnect().await,
            Self::Pinecone(provider) => provider.disconnect().await,
            Self::Qdrant(provider) => provider.disconnect().await,
            Self::Milvus(provider) => provider.disconnect().await,
            Self::Weaviate(provider) => provider.disconnect().await,
        }
    }
}
