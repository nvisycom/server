//! Input provider types and implementations.

use derive_more::From;
use nvisy_core::Provider;
use nvisy_dal::provider::{
    AzblobParams, AzblobProvider, GcsParams, GcsProvider, MysqlParams, MysqlProvider,
    PostgresParams, PostgresProvider, S3Params, S3Provider,
};
use nvisy_dal::{AnyDataValue, DataTypeId, ObjectContext, RelationalContext};
use serde::{Deserialize, Serialize};
use strum::IntoStaticStr;
use uuid::Uuid;

use super::ProviderCredentials;
use crate::error::{Error, Result};

/// Input provider configuration (credentials reference + params).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputProviderConfig {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Provider-specific parameters.
    #[serde(flatten)]
    pub params: InputProviderParams,
}

impl InputProviderConfig {
    /// Creates a new input provider configuration.
    pub fn new(credentials_id: Uuid, params: InputProviderParams) -> Self {
        Self {
            credentials_id,
            params,
        }
    }

    /// Returns the provider kind as a string.
    pub fn kind(&self) -> &'static str {
        self.params.kind()
    }

    /// Returns the output data type for this provider.
    pub const fn output_type(&self) -> DataTypeId {
        self.params.output_type()
    }

    /// Creates an input provider from this configuration and credentials.
    pub async fn into_provider(self, credentials: ProviderCredentials) -> Result<InputProvider> {
        self.params.into_provider(credentials).await
    }
}

/// Input provider parameters (storage backends only, no vector DBs).
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize, IntoStaticStr)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
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
    /// Returns the provider kind as a string.
    pub fn kind(&self) -> &'static str {
        self.into()
    }

    /// Returns the output data type for this provider.
    pub const fn output_type(&self) -> DataTypeId {
        match self {
            Self::S3(_) | Self::Gcs(_) | Self::Azblob(_) => DataTypeId::Blob,
            Self::Postgres(_) | Self::Mysql(_) => DataTypeId::Record,
        }
    }

    /// Creates an input provider from these params and credentials.
    pub async fn into_provider(self, credentials: ProviderCredentials) -> Result<InputProvider> {
        match (self, credentials) {
            (Self::S3(p), ProviderCredentials::S3(c)) => Ok(InputProvider::S3(
                S3Provider::connect(p, c)
                    .await
                    .map_err(|e| Error::Internal(e.to_string()))?,
            )),
            (Self::Gcs(p), ProviderCredentials::Gcs(c)) => Ok(InputProvider::Gcs(
                GcsProvider::connect(p, c)
                    .await
                    .map_err(|e| Error::Internal(e.to_string()))?,
            )),
            (Self::Azblob(p), ProviderCredentials::Azblob(c)) => Ok(InputProvider::Azblob(
                AzblobProvider::connect(p, c)
                    .await
                    .map_err(|e| Error::Internal(e.to_string()))?,
            )),
            (Self::Postgres(p), ProviderCredentials::Postgres(c)) => Ok(InputProvider::Postgres(
                PostgresProvider::connect(p, c)
                    .await
                    .map_err(|e| Error::Internal(e.to_string()))?,
            )),
            (Self::Mysql(p), ProviderCredentials::Mysql(c)) => Ok(InputProvider::Mysql(
                MysqlProvider::connect(p, c)
                    .await
                    .map_err(|e| Error::Internal(e.to_string()))?,
            )),
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
