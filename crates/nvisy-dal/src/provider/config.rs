//! Provider configuration types.

use serde::{Deserialize, Serialize};

use super::{AzblobConfig, GcsConfig, MysqlConfig, PostgresConfig, S3Config};
use crate::datatype::DataTypeId;

/// Unified provider configuration for different backends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderConfig {
    /// Amazon S3 storage.
    S3(S3Config),
    /// Google Cloud Storage.
    Gcs(GcsConfig),
    /// Azure Blob Storage.
    Azblob(AzblobConfig),
    /// PostgreSQL database.
    Postgres(PostgresConfig),
    /// MySQL database.
    Mysql(MysqlConfig),
}

impl ProviderConfig {
    /// Returns the output data type for this provider.
    pub const fn output_type(&self) -> DataTypeId {
        match self {
            Self::S3(_) | Self::Gcs(_) | Self::Azblob(_) => DataTypeId::Blob,
            Self::Postgres(_) | Self::Mysql(_) => DataTypeId::Record,
        }
    }
}
