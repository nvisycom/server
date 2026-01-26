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
use strum::AsRefStr;

mod pinecone;
mod postgres;
mod s3;

pub use self::pinecone::{PineconeCredentials, PineconeParams, PineconeProvider};
pub use self::postgres::{PostgresCredentials, PostgresParams, PostgresProvider};
pub use self::s3::{S3Credentials, S3Params, S3Provider};

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

use futures::StreamExt;

use crate::contexts::AnyContext;
use crate::datatypes::AnyDataValue;
use crate::streams::InputStream;
use crate::{DataInput, DataOutput, Error, Result};

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
        }
    }
}

#[async_trait::async_trait]
impl DataInput for AnyProvider {
    type Context = AnyContext;
    type Item = AnyDataValue;

    async fn read(&self, ctx: &Self::Context) -> Result<InputStream<Self::Item>> {
        match self {
            Self::Postgres(provider) => {
                let ctx = ctx.as_relational().cloned().unwrap_or_default();
                let stream = provider.read(&ctx).await?;
                let mapped = stream.map(|r| r.map(AnyDataValue::from));
                Ok(InputStream::new(Box::pin(mapped)))
            }
            Self::S3(provider) => {
                let ctx = ctx.as_object().cloned().unwrap_or_default();
                let stream = provider.read(&ctx).await?;
                let mapped = stream.map(|r| r.map(AnyDataValue::from));
                Ok(InputStream::new(Box::pin(mapped)))
            }
            Self::Pinecone(_) => Err(Error::invalid_input(
                "Pinecone provider does not support reading",
            )),
        }
    }
}

#[async_trait::async_trait]
impl DataOutput for AnyProvider {
    type Item = AnyDataValue;

    async fn write(&self, items: Vec<Self::Item>) -> Result<()> {
        match self {
            Self::Postgres(provider) => {
                let records: Result<Vec<_>> = items
                    .into_iter()
                    .map(|item| match item {
                        AnyDataValue::Record(r) => Ok(r),
                        other => Err(Error::invalid_input(format!(
                            "expected Record, got {:?}",
                            std::mem::discriminant(&other)
                        ))),
                    })
                    .collect();
                provider.write(records?).await
            }
            Self::S3(provider) => {
                let objects: Result<Vec<_>> = items
                    .into_iter()
                    .map(|item| match item {
                        AnyDataValue::Object(o) => Ok(o),
                        other => Err(Error::invalid_input(format!(
                            "expected Object, got {:?}",
                            std::mem::discriminant(&other)
                        ))),
                    })
                    .collect();
                provider.write(objects?).await
            }
            Self::Pinecone(provider) => {
                let embeddings: Result<Vec<_>> = items
                    .into_iter()
                    .map(|item| match item {
                        AnyDataValue::Embedding(e) => Ok(e),
                        other => Err(Error::invalid_input(format!(
                            "expected Embedding, got {:?}",
                            std::mem::discriminant(&other)
                        ))),
                    })
                    .collect();
                provider.write(embeddings?).await
            }
        }
    }
}
