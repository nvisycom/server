//! PostgreSQL provider.

use nvisy_dal::provider::{PostgresConfig, PostgresProvider};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::IntoProvider;
use crate::error::{Error, Result};

/// PostgreSQL credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresCredentials {
    /// Connection string (e.g., "postgresql://user:pass@host:5432/db").
    pub connection_string: String,
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

#[async_trait::async_trait]
impl IntoProvider for PostgresParams {
    type Credentials = PostgresCredentials;
    type Output = PostgresProvider;

    async fn into_provider(self, credentials: Self::Credentials) -> Result<Self::Output> {
        let mut config = PostgresConfig::new(credentials.connection_string).with_table(self.table);

        if let Some(schema) = self.schema {
            config = config.with_schema(schema);
        }

        PostgresProvider::new(&config).map_err(|e| Error::Internal(e.to_string()))
    }
}
