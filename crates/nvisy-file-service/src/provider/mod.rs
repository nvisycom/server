//! Typed cloud file-service configuration and provider dispatch.
//!
//! [`FileServiceProvider`] is the single enum of supported providers; it owns each
//! provider's identity, OAuth endpoints, and client construction, so no caller
//! re-derives per-provider facts. [`FileServiceConfig`] pairs a provider with its
//! [`ConnectionSettings`] (OAuth tokens + sync root) and is what a connection
//! stores, encrypted.

mod box_provider;
mod drive;
mod dropbox;
mod onedrive;
mod session;

use futures::TryStreamExt;
use nvisy_core::{ByteStream, Error, ErrorKind, Result, Secret};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::Response;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoStaticStr};

use crate::client::{FileService, FileServiceClient, PickerAccessToken};
use crate::oauth::{OAuthProvider, OAuthTokens};

/// Tracing target for provider requests.
const TRACING_TARGET: &str = "nvisy_file_service::provider";

/// Adapts a response body into the provider-neutral [`ByteStream`], mapping any
/// stream error into the crate error type. Used by every provider's download.
fn response_stream(response: Response) -> ByteStream {
    Box::pin(response.bytes_stream().map_err(Error::from))
}

/// Percent-encodes a provider file id or name for safe interpolation into a
/// request URL path segment.
///
/// A stored key or a user-chosen export name must never alter the request URL:
/// an unencoded `?`, `#`, `/`, or space would silently retarget the request.
/// Encodes conservatively (every non-alphanumeric byte), which is always valid
/// inside a path segment. Every provider that puts an id/name in the URL path
/// routes it through this.
fn encode_path_segment(segment: &str) -> String {
    utf8_percent_encode(segment, NON_ALPHANUMERIC).to_string()
}

/// A supported cloud file-service provider.
///
/// The serialized form (`snake_case`) is the `provider` tag stored on a connection
/// and used in the API, so every provider name lives in exactly one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Deserialize, Serialize, EnumIter, IntoStaticStr)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum FileServiceProvider {
    /// Google Drive.
    GoogleDrive,
    /// Dropbox.
    Dropbox,
    /// `OneDrive` (Microsoft Graph).
    OneDrive,
    /// Box.
    Box,
}

impl FileServiceProvider {
    /// The stable identifier stored in the connection's `provider` column and
    /// matching the serialized tag.
    #[must_use]
    pub fn id(self) -> &'static str {
        self.into()
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

    /// Whether this provider's browser file picker consumes a user OAuth access
    /// token minted by the server.
    ///
    /// Google Picker, the `OneDrive` file picker, and the Box picker each take a
    /// user access token. Dropbox's Chooser is keyed by a public app key instead,
    /// so it needs no server-minted token — a picker-token request for Dropbox is
    /// meaningless and should be rejected.
    #[must_use]
    pub fn picker_needs_user_token(self) -> bool {
        match self {
            Self::GoogleDrive | Self::OneDrive | Self::Box => true,
            Self::Dropbox => false,
        }
    }

    /// Builds a connected client for this provider from a valid `access_token`.
    fn connect(
        self,
        http: reqwest::Client,
        access_token: Secret<String>,
        root: Option<String>,
    ) -> Box<dyn FileServiceClient> {
        match self {
            Self::GoogleDrive => Box::new(drive::DriveClient::new(http, access_token, root)),
            Self::Dropbox => Box::new(dropbox::DropboxClient::new(http, access_token, root)),
            Self::OneDrive => Box::new(onedrive::OneDriveClient::new(http, access_token, root)),
            Self::Box => Box::new(box_provider::BoxClient::new(http, access_token, root)),
        }
    }

    /// Mints a browser file-picker token for a connection of this provider,
    /// dispatching to the provider's picker-token behavior.
    ///
    /// `OneDrive` mints a SharePoint-audience token (its picker requires one,
    /// distinct from the Graph token the connector uses); Google Drive and Box
    /// return their ordinary access token, refreshed if stale; Dropbox's Chooser
    /// uses a client-side app key and has no server token.
    ///
    /// The provider-specific logic lives in each provider module (e.g.
    /// [`onedrive::mint_picker_token`]); this is the dispatch, kept here beside
    /// [`connect`](Self::connect) so the generic [`FileService`] stays
    /// provider-neutral.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider's picker does not use a server token, the
    /// account is unsupported (e.g. a personal `OneDrive` account), or the token
    /// cannot be minted.
    pub(crate) async fn mint_picker_token(
        self,
        service: &FileService,
        config: &FileServiceConfig,
        resource: Option<&str>,
    ) -> Result<PickerAccessToken> {
        match self {
            Self::OneDrive => onedrive::mint_picker_token(service, config, resource).await,
            // Google Drive and Box pickers take the provider's ordinary access
            // token, refreshed if stale.
            Self::GoogleDrive | Self::Box => {
                let fresh = service.ensure_fresh(config).await?;
                let expires_at = fresh
                    .refreshed
                    .as_ref()
                    .unwrap_or(config)
                    .tokens()
                    .expires_at;
                Ok(PickerAccessToken {
                    access_token: fresh.access_token,
                    expires_at,
                    refreshed: fresh.refreshed,
                })
            }
            Self::Dropbox => Err(Error::new(
                ErrorKind::Invalid,
                "this provider's picker uses a client-side app key, not a server token",
            )),
        }
    }
}

/// The per-connection settings shared by every provider: the OAuth token set and
/// an optional export root (a folder id for Drive, `OneDrive`, and Box, or a folder
/// path for Dropbox) that new exports are written into. `None` means the account
/// root.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSettings {
    /// The OAuth token set for this connection.
    pub tokens: OAuthTokens,
    /// The folder (id or path) new exports are written into; `None` uses the
    /// account root.
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
    pub provider: FileServiceProvider,
    /// The connection's tokens and sync root.
    #[serde(flatten)]
    pub settings: ConnectionSettings,
}

impl FileServiceConfig {
    /// Creates a config for `provider` with the given settings.
    #[must_use]
    pub fn new(provider: FileServiceProvider, settings: ConnectionSettings) -> Self {
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
        access_token: Secret<String>,
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
        for provider in FileServiceProvider::iter() {
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
