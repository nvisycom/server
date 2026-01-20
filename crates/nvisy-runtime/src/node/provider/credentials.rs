//! Provider credentials (sensitive, stored per workspace).

use derive_more::From;
use serde::{Deserialize, Serialize};

/// Provider credentials (sensitive).
#[derive(Debug, Clone, From, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderCredentials {
    /// Amazon S3 credentials.
    S3(S3Credentials),
    /// Google Cloud Storage credentials.
    Gcs(GcsCredentials),
    /// Azure Blob Storage credentials.
    Azblob(AzblobCredentials),
    /// PostgreSQL credentials.
    Postgres(PostgresCredentials),
    /// MySQL credentials.
    Mysql(MysqlCredentials),
}

/// Amazon S3 credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Credentials {
    /// AWS region.
    pub region: String,
    /// Access key ID.
    pub access_key_id: String,
    /// Secret access key.
    pub secret_access_key: String,
    /// Custom endpoint URL (for S3-compatible storage like MinIO, R2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

/// Google Cloud Storage credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcsCredentials {
    /// Service account credentials JSON.
    pub credentials_json: String,
}

/// Azure Blob Storage credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzblobCredentials {
    /// Storage account name.
    pub account_name: String,
    /// Account key for authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_key: Option<String>,
    /// SAS token for authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sas_token: Option<String>,
}

/// PostgreSQL credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresCredentials {
    /// Connection string (e.g., "postgresql://user:pass@host:5432/db").
    pub connection_string: String,
}

/// MySQL credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MysqlCredentials {
    /// Connection string (e.g., "mysql://user:pass@host:3306/db").
    pub connection_string: String,
}
