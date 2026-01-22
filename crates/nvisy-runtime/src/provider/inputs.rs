//! Input provider types and implementations.

use derive_more::From;
use nvisy_dal::core::Context;
use nvisy_dal::provider::{
    AzblobConfig, AzblobProvider, GcsConfig, GcsProvider, MysqlConfig, MysqlProvider,
    PostgresConfig, PostgresProvider, S3Config, S3Provider,
};
use nvisy_dal::{AnyDataValue, DataTypeId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ProviderCredentials;
use super::backend::{
    AzblobParams, GcsParams, IntoProvider, MysqlParams, PostgresParams, S3Params,
};
use crate::error::{WorkflowError, WorkflowResult};

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

impl IntoProvider for InputProviderParams {
    type Credentials = ProviderCredentials;
    type Output = InputProviderConfig;

    fn into_provider(self, credentials: Self::Credentials) -> WorkflowResult<Self::Output> {
        match (self, credentials) {
            (Self::S3(p), ProviderCredentials::S3(c)) => {
                Ok(InputProviderConfig::S3(p.into_provider(c)?))
            }
            (Self::Gcs(p), ProviderCredentials::Gcs(c)) => {
                Ok(InputProviderConfig::Gcs(p.into_provider(c)?))
            }
            (Self::Azblob(p), ProviderCredentials::Azblob(c)) => {
                Ok(InputProviderConfig::Azblob(p.into_provider(c)?))
            }
            (Self::Postgres(p), ProviderCredentials::Postgres(c)) => {
                Ok(InputProviderConfig::Postgres(p.into_provider(c)?))
            }
            (Self::Mysql(p), ProviderCredentials::Mysql(c)) => {
                Ok(InputProviderConfig::Mysql(p.into_provider(c)?))
            }
            (params, creds) => Err(WorkflowError::Internal(format!(
                "credentials type mismatch: expected '{}', got '{}'",
                params.kind(),
                creds.kind()
            ))),
        }
    }
}

/// Resolved input provider config (params + credentials combined).
#[derive(Debug, Clone)]
pub enum InputProviderConfig {
    S3(S3Config),
    Gcs(GcsConfig),
    Azblob(AzblobConfig),
    Postgres(PostgresConfig),
    Mysql(MysqlConfig),
}

impl InputProviderConfig {
    /// Creates an input provider from this config.
    pub fn into_provider(self) -> WorkflowResult<InputProvider> {
        match self {
            Self::S3(config) => S3Provider::new(&config)
                .map(InputProvider::S3)
                .map_err(|e| WorkflowError::Internal(e.to_string())),
            Self::Gcs(config) => GcsProvider::new(&config)
                .map(InputProvider::Gcs)
                .map_err(|e| WorkflowError::Internal(e.to_string())),
            Self::Azblob(config) => AzblobProvider::new(&config)
                .map(InputProvider::Azblob)
                .map_err(|e| WorkflowError::Internal(e.to_string())),
            Self::Postgres(config) => PostgresProvider::new(&config)
                .map(InputProvider::Postgres)
                .map_err(|e| WorkflowError::Internal(e.to_string())),
            Self::Mysql(config) => MysqlProvider::new(&config)
                .map(InputProvider::Mysql)
                .map_err(|e| WorkflowError::Internal(e.to_string())),
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

    /// Reads data from the provider, returning type-erased values.
    pub async fn read(&self, ctx: &Context) -> WorkflowResult<Vec<AnyDataValue>> {
        match self {
            Self::S3(p) => read_data!(p, ctx, Blob),
            Self::Gcs(p) => read_data!(p, ctx, Blob),
            Self::Azblob(p) => read_data!(p, ctx, Blob),
            Self::Postgres(p) => read_data!(p, ctx, Record),
            Self::Mysql(p) => read_data!(p, ctx, Record),
        }
    }
}

/// Helper macro to read data from a provider and convert to AnyDataValue.
macro_rules! read_data {
    ($provider:expr, $ctx:expr, $variant:ident) => {{
        use futures::StreamExt;
        use nvisy_dal::core::DataInput;
        use nvisy_dal::datatype::$variant;

        let stream = $provider
            .read($ctx)
            .await
            .map_err(|e| WorkflowError::Internal(e.to_string()))?;

        let items: Vec<$variant> = stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| WorkflowError::Internal(e.to_string()))?;

        Ok(items.into_iter().map(AnyDataValue::$variant).collect())
    }};
}

use read_data;
