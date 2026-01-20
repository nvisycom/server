//! Azure Blob Storage configuration.

use serde::{Deserialize, Serialize};

/// Azure Blob Storage configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AzureBlobConfig {
    /// Container name.
    pub container: String,
    /// Storage account name.
    pub account_name: String,
    /// Storage account key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_key: Option<String>,
    /// Path prefix within the container.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

impl AzureBlobConfig {
    /// Creates a new Azure Blob configuration.
    pub fn new(container: impl Into<String>, account_name: impl Into<String>) -> Self {
        Self {
            container: container.into(),
            account_name: account_name.into(),
            account_key: None,
            prefix: None,
        }
    }

    /// Sets the account key.
    pub fn with_account_key(mut self, account_key: impl Into<String>) -> Self {
        self.account_key = Some(account_key.into());
        self
    }

    /// Sets the path prefix.
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }
}
