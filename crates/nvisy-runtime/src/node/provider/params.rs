//! Provider parameters (non-sensitive, part of node definition).

use derive_more::From;
use nvisy_dal::DataTypeId;
use nvisy_dal::provider::{
    AzblobConfig, GcsConfig, MysqlConfig, PostgresConfig, ProviderConfig, S3Config,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    AzblobCredentials, GcsCredentials, MysqlCredentials, PostgresCredentials, ProviderCredentials,
    S3Credentials,
};

/// Provider parameters with credentials reference.
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderParams {
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

impl ProviderParams {
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

    /// Returns the output data type for this provider.
    pub const fn output_type(&self) -> DataTypeId {
        match self {
            Self::S3(_) | Self::Gcs(_) | Self::Azblob(_) => DataTypeId::Blob,
            Self::Postgres(_) | Self::Mysql(_) => DataTypeId::Record,
        }
    }

    /// Combines params with credentials to create a full provider config.
    ///
    /// # Panics
    ///
    /// Panics if the credentials type doesn't match the params type.
    pub fn into_config(self, credentials: ProviderCredentials) -> ProviderConfig {
        match (self, credentials) {
            (Self::S3(p), ProviderCredentials::S3(c)) => ProviderConfig::S3(p.into_config(c)),
            (Self::Gcs(p), ProviderCredentials::Gcs(c)) => ProviderConfig::Gcs(p.into_config(c)),
            (Self::Azblob(p), ProviderCredentials::Azblob(c)) => {
                ProviderConfig::Azblob(p.into_config(c))
            }
            (Self::Postgres(p), ProviderCredentials::Postgres(c)) => {
                ProviderConfig::Postgres(p.into_config(c))
            }
            (Self::Mysql(p), ProviderCredentials::Mysql(c)) => {
                ProviderConfig::Mysql(p.into_config(c))
            }
            _ => panic!("credentials type mismatch"),
        }
    }
}

/// Amazon S3 parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct S3Params {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Bucket name.
    pub bucket: String,
    /// Path prefix within the bucket.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

impl S3Params {
    fn into_config(self, credentials: S3Credentials) -> S3Config {
        let mut config = S3Config::new(self.bucket, credentials.region)
            .with_credentials(credentials.access_key_id, credentials.secret_access_key);

        if let Some(endpoint) = credentials.endpoint {
            config = config.with_endpoint(endpoint);
        }
        if let Some(prefix) = self.prefix {
            config = config.with_prefix(prefix);
        }

        config
    }
}

/// Google Cloud Storage parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GcsParams {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Bucket name.
    pub bucket: String,
    /// Path prefix within the bucket.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

impl GcsParams {
    fn into_config(self, credentials: GcsCredentials) -> GcsConfig {
        let mut config = GcsConfig::new(self.bucket).with_credentials(credentials.credentials_json);

        if let Some(prefix) = self.prefix {
            config = config.with_prefix(prefix);
        }

        config
    }
}

/// Azure Blob Storage parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AzblobParams {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Container name.
    pub container: String,
    /// Path prefix within the container.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

impl AzblobParams {
    fn into_config(self, credentials: AzblobCredentials) -> AzblobConfig {
        let mut config = AzblobConfig::new(credentials.account_name, self.container);

        if let Some(account_key) = credentials.account_key {
            config = config.with_account_key(account_key);
        }
        if let Some(sas_token) = credentials.sas_token {
            config = config.with_sas_token(sas_token);
        }
        if let Some(prefix) = self.prefix {
            config = config.with_prefix(prefix);
        }

        config
    }
}

/// PostgreSQL parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostgresParams {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Table name.
    pub table: String,
    /// Schema name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
}

impl PostgresParams {
    fn into_config(self, credentials: PostgresCredentials) -> PostgresConfig {
        let mut config = PostgresConfig::new(credentials.connection_string).with_table(self.table);

        if let Some(schema) = self.schema {
            config = config.with_schema(schema);
        }

        config
    }
}

/// MySQL parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MysqlParams {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Table name.
    pub table: String,
    /// Database name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
}

impl MysqlParams {
    fn into_config(self, credentials: MysqlCredentials) -> MysqlConfig {
        let mut config = MysqlConfig::new(credentials.connection_string).with_table(self.table);

        if let Some(database) = self.database {
            config = config.with_database(database);
        }

        config
    }
}
