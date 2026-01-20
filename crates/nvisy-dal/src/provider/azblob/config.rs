//! Azure Blob Storage configuration.

use serde::{Deserialize, Serialize};

/// Azure Blob Storage configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AzblobConfig {
    /// Storage account name.
    pub account_name: String,
    /// Container name.
    pub container: String,
    /// Account key for authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_key: Option<String>,
    /// SAS token for authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sas_token: Option<String>,
    /// Path prefix within the container.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

impl AzblobConfig {
    /// Creates a new Azure Blob configuration.
    pub fn new(account_name: impl Into<String>, container: impl Into<String>) -> Self {
        Self {
            account_name: account_name.into(),
            container: container.into(),
            account_key: None,
            sas_token: None,
            prefix: None,
        }
    }

    /// Sets the account key.
    pub fn with_account_key(mut self, account_key: impl Into<String>) -> Self {
        self.account_key = Some(account_key.into());
        self
    }

    /// Sets the SAS token.
    pub fn with_sas_token(mut self, sas_token: impl Into<String>) -> Self {
        self.sas_token = Some(sas_token.into());
        self
    }

    /// Sets the path prefix.
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }
}
