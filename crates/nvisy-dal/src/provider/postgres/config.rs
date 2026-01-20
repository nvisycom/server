//! PostgreSQL configuration.

use serde::{Deserialize, Serialize};

/// PostgreSQL configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostgresConfig {
    /// Connection string (e.g., "postgresql://user:pass@host:5432/db").
    pub connection_string: String,
    /// Default table name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    /// Default schema.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
}

impl PostgresConfig {
    /// Creates a new PostgreSQL configuration.
    pub fn new(connection_string: impl Into<String>) -> Self {
        Self {
            connection_string: connection_string.into(),
            table: None,
            schema: None,
        }
    }

    /// Sets the default table.
    pub fn with_table(mut self, table: impl Into<String>) -> Self {
        self.table = Some(table.into());
        self
    }

    /// Sets the default schema.
    pub fn with_schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = Some(schema.into());
        self
    }
}
