//! MySQL configuration types.

use serde::{Deserialize, Serialize};

/// MySQL credentials (sensitive).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MysqlCredentials {
    /// Connection string (e.g., "mysql://user:pass@host:3306/db").
    pub connection_string: String,
}

/// MySQL parameters (non-sensitive).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MysqlParams {
    /// Table name.
    pub table: String,
    /// Database name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
}
