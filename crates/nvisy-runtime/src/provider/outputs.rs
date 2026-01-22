//! Output provider types and implementations.

use derive_more::From;
use nvisy_dal::core::Context;
use nvisy_dal::provider::{
    AzblobConfig, AzblobProvider, GcsConfig, GcsProvider, MilvusConfig, MilvusProvider,
    MysqlConfig, MysqlProvider, PgVectorConfig, PgVectorProvider, PineconeConfig, PineconeProvider,
    PostgresConfig, PostgresProvider, QdrantConfig, QdrantProvider, S3Config, S3Provider,
};
use nvisy_dal::{AnyDataValue, DataTypeId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ProviderCredentials;
use super::backend::{
    AzblobParams, GcsParams, IntoProvider, MilvusParams, MysqlParams, PgVectorParams,
    PineconeParams, PostgresParams, QdrantParams, S3Params,
};
use crate::error::{WorkflowError, WorkflowResult};

/// Output provider parameters (storage backends + vector DBs).
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OutputProviderParams {
    /// Amazon S3 storage.
    S3(S3Params),
    /// Google Cloud Storage.
    Gcs(GcsParams),
    /// Azure Blob Storage.
    Azblob(AzblobParams),
    /// PostgreSQL database.
    Postgres(PostgresParams),
    /// MySQL database.
    Mysql(MysqlParams),
    /// Qdrant vector database.
    Qdrant(QdrantParams),
    /// Pinecone vector database.
    Pinecone(PineconeParams),
    /// Milvus vector database.
    Milvus(MilvusParams),
    /// pgvector (PostgreSQL extension).
    PgVector(PgVectorParams),
}

impl OutputProviderParams {
    /// Returns the credentials ID for this provider.
    pub fn credentials_id(&self) -> Uuid {
        match self {
            Self::S3(p) => p.credentials_id,
            Self::Gcs(p) => p.credentials_id,
            Self::Azblob(p) => p.credentials_id,
            Self::Postgres(p) => p.credentials_id,
            Self::Mysql(p) => p.credentials_id,
            Self::Qdrant(p) => p.credentials_id,
            Self::Pinecone(p) => p.credentials_id,
            Self::Milvus(p) => p.credentials_id,
            Self::PgVector(p) => p.credentials_id,
        }
    }

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

    /// Returns the output data type for this provider.
    pub const fn output_type(&self) -> DataTypeId {
        match self {
            Self::S3(_) | Self::Gcs(_) | Self::Azblob(_) => DataTypeId::Blob,
            Self::Postgres(_) | Self::Mysql(_) => DataTypeId::Record,
            Self::Qdrant(_) | Self::Pinecone(_) | Self::Milvus(_) | Self::PgVector(_) => {
                DataTypeId::Embedding
            }
        }
    }
}

impl IntoProvider for OutputProviderParams {
    type Credentials = ProviderCredentials;
    type Output = OutputProviderConfig;

    fn into_provider(self, credentials: Self::Credentials) -> WorkflowResult<Self::Output> {
        match (self, credentials) {
            (Self::S3(p), ProviderCredentials::S3(c)) => {
                Ok(OutputProviderConfig::S3(p.into_provider(c)?))
            }
            (Self::Gcs(p), ProviderCredentials::Gcs(c)) => {
                Ok(OutputProviderConfig::Gcs(p.into_provider(c)?))
            }
            (Self::Azblob(p), ProviderCredentials::Azblob(c)) => {
                Ok(OutputProviderConfig::Azblob(p.into_provider(c)?))
            }
            (Self::Postgres(p), ProviderCredentials::Postgres(c)) => {
                Ok(OutputProviderConfig::Postgres(p.into_provider(c)?))
            }
            (Self::Mysql(p), ProviderCredentials::Mysql(c)) => {
                Ok(OutputProviderConfig::Mysql(p.into_provider(c)?))
            }
            (Self::Qdrant(p), ProviderCredentials::Qdrant(c)) => {
                Ok(OutputProviderConfig::Qdrant(p.into_provider(c)?))
            }
            (Self::Pinecone(p), ProviderCredentials::Pinecone(c)) => {
                Ok(OutputProviderConfig::Pinecone(p.into_provider(c)?))
            }
            (Self::Milvus(p), ProviderCredentials::Milvus(c)) => {
                Ok(OutputProviderConfig::Milvus(p.into_provider(c)?))
            }
            (Self::PgVector(p), ProviderCredentials::PgVector(c)) => {
                Ok(OutputProviderConfig::PgVector(p.into_provider(c)?))
            }
            (params, creds) => Err(WorkflowError::Internal(format!(
                "credentials type mismatch: expected '{}', got '{}'",
                params.kind(),
                creds.kind()
            ))),
        }
    }
}

/// Resolved output provider config (params + credentials combined).
#[derive(Debug, Clone)]
pub enum OutputProviderConfig {
    S3(S3Config),
    Gcs(GcsConfig),
    Azblob(AzblobConfig),
    Postgres(PostgresConfig),
    Mysql(MysqlConfig),
    Qdrant(QdrantConfig),
    Pinecone(PineconeConfig),
    Milvus(MilvusConfig),
    PgVector(PgVectorConfig),
}

impl OutputProviderConfig {
    /// Creates an output provider from this config.
    pub async fn into_provider(self) -> WorkflowResult<OutputProvider> {
        match self {
            Self::S3(config) => S3Provider::new(&config)
                .map(OutputProvider::S3)
                .map_err(|e| WorkflowError::Internal(e.to_string())),
            Self::Gcs(config) => GcsProvider::new(&config)
                .map(OutputProvider::Gcs)
                .map_err(|e| WorkflowError::Internal(e.to_string())),
            Self::Azblob(config) => AzblobProvider::new(&config)
                .map(OutputProvider::Azblob)
                .map_err(|e| WorkflowError::Internal(e.to_string())),
            Self::Postgres(config) => PostgresProvider::new(&config)
                .map(OutputProvider::Postgres)
                .map_err(|e| WorkflowError::Internal(e.to_string())),
            Self::Mysql(config) => MysqlProvider::new(&config)
                .map(OutputProvider::Mysql)
                .map_err(|e| WorkflowError::Internal(e.to_string())),
            Self::Qdrant(config) => QdrantProvider::new(&config)
                .await
                .map(OutputProvider::Qdrant)
                .map_err(|e| WorkflowError::Internal(e.to_string())),
            Self::Pinecone(config) => PineconeProvider::new(&config)
                .await
                .map(OutputProvider::Pinecone)
                .map_err(|e| WorkflowError::Internal(e.to_string())),
            Self::Milvus(config) => MilvusProvider::new(&config)
                .await
                .map(OutputProvider::Milvus)
                .map_err(|e| WorkflowError::Internal(e.to_string())),
            Self::PgVector(config) => PgVectorProvider::new(&config)
                .await
                .map(OutputProvider::PgVector)
                .map_err(|e| WorkflowError::Internal(e.to_string())),
        }
    }
}

/// Output provider instance (created from config).
#[derive(Debug)]
pub enum OutputProvider {
    S3(S3Provider),
    Gcs(GcsProvider),
    Azblob(AzblobProvider),
    Postgres(PostgresProvider),
    Mysql(MysqlProvider),
    Qdrant(QdrantProvider),
    Pinecone(PineconeProvider),
    Milvus(MilvusProvider),
    PgVector(PgVectorProvider),
}

impl OutputProvider {
    /// Returns the input data type expected by this provider.
    pub const fn input_type(&self) -> DataTypeId {
        match self {
            Self::S3(_) | Self::Gcs(_) | Self::Azblob(_) => DataTypeId::Blob,
            Self::Postgres(_) | Self::Mysql(_) => DataTypeId::Record,
            Self::Qdrant(_) | Self::Pinecone(_) | Self::Milvus(_) | Self::PgVector(_) => {
                DataTypeId::Embedding
            }
        }
    }

    /// Writes data to the provider, accepting type-erased values.
    pub async fn write(&self, ctx: &Context, data: Vec<AnyDataValue>) -> WorkflowResult<()> {
        match self {
            Self::S3(p) => write_data!(p, ctx, data, Blob, into_blob),
            Self::Gcs(p) => write_data!(p, ctx, data, Blob, into_blob),
            Self::Azblob(p) => write_data!(p, ctx, data, Blob, into_blob),
            Self::Postgres(p) => write_data!(p, ctx, data, Record, into_record),
            Self::Mysql(p) => write_data!(p, ctx, data, Record, into_record),
            Self::Qdrant(p) => write_data!(p, ctx, data, Embedding, into_embedding),
            Self::Pinecone(p) => write_data!(p, ctx, data, Embedding, into_embedding),
            Self::Milvus(p) => write_data!(p, ctx, data, Embedding, into_embedding),
            Self::PgVector(p) => write_data!(p, ctx, data, Embedding, into_embedding),
        }
    }
}

/// Helper macro to write data to a provider from AnyDataValue.
macro_rules! write_data {
    ($provider:expr, $ctx:expr, $data:expr, $type:ident, $converter:ident) => {{
        use nvisy_dal::core::DataOutput;
        use nvisy_dal::datatype::$type;

        let items: Vec<$type> = $data.into_iter().filter_map(|v| v.$converter()).collect();

        $provider
            .write($ctx, items)
            .await
            .map_err(|e| WorkflowError::Internal(e.to_string()))
    }};
}

use write_data;
