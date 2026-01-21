//! MySQL provider.

use nvisy_dal::provider::MysqlConfig;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

impl MysqlParams {
    /// Combines params with credentials to create a full provider config.
    pub fn into_config(self, credentials: MysqlCredentials) -> MysqlConfig {
        let mut config = MysqlConfig::new(credentials.connection_string).with_table(self.table);

        if let Some(database) = self.database {
            config = config.with_database(database);
        }

        config
    }
}
