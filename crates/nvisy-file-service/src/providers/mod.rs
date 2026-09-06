//! Typed cloud file-service configuration and provider dispatch.

mod box_provider;
mod drive;
mod dropbox;
mod onedrive;

pub use box_provider::{BoxClient, PROVIDER_ID as BOX, oauth_provider as box_oauth};
pub use drive::{DriveClient, PROVIDER_ID as GOOGLE_DRIVE, oauth_provider as drive_oauth};
pub use dropbox::{DropboxClient, PROVIDER_ID as DROPBOX, oauth_provider as dropbox_oauth};
pub use onedrive::{OneDriveClient, PROVIDER_ID as ONEDRIVE, oauth_provider as onedrive_oauth};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::client::FileServiceClient;
use crate::oauth::{OAuthProvider, OAuthTokens};

/// A fully-typed cloud file-service connection configuration.
///
/// The `provider` tag selects the variant and its credential shape. Serialization
/// exists only to persist the config (including OAuth tokens) encrypted at rest,
/// never to return it in API responses.
///
/// Every provider carries its OAuth `tokens` plus an optional `root` that scopes
/// the sync: a folder id for Drive, OneDrive, and Box, or a folder path for
/// Dropbox. `None` means the account root.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "provider", rename_all_fields = "camelCase")]
pub enum FileServiceConfig {
    /// Google Drive.
    #[serde(rename = "google_drive")]
    GoogleDrive {
        /// The OAuth token set for this connection.
        tokens: OAuthTokens,
        /// Drive folder id to scope the sync to; `None` uses the user's root.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        root: Option<String>,
    },
    /// Dropbox.
    #[serde(rename = "dropbox")]
    Dropbox {
        /// The OAuth token set for this connection.
        tokens: OAuthTokens,
        /// Dropbox folder path to scope the sync to; `None` uses the root.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        root: Option<String>,
    },
    /// OneDrive (Microsoft Graph).
    #[serde(rename = "onedrive")]
    OneDrive {
        /// The OAuth token set for this connection.
        tokens: OAuthTokens,
        /// Drive item id of the folder to scope the sync to; `None` uses root.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        root: Option<String>,
    },
    /// Box.
    #[serde(rename = "box")]
    Box {
        /// The OAuth token set for this connection.
        tokens: OAuthTokens,
        /// Box folder id to scope the sync to; `None` uses the account root.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        root: Option<String>,
    },
}

impl FileServiceConfig {
    /// The provider identifier for this config, matching the serialized
    /// `provider` tag and the connection's stored `provider` column.
    #[must_use]
    pub fn provider_id(&self) -> &'static str {
        match self {
            Self::GoogleDrive { .. } => GOOGLE_DRIVE,
            Self::Dropbox { .. } => DROPBOX,
            Self::OneDrive { .. } => ONEDRIVE,
            Self::Box { .. } => BOX,
        }
    }

    /// The OAuth endpoints and scopes for this config's provider.
    #[must_use]
    pub fn oauth_provider(&self) -> OAuthProvider {
        match self {
            Self::GoogleDrive { .. } => drive_oauth(),
            Self::Dropbox { .. } => dropbox_oauth(),
            Self::OneDrive { .. } => onedrive_oauth(),
            Self::Box { .. } => box_oauth(),
        }
    }

    /// The stored OAuth tokens for this connection.
    #[must_use]
    pub fn tokens(&self) -> &OAuthTokens {
        match self {
            Self::GoogleDrive { tokens, .. }
            | Self::Dropbox { tokens, .. }
            | Self::OneDrive { tokens, .. }
            | Self::Box { tokens, .. } => tokens,
        }
    }

    /// Replaces the stored OAuth tokens (after a refresh).
    pub fn set_tokens(&mut self, new_tokens: OAuthTokens) {
        match self {
            Self::GoogleDrive { tokens, .. }
            | Self::Dropbox { tokens, .. }
            | Self::OneDrive { tokens, .. }
            | Self::Box { tokens, .. } => *tokens = new_tokens,
        }
    }

    /// Builds a connected client from this config and a valid `access_token`,
    /// using the shared `http` client for provider requests.
    #[must_use]
    pub fn connect(
        &self,
        http: reqwest::Client,
        access_token: String,
    ) -> Box<dyn FileServiceClient> {
        match self {
            Self::GoogleDrive { root, .. } => {
                Box::new(DriveClient::new(http, access_token, root.clone()))
            }
            Self::Dropbox { root, .. } => {
                Box::new(DropboxClient::new(http, access_token, root.clone()))
            }
            Self::OneDrive { root, .. } => {
                Box::new(OneDriveClient::new(http, access_token, root.clone()))
            }
            Self::Box { root, .. } => Box::new(BoxClient::new(http, access_token, root.clone())),
        }
    }
}
