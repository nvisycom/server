//! Azure Blob Storage provider.

use nvisy_dal::provider::AzblobConfig;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Azure Blob Storage credentials.
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

/// Azure Blob Storage parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AzblobParams {
    /// Reference to stored credentials.
    pub credentials_id: Uuid,
    /// Container name.
    pub container: String,
    /// Path prefix within the container.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

impl AzblobParams {
    /// Combines params with credentials to create a full provider config.
    pub fn into_config(self, credentials: AzblobCredentials) -> AzblobConfig {
        let mut config = AzblobConfig::new(credentials.account_name, self.container);

        if let Some(account_key) = credentials.account_key {
            config = config.with_account_key(account_key);
        }
        if let Some(sas_token) = credentials.sas_token {
            config = config.with_sas_token(sas_token);
        }
        if let Some(prefix) = self.prefix {
            config = config.with_prefix(prefix);
        }

        config
    }
}
