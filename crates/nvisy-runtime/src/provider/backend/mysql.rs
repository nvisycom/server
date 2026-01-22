//! MySQL provider.

use nvisy_dal::provider::{MysqlConfig, MysqlProvider};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::IntoProvider;
use crate::error::{Error, Result};

/// MySQL credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MysqlCredentials {
    /// Connection string (e.g., "mysql://user:pass@host:3306/db").
    pub connection_string: String,
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

#[async_trait::async_trait]
impl IntoProvider for MysqlParams {
    type Credentials = MysqlCredentials;
    type Output = MysqlProvider;

    async fn into_provider(self, credentials: Self::Credentials) -> Result<Self::Output> {
        let mut config = MysqlConfig::new(credentials.connection_string).with_table(self.table);

        if let Some(database) = self.database {
            config = config.with_database(database);
        }

        MysqlProvider::new(&config).map_err(|e| Error::Internal(e.to_string()))
    }
}
