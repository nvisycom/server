//! MySQL configuration.

use serde::{Deserialize, Serialize};

/// MySQL configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MysqlConfig {
    /// Connection string (e.g., "mysql://user:pass@host:3306/db").
    pub connection_string: String,
    /// Default table name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    /// Default database.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
}

impl MysqlConfig {
    /// Creates a new MySQL configuration.
    pub fn new(connection_string: impl Into<String>) -> Self {
        Self {
            connection_string: connection_string.into(),
            table: None,
            database: None,
        }
    }

    /// Sets the default table.
    pub fn with_table(mut self, table: impl Into<String>) -> Self {
        self.table = Some(table.into());
        self
    }

    /// Sets the default database.
    pub fn with_database(mut self, database: impl Into<String>) -> Self {
        self.database = Some(database.into());
        self
    }
}
