//! PostgreSQL configuration types.

use serde::{Deserialize, Serialize};

/// PostgreSQL credentials (sensitive).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresCredentials {
    /// Connection string (e.g., "postgresql://user:pass@host:5432/db").
    pub connection_string: String,
}

/// PostgreSQL parameters (non-sensitive).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostgresParams {
    /// Table name.
    pub table: String,
    /// Schema name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
}
