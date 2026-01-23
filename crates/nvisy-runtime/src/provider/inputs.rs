//! Input provider types and implementations.

use derive_more::From;
use nvisy_dal::provider::{
    AzblobProvider, GcsProvider, MysqlProvider, PostgresProvider, S3Provider,
};
use nvisy_dal::{AnyDataValue, DataTypeId, ObjectContext, RelationalContext};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ProviderCredentials;
use super::backend::{
    AzblobParams, GcsParams, IntoProvider, MysqlParams, PostgresParams, S3Params,
};
use crate::error::{Error, Result};

/// Input provider parameters (storage backends only, no vector DBs).
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputProviderParams {
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
}

impl InputProviderParams {
    /// Returns the credentials ID for this provider.
    pub fn credentials_id(&self) -> Uuid {
        match self {
            Self::S3(p) => p.credentials_id,
            Self::Gcs(p) => p.credentials_id,
            Self::Azblob(p) => p.credentials_id,
            Self::Postgres(p) => p.credentials_id,
            Self::Mysql(p) => p.credentials_id,
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
        }
    }

    /// Returns the output data type for this provider.
    pub const fn output_type(&self) -> DataTypeId {
        match self {
            Self::S3(_) | Self::Gcs(_) | Self::Azblob(_) => DataTypeId::Blob,
            Self::Postgres(_) | Self::Mysql(_) => DataTypeId::Record,
        }
    }
}

#[async_trait::async_trait]
impl IntoProvider for InputProviderParams {
    type Credentials = ProviderCredentials;
    type Output = InputProvider;

    async fn into_provider(self, credentials: Self::Credentials) -> Result<Self::Output> {
        match (self, credentials) {
            (Self::S3(p), ProviderCredentials::S3(c)) => {
                Ok(InputProvider::S3(p.into_provider(c).await?))
            }
            (Self::Gcs(p), ProviderCredentials::Gcs(c)) => {
                Ok(InputProvider::Gcs(p.into_provider(c).await?))
            }
            (Self::Azblob(p), ProviderCredentials::Azblob(c)) => {
                Ok(InputProvider::Azblob(p.into_provider(c).await?))
            }
            (Self::Postgres(p), ProviderCredentials::Postgres(c)) => {
                Ok(InputProvider::Postgres(p.into_provider(c).await?))
            }
            (Self::Mysql(p), ProviderCredentials::Mysql(c)) => {
                Ok(InputProvider::Mysql(p.into_provider(c).await?))
            }
            (params, creds) => Err(Error::Internal(format!(
                "credentials type mismatch: expected '{}', got '{}'",
                params.kind(),
                creds.kind()
            ))),
        }
    }
}

/// Input provider instance (created from config).
#[derive(Debug, Clone)]
pub enum InputProvider {
    S3(S3Provider),
    Gcs(GcsProvider),
    Azblob(AzblobProvider),
    Postgres(PostgresProvider),
    Mysql(MysqlProvider),
}

impl InputProvider {
    /// Returns the output data type for this provider.
    pub const fn output_type(&self) -> DataTypeId {
        match self {
            Self::S3(_) | Self::Gcs(_) | Self::Azblob(_) => DataTypeId::Blob,
            Self::Postgres(_) | Self::Mysql(_) => DataTypeId::Record,
        }
    }

    /// Reads data from the provider as a stream using object context.
    pub async fn read_object_stream(
        &self,
        ctx: &ObjectContext,
    ) -> Result<futures::stream::BoxStream<'static, nvisy_dal::Result<AnyDataValue>>> {
        match self {
            Self::S3(p) => read_stream!(p, ctx, Blob),
            Self::Gcs(p) => read_stream!(p, ctx, Blob),
            Self::Azblob(p) => read_stream!(p, ctx, Blob),
            _ => Err(Error::Internal(
                "Provider does not support ObjectContext".into(),
            )),
        }
    }

    /// Reads data from the provider as a stream using relational context.
    pub async fn read_relational_stream(
        &self,
        ctx: &RelationalContext,
    ) -> Result<futures::stream::BoxStream<'static, nvisy_dal::Result<AnyDataValue>>> {
        match self {
            Self::Postgres(p) => read_stream!(p, ctx, Record),
            Self::Mysql(p) => read_stream!(p, ctx, Record),
            _ => Err(Error::Internal(
                "Provider does not support RelationalContext".into(),
            )),
        }
    }

    /// Reads data from the provider using object context.
    pub async fn read_object(&self, ctx: &ObjectContext) -> Result<Vec<AnyDataValue>> {
        match self {
            Self::S3(p) => read_data!(p, ctx, Blob),
            Self::Gcs(p) => read_data!(p, ctx, Blob),
            Self::Azblob(p) => read_data!(p, ctx, Blob),
            _ => Err(Error::Internal(
                "Provider does not support ObjectContext".into(),
            )),
        }
    }

    /// Reads data from the provider using relational context.
    pub async fn read_relational(&self, ctx: &RelationalContext) -> Result<Vec<AnyDataValue>> {
        match self {
            Self::Postgres(p) => read_data!(p, ctx, Record),
            Self::Mysql(p) => read_data!(p, ctx, Record),
            _ => Err(Error::Internal(
                "Provider does not support RelationalContext".into(),
            )),
        }
    }
}

/// Helper macro to read data from a provider as a boxed stream of AnyDataValue.
macro_rules! read_stream {
    ($provider:expr, $ctx:expr, $variant:ident) => {{
        use futures::StreamExt;
        use nvisy_dal::core::DataInput;

        let stream = $provider
            .read($ctx)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let mapped = stream.map(|result| result.map(AnyDataValue::$variant));
        Ok(Box::pin(mapped) as futures::stream::BoxStream<'static, _>)
    }};
}

use read_stream;

/// Helper macro to read data from a provider and convert to AnyDataValue.
macro_rules! read_data {
    ($provider:expr, $ctx:expr, $variant:ident) => {{
        use futures::StreamExt;
        use nvisy_dal::core::DataInput;
        use nvisy_dal::datatype::$variant;

        let stream = $provider
            .read($ctx)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let items: Vec<$variant> = stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Internal(e.to_string()))?;

        Ok(items.into_iter().map(AnyDataValue::$variant).collect())
    }};
}

use read_data;
