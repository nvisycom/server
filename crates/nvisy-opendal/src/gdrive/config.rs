//! Google Drive configuration.

use serde::{Deserialize, Serialize};

/// Google Drive configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoogleDriveConfig {
    /// Root folder path or ID.
    pub root: String,
    /// OAuth client ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// OAuth client secret.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    /// OAuth access token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    /// OAuth refresh token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

impl GoogleDriveConfig {
    /// Creates a new Google Drive configuration.
    pub fn new(root: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            client_id: None,
            client_secret: None,
            access_token: None,
            refresh_token: None,
        }
    }

    /// Sets the OAuth client credentials.
    pub fn with_client_credentials(
        mut self,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self {
        self.client_id = Some(client_id.into());
        self.client_secret = Some(client_secret.into());
        self
    }

    /// Sets the access token.
    pub fn with_access_token(mut self, access_token: impl Into<String>) -> Self {
        self.access_token = Some(access_token.into());
        self
    }

    /// Sets the refresh token.
    pub fn with_refresh_token(mut self, refresh_token: impl Into<String>) -> Self {
        self.refresh_token = Some(refresh_token.into());
        self
    }
}
