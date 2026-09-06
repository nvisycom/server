//! Cloud file-service access.
//!
//! Bridges stored workspace connections to the [`nvisy_file_service`] providers:
//! a connection carries an encrypted, typed
//! [`FileServiceConfig`](nvisy_file_service::providers::FileServiceConfig) whose
//! OAuth tokens [`CloudFileService`] refreshes (when expired) before building a
//! connected client. The sync orchestration built on top lives in the
//! [`sync`](crate::service::sync) module.

use nvisy_file_service::client::FileServiceClient;
use nvisy_file_service::oauth::{self, OAuthApp, OAuthTokens};
use nvisy_file_service::providers::FileServiceConfig;

use crate::handler::{ErrorKind, Result};

/// Tracing target for cloud file-service operations.
const TRACING_TARGET: &str = "nvisy_server::service::cloud_file_service";

/// Refresh an access token this many seconds before it actually expires, so a
/// token does not lapse mid-transfer.
const REFRESH_SKEW_SECS: i64 = 60;

/// The OAuth application credentials for each supported cloud file provider,
/// supplied by configuration. A provider with no configured app cannot be
/// connected.
#[derive(Debug, Clone, Default)]
pub struct OAuthApps {
    /// The Google Drive OAuth app, if configured.
    pub google_drive: Option<OAuthApp>,
    /// The Dropbox OAuth app, if configured.
    pub dropbox: Option<OAuthApp>,
    /// The OneDrive OAuth app, if configured.
    pub onedrive: Option<OAuthApp>,
    /// The Box OAuth app, if configured.
    pub box_app: Option<OAuthApp>,
    /// Frontend URL the callback redirects to when the flow finishes.
    pub post_auth_redirect_uri: Option<String>,
}

/// Deployment configuration for the cloud file-service OAuth apps.
///
/// Each provider needs a client id, secret, and redirect URI (the callback
/// route). A provider whose three values are not all set is left unconfigured
/// and cannot be connected.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "cli", derive(clap::Args))]
pub struct CloudFilesConfig {
    /// Google Drive OAuth client id.
    #[cfg_attr(feature = "cli", arg(long, env = "GOOGLE_DRIVE_CLIENT_ID"))]
    pub google_drive_client_id: Option<String>,
    /// Google Drive OAuth client secret.
    #[cfg_attr(feature = "cli", arg(long, env = "GOOGLE_DRIVE_CLIENT_SECRET"))]
    pub google_drive_client_secret: Option<String>,
    /// Google Drive OAuth redirect URI.
    #[cfg_attr(feature = "cli", arg(long, env = "GOOGLE_DRIVE_REDIRECT_URI"))]
    pub google_drive_redirect_uri: Option<String>,

    /// Dropbox OAuth client id.
    #[cfg_attr(feature = "cli", arg(long, env = "DROPBOX_CLIENT_ID"))]
    pub dropbox_client_id: Option<String>,
    /// Dropbox OAuth client secret.
    #[cfg_attr(feature = "cli", arg(long, env = "DROPBOX_CLIENT_SECRET"))]
    pub dropbox_client_secret: Option<String>,
    /// Dropbox OAuth redirect URI.
    #[cfg_attr(feature = "cli", arg(long, env = "DROPBOX_REDIRECT_URI"))]
    pub dropbox_redirect_uri: Option<String>,

    /// OneDrive OAuth client id.
    #[cfg_attr(feature = "cli", arg(long, env = "ONEDRIVE_CLIENT_ID"))]
    pub onedrive_client_id: Option<String>,
    /// OneDrive OAuth client secret.
    #[cfg_attr(feature = "cli", arg(long, env = "ONEDRIVE_CLIENT_SECRET"))]
    pub onedrive_client_secret: Option<String>,
    /// OneDrive OAuth redirect URI.
    #[cfg_attr(feature = "cli", arg(long, env = "ONEDRIVE_REDIRECT_URI"))]
    pub onedrive_redirect_uri: Option<String>,

    /// Box OAuth client id.
    #[cfg_attr(feature = "cli", arg(long, env = "BOX_CLIENT_ID"))]
    pub box_client_id: Option<String>,
    /// Box OAuth client secret.
    #[cfg_attr(feature = "cli", arg(long, env = "BOX_CLIENT_SECRET"))]
    pub box_client_secret: Option<String>,
    /// Box OAuth redirect URI.
    #[cfg_attr(feature = "cli", arg(long, env = "BOX_REDIRECT_URI"))]
    pub box_redirect_uri: Option<String>,

    /// Frontend URL the OAuth callback redirects the user's browser to once the
    /// flow finishes. The callback appends a `?connection=success` or
    /// `?connection=error` query. Unset falls back to a minimal in-page result.
    #[cfg_attr(feature = "cli", arg(long, env = "CLOUD_FILES_POST_AUTH_REDIRECT_URI"))]
    pub post_auth_redirect_uri: Option<String>,
}

/// Assembles an [`OAuthApp`] only when all three of its values are set.
fn app(
    client_id: Option<String>,
    client_secret: Option<String>,
    redirect_uri: Option<String>,
) -> Option<OAuthApp> {
    match (client_id, client_secret, redirect_uri) {
        (Some(client_id), Some(client_secret), Some(redirect_uri)) => Some(OAuthApp {
            client_id,
            client_secret,
            redirect_uri,
        }),
        _ => None,
    }
}

impl CloudFilesConfig {
    /// Resolves the configured OAuth apps. A provider is included only when all
    /// three of its values are set.
    #[must_use]
    pub fn into_apps(self) -> OAuthApps {
        OAuthApps {
            google_drive: app(
                self.google_drive_client_id,
                self.google_drive_client_secret,
                self.google_drive_redirect_uri,
            ),
            dropbox: app(
                self.dropbox_client_id,
                self.dropbox_client_secret,
                self.dropbox_redirect_uri,
            ),
            onedrive: app(
                self.onedrive_client_id,
                self.onedrive_client_secret,
                self.onedrive_redirect_uri,
            ),
            box_app: app(
                self.box_client_id,
                self.box_client_secret,
                self.box_redirect_uri,
            ),
            post_auth_redirect_uri: self.post_auth_redirect_uri,
        }
    }
}

/// A connected client plus, when a refresh happened, the new tokens the caller
/// must persist back to the connection.
pub struct ConnectedFileService {
    /// The connected provider client.
    pub client: Box<dyn FileServiceClient>,
    /// Present only when the access token was refreshed: the updated config to
    /// re-encrypt and store, so the next sync starts from fresh tokens.
    pub refreshed: Option<FileServiceConfig>,
}

/// Connects workspace connections to their cloud file-service backends,
/// refreshing OAuth tokens as needed.
///
/// Cloneable and cheap to pass around; it holds the shared HTTP client and the
/// OAuth app credentials, and builds a fresh provider client per request.
#[derive(Clone)]
#[must_use = "service does nothing unless you use it"]
pub struct CloudFileService {
    http: reqwest::Client,
    apps: OAuthApps,
}

impl CloudFileService {
    /// Creates a new [`CloudFileService`] with the given HTTP client and OAuth
    /// app credentials.
    pub fn new(http: reqwest::Client, apps: OAuthApps) -> Self {
        Self { http, apps }
    }

    /// The shared HTTP client, for driving OAuth token exchanges.
    #[must_use]
    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// The configured OAuth apps.
    #[must_use]
    pub fn apps(&self) -> &OAuthApps {
        &self.apps
    }

    /// The configured OAuth app for a config's provider, or an error if the
    /// deployment has not configured one.
    fn app_for(&self, config: &FileServiceConfig) -> Result<&OAuthApp> {
        let app = match config {
            FileServiceConfig::GoogleDrive { .. } => self.apps.google_drive.as_ref(),
            FileServiceConfig::Dropbox { .. } => self.apps.dropbox.as_ref(),
            FileServiceConfig::OneDrive { .. } => self.apps.onedrive.as_ref(),
            FileServiceConfig::Box { .. } => self.apps.box_app.as_ref(),
        };
        app.ok_or_else(|| {
            ErrorKind::BadRequest.with_message("This cloud file provider is not configured")
        })
    }

    /// Refreshes the OAuth access token for `config` if it is expired, returning
    /// the config to persist when a refresh happened.
    ///
    /// The refresh token is required; a config without one that has expired
    /// cannot be renewed and must be reconnected by the user.
    pub async fn ensure_fresh(
        &self,
        config: &FileServiceConfig,
    ) -> Result<(String, Option<FileServiceConfig>)> {
        let now = jiff::Timestamp::now().as_second();
        let tokens = config.tokens();
        if !tokens.is_expired(now, REFRESH_SKEW_SECS) {
            return Ok((tokens.access_token.clone(), None));
        }

        let refresh_token = tokens.refresh_token.clone().ok_or_else(|| {
            ErrorKind::BadRequest.with_message("The connection has expired and must be reconnected")
        })?;
        let app = self.app_for(config)?;
        let provider = config.oauth_provider();

        tracing::debug!(target: TRACING_TARGET, "Refreshing cloud file OAuth token");
        let new_tokens: OAuthTokens =
            oauth::refresh_tokens(&provider, app, &self.http, &refresh_token).await?;
        let access_token = new_tokens.access_token.clone();

        let mut refreshed = config.clone();
        refreshed.set_tokens(new_tokens);
        Ok((access_token, Some(refreshed)))
    }

    /// Connects to the file service described by `config`, refreshing its OAuth
    /// token first if needed.
    pub async fn connect(&self, config: &FileServiceConfig) -> Result<ConnectedFileService> {
        let (access_token, refreshed) = self.ensure_fresh(config).await?;
        let client = config.connect(self.http.clone(), access_token);
        Ok(ConnectedFileService { client, refreshed })
    }
}
