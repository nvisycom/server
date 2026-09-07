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
mod onedrive;

use futures::TryStreamExt;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::Response;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::EnumIter;

use crate::client::{ByteStream, FileServiceClient};
use crate::error::{Error, Result, kind_for_status};
use crate::oauth::{OAuthProvider, OAuthTokens};

/// Tracing target for provider requests.
const TRACING_TARGET: &str = "nvisy_file_service::provider";

/// Adapts a response body into the provider-neutral [`ByteStream`], mapping any
/// stream error into the crate error type. Used by every provider's download.
fn response_stream(response: Response) -> ByteStream {
    Box::pin(response.bytes_stream().map_err(Error::from))
}

/// Sends a provider request and checks its status in one step, surfacing the
/// provider's own error body — the single place every provider request goes
/// through so a failure is legible rather than an opaque status.
#[async_trait::async_trait]
trait ProviderRequest {
    /// Sends the request; on a non-2xx, reads a bounded prefix of the provider's
    /// response body, logs it, and returns an error carrying it, classified by
    /// status. On success, returns the [`Response`] for the caller to consume
    /// (JSON, byte stream, or discard).
    ///
    /// `reqwest`'s own `error_for_status` drops the body, but providers (Dropbox
    /// especially) put the actual reason there, so a bare status is useless for
    /// diagnosis. `provider` labels the log and error for the failing provider.
    async fn send_checked(self, provider: &str) -> Result<Response>;
}

/// Cap on how many bytes of a failed provider response body are read, so a large
/// or unbounded error body cannot cause memory pressure or flood the logs on this
/// shared request path. Comfortably covers the JSON error objects providers
/// return while bounding the pathological case.
const MAX_ERROR_BODY_BYTES: usize = 8 * 1024;

#[async_trait::async_trait]
impl ProviderRequest for reqwest::RequestBuilder {
    async fn send_checked(self, provider: &str) -> Result<Response> {
        let response = self.send().await?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        // Read only a bounded prefix of the body: it may be large or unbounded,
        // and this runs on every provider request.
        let body = read_bounded_body(response, MAX_ERROR_BODY_BYTES).await;
        tracing::warn!(
            target: TRACING_TARGET,
            %provider, status = status.as_u16(), body = %body,
            "Provider request failed",
        );
        let detail: String = body.trim().chars().take(300).collect();
        let message = if detail.is_empty() {
            format!(
                "[{provider}] request failed with status {}",
                status.as_u16()
            )
        } else {
            format!("[{provider}] {detail}")
        };
        Err(Error::new(kind_for_status(status.as_u16()), message))
    }
}

/// Reads at most `cap` bytes of a response body, then stops (dropping the rest of
/// the stream). Best-effort and lossy by design: it is used only to surface a
/// diagnostic prefix of a provider error, so a stream error mid-read yields
/// whatever was read so far, and the bytes are decoded lossily.
async fn read_bounded_body(response: Response, cap: usize) -> String {
    use futures::StreamExt;

    let mut stream = response.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    while buf.len() < cap {
        match stream.next().await {
            Some(Ok(chunk)) => {
                let take = (cap - buf.len()).min(chunk.len());
                buf.extend_from_slice(&chunk[..take]);
            }
            Some(Err(_)) | None => break,
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
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

    /// Whether this provider's browser file picker consumes a user OAuth access
    /// token minted by the server.
    ///
    /// Google Picker, the OneDrive file picker, and the Box picker each take a
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
/// an optional export root (a folder id for Drive, OneDrive, and Box, or a folder
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
