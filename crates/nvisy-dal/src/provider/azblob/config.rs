//! Azure Blob Storage configuration types.

use serde::{Deserialize, Serialize};

/// Azure Blob Storage credentials (sensitive).
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

/// Azure Blob Storage parameters (non-sensitive).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AzblobParams {
    /// Container name.
    pub container: String,
    /// Path prefix within the container.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}
