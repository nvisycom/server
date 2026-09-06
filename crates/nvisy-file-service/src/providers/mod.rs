//! Typed cloud file-service configuration and provider dispatch.
//!
//! [`Provider`] is the single enum of supported providers; it owns each
//! provider's identity, OAuth endpoints, and client construction, so no caller
//! re-derives per-provider facts. [`FileServiceConfig`] pairs a provider with its
//! [`ConnectionSettings`] (OAuth tokens + sync root) and is what a connection
//! stores, encrypted.

mod box_provider;
mod drive;
mod dropbox;
mod http;
mod onedrive;

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::EnumIter;

use crate::client::FileServiceClient;
use crate::oauth::{OAuthProvider, OAuthTokens};

/// A supported cloud file-service provider.
///
/// The serialized form (snake_case) is the `provider` tag stored on a connection
/// and used in the API, so every provider name lives in exactly one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, EnumIter)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    /// Google Drive.
    GoogleDrive,
    /// Dropbox.
    Dropbox,
    /// OneDrive (Microsoft Graph).
    OneDrive,
    /// Box.
    Box,
}

impl Provider {
    /// The stable identifier stored in the connection's `provider` column and
    /// matching the serialized tag.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::GoogleDrive => drive::PROVIDER_ID,
            Self::Dropbox => dropbox::PROVIDER_ID,
            Self::OneDrive => onedrive::PROVIDER_ID,
            Self::Box => box_provider::PROVIDER_ID,
        }
    }

    /// The OAuth endpoints and scopes for this provider.
    #[must_use]
    pub fn oauth_provider(self) -> OAuthProvider {
        match self {
            Self::GoogleDrive => drive::oauth_provider(),
            Self::Dropbox => dropbox::oauth_provider(),
            Self::OneDrive => onedrive::oauth_provider(),
            Self::Box => box_provider::oauth_provider(),
        }
    }

    /// Builds a connected client for this provider from a valid `access_token`.
    fn connect(
        self,
        http: reqwest::Client,
        access_token: String,
        root: Option<String>,
    ) -> Box<dyn FileServiceClient> {
        match self {
            Self::GoogleDrive => Box::new(drive::DriveClient::new(http, access_token, root)),
            Self::Dropbox => Box::new(dropbox::DropboxClient::new(http, access_token, root)),
            Self::OneDrive => Box::new(onedrive::OneDriveClient::new(http, access_token, root)),
            Self::Box => Box::new(box_provider::BoxClient::new(http, access_token, root)),
        }
    }
}

/// The per-connection settings shared by every provider: the OAuth token set and
/// an optional sync root (a folder id for Drive, OneDrive, and Box, or a folder
/// path for Dropbox). `None` means the account root.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSettings {
    /// The OAuth token set for this connection.
    pub tokens: OAuthTokens,
    /// The folder (id or path) to scope the sync to; `None` uses the root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
}

/// A fully-typed cloud file-service connection configuration: a provider and its
/// settings. Serialized (flat: `{ "provider": ..., "tokens": ..., "root": ... }`)
/// only to persist the config encrypted at rest, never returned in API responses.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct FileServiceConfig {
    /// Which provider backs this connection.
    pub provider: Provider,
    /// The connection's tokens and sync root.
    #[serde(flatten)]
    pub settings: ConnectionSettings,
}

impl FileServiceConfig {
    /// Creates a config for `provider` with the given settings.
    #[must_use]
    pub fn new(provider: Provider, settings: ConnectionSettings) -> Self {
        Self { provider, settings }
    }

    /// The provider identifier for this config, matching the stored `provider`
    /// column and the serialized tag.
    #[must_use]
    pub fn provider_id(&self) -> &'static str {
        self.provider.id()
    }

    /// The stored OAuth tokens for this connection.
    #[must_use]
    pub fn tokens(&self) -> &OAuthTokens {
        &self.settings.tokens
    }

    /// Replaces the stored OAuth tokens (after a refresh).
    pub fn set_tokens(&mut self, new_tokens: OAuthTokens) {
        self.settings.tokens = new_tokens;
    }

    /// Builds a connected client from this config and a valid `access_token`.
    #[must_use]
    pub fn connect(
        &self,
        http: reqwest::Client,
        access_token: String,
    ) -> Box<dyn FileServiceClient> {
        self.provider
            .connect(http, access_token, self.settings.root.clone())
    }
}

#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use super::*;

    /// The stored `id()` must equal the serde tag, since the two are written to
    /// different columns of the same connection and are later compared.
    #[test]
    fn provider_id_matches_serde_tag() {
        for provider in Provider::iter() {
            let tag = serde_json::to_value(provider)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned();
            assert_eq!(
                provider.id(),
                tag,
                "id() and serde tag disagree for {provider:?}"
            );
        }
    }
}
