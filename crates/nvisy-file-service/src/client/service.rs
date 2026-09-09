//! The dependency-injected entry point and the provider-neutral client surface.
//!
//! [`FileService`] holds the shared HTTP client and the configured OAuth
//! apps, and turns a stored [`FileServiceConfig`] into a connected provider
//! client, refreshing the OAuth token when expired. [`FileServiceClient`] is the
//! provider-neutral surface the sync engine drives: verify the connection,
//! stream a file's bytes, and upload a stream. Files to import are chosen through
//! the frontend picker, which hands the backend their ids, so the client does not
//! enumerate the account.
//!
//! Token *persistence* is the caller's responsibility: a refresh returns the
//! updated config via [`ConnectedFileService::refreshed`] for the caller to store.

use std::time::Duration;

use bytes::Bytes;
use futures::stream::BoxStream;

use super::apps::OAuthApps;
use super::connected::ConnectedFileService;
use crate::error::{Error, ErrorKind, Result};
use crate::oauth::{OAuthClient, OAuthTokens};
use crate::provider::{FileServiceConfig, FileServiceProvider};

/// Tracing target for cloud file-service operations.
const TRACING_TARGET: &str = "nvisy_file_service::client";

/// Refresh an access token this many seconds before it actually expires, so a
/// token does not lapse mid-transfer.
const REFRESH_SKEW_SECS: i64 = 60;

/// Maximum time to establish a connection to a provider.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum idle time between received response bytes; bounds a stalled provider
/// without capping a long streaming transfer (no total request timeout is set).
const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Connects cloud file-service configs to their provider backends, refreshing
/// OAuth tokens as needed.
///
/// Cloneable and cheap to pass around; it holds the shared HTTP client and the
/// OAuth app credentials, and builds a fresh provider client per request.
#[derive(Clone)]
#[must_use = "service does nothing unless you use it"]
pub struct FileService {
    http: reqwest::Client,
    apps: OAuthApps,
}

impl FileService {
    /// Creates a new [`FileService`] with the given OAuth app credentials.
    ///
    /// Builds an HTTP client with finite connect and read timeouts so a stalled
    /// provider cannot pin a caller for its whole timeout window; no total
    /// request timeout is set, since transfers are streamed and can run long.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built. Falling back to a
    /// default client would silently drop the connect and read timeouts and let
    /// a stalled provider pin a caller indefinitely.
    pub fn new(apps: OAuthApps) -> Result<Self> {
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .build()
            .map_err(|err| Error::connection("failed to build HTTP client").with_source(err))?;
        Ok(Self { http, apps })
    }

    /// Whether the host has configured an OAuth app for `provider`, and so
    /// whether it can be connected. Lets callers surface availability without
    /// attempting a flow that would fail.
    #[must_use]
    pub fn is_configured(&self, provider: FileServiceProvider) -> bool {
        self.apps.for_provider(provider).is_some()
    }

    /// An [`OAuthClient`] for `provider`, resolving its configured app, or an
    /// error if the host has not configured one. This is the handle for the
    /// authorize URL and the code/token exchanges.
    pub fn oauth_client(&self, provider: FileServiceProvider) -> Result<OAuthClient> {
        let app = self.apps.for_provider(provider).ok_or_else(|| {
            Error::new(
                ErrorKind::BadRequest,
                "this cloud file provider is not configured",
            )
        })?;
        Ok(OAuthClient::new(
            provider.oauth_provider(),
            app.clone(),
            self.http.clone(),
        ))
    }

    /// Ensures `config` has a usable access token, refreshing it if expired.
    ///
    /// The refresh token is required; a config without one that has expired
    /// cannot be renewed and must be reconnected by the user.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorKind::Unauthenticated`] if the token is expired and there is
    /// no refresh token, or the refresh is rejected.
    pub async fn ensure_fresh(&self, config: &FileServiceConfig) -> Result<FreshToken> {
        let now = jiff::Timestamp::now().as_second();
        let tokens = config.tokens();
        if !tokens.is_expired(now, REFRESH_SKEW_SECS) {
            return Ok(FreshToken {
                access_token: tokens.access_token.clone(),
                refreshed: None,
            });
        }

        let refresh_token = tokens.refresh_token.clone().ok_or_else(|| {
            Error::new(
                ErrorKind::Unauthenticated,
                "the connection has expired and must be reconnected",
            )
        })?;

        tracing::debug!(target: TRACING_TARGET, "Refreshing cloud file OAuth token");
        let new_tokens: OAuthTokens = self
            .oauth_client(config.provider)?
            .refresh_tokens(&refresh_token)
            .await?;
        let access_token = new_tokens.access_token.clone();

        let mut refreshed = config.clone();
        refreshed.set_tokens(new_tokens);
        Ok(FreshToken {
            access_token,
            refreshed: Some(refreshed),
        })
    }

    /// The shared HTTP client, for provider-specific helpers within this crate
    /// that need a raw request (e.g. resolving a picker's target resource).
    pub(crate) fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// Mints an access token for `config` scoped to `scopes` (a subset of the
    /// original grant), from the stored refresh token, without disturbing the
    /// connection's primary token. Crate-internal building block for provider
    /// picker-token helpers.
    pub(crate) async fn mint_scoped_token(
        &self,
        config: &FileServiceConfig,
        scopes: &[String],
    ) -> Result<OAuthTokens> {
        let refresh_token = config.tokens().refresh_token.clone().ok_or_else(|| {
            Error::new(
                ErrorKind::Unauthenticated,
                "the connection has expired and must be reconnected",
            )
        })?;
        self.oauth_client(config.provider)?
            .refresh_with_scopes(&refresh_token, scopes)
            .await
    }

    /// Mints a short-lived access token for a provider's browser file picker.
    ///
    /// FileServiceProvider-neutral entry point: the caller passes the connection config and,
    /// when the picker asked for a specific resource, that `resource`; this
    /// dispatches to the provider's picker-token logic. OneDrive mints a
    /// SharePoint-audience token (its picker requires one, distinct from the Graph
    /// token the connector uses); Google Drive and Box return their ordinary
    /// provider token. The connection's primary token is not disturbed; a rotated
    /// refresh token is returned via [`PickerAccessToken::refreshed`] to persist.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider's picker does not use a server token, if
    /// the account is unsupported (e.g. a personal OneDrive account), or if the
    /// token cannot be minted.
    pub async fn mint_picker_token(
        &self,
        config: &FileServiceConfig,
        resource: Option<&str>,
    ) -> Result<PickerAccessToken> {
        // FileServiceProvider dispatch lives on `FileServiceProvider` (beside `connect`), so this stays
        // provider-neutral.
        config
            .provider
            .mint_picker_token(self, config, resource)
            .await
    }

    /// Connects to the file service described by `config`, refreshing its OAuth
    /// token first if needed.
    pub async fn connect(&self, config: &FileServiceConfig) -> Result<ConnectedFileService> {
        let fresh = self.ensure_fresh(config).await?;
        let client = config.connect(self.http.clone(), fresh.access_token);
        Ok(ConnectedFileService {
            client,
            refreshed: fresh.refreshed,
        })
    }
}

/// A usable access token for a connection, from [`FileService::ensure_fresh`]:
/// the access token to use, plus the updated config to persist when a refresh
/// produced new tokens (`None` when the stored token was still valid).
#[must_use]
pub struct FreshToken {
    /// The access token to use for the request.
    pub access_token: String,
    /// The updated config to persist, present only if a refresh happened.
    pub refreshed: Option<FileServiceConfig>,
}

/// A minted browser file-picker access token: the token, its expiry (Unix
/// seconds, if the provider returned one), and — when minting rotated the
/// connection's refresh token — the updated config for the caller to persist.
#[must_use]
pub struct PickerAccessToken {
    /// The access token to hand to the picker.
    pub access_token: String,
    /// Expiry as a Unix timestamp, if known.
    pub expires_at: Option<i64>,
    /// The updated config to persist, present only if the refresh token rotated.
    pub refreshed: Option<FileServiceConfig>,
}

/// A byte stream, the shape both directions of a transfer move data in.
pub type ByteStream = BoxStream<'static, Result<Bytes>>;

/// A streamed file upload: the destination name, its content type, the exact
/// byte length, and the body stream.
///
/// `content_length` must equal the number of bytes `body` yields — a provider
/// that needs the length up front (Dropbox rejects a chunked body) sets it as
/// the request `Content-Length`, and a mismatch fails the upload.
#[must_use]
pub struct FileUpload<'a> {
    /// The name of the new file to create on the provider.
    pub name: &'a str,
    /// The MIME type of the content.
    pub content_type: &'a str,
    /// The exact number of bytes `body` yields.
    pub content_length: u64,
    /// The file's bytes, streamed to the provider.
    pub body: ByteStream,
}

/// FileServiceProvider-neutral read/write access to a connected file service.
#[async_trait::async_trait]
pub trait FileServiceClient: Send + Sync {
    /// Verifies the connection is reachable with the current credentials,
    /// without transferring any file.
    async fn verify(&self) -> Result<()>;

    /// Streams one file's bytes without buffering the whole file in memory. The
    /// `id` is a provider file id, as chosen through the picker.
    async fn get_stream(&self, id: &str) -> Result<ByteStream>;

    /// Uploads a new file, streaming its body to the provider.
    async fn put_stream(&self, upload: FileUpload<'_>) -> Result<()>;
}
