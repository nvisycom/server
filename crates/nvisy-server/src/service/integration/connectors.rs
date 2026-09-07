//! Host-side glue for cloud file-service connectors.
//!
//! The OAuth apps and the connect/refresh logic live in the
//! [`nvisy_file_service`] crate ([`OAuthAppsConfig`], [`FileService`]). This
//! module adds only what is host-specific: [`FileConnectorsConfig`] flattens the
//! crate's config and adds the post-authorization redirect. Writing refreshed
//! tokens back through Postgres lives in this module's `persist_oauth` sibling
//! ([`persist_refreshed_tokens`](super::persist_refreshed_tokens)).

use nvisy_file_service::FileService;
use nvisy_file_service::client::OAuthAppsConfig;

/// Deployment configuration for cloud file-service connectors: the crate's
/// per-provider OAuth apps plus the host-side post-auth redirect.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "cli", derive(clap::Args))]
pub struct FileConnectorsConfig {
    /// The per-provider OAuth app credentials.
    #[cfg_attr(feature = "cli", command(flatten))]
    pub apps: OAuthAppsConfig,

    /// Frontend URL the OAuth callback redirects the user's browser to once the
    /// flow finishes. The callback appends a `?connection=success` or
    /// `?connection=error` query. Unset (or blank) falls back to a minimal
    /// in-page result. Point this at a frontend page, never at the callback
    /// route itself.
    #[cfg_attr(
        feature = "cli",
        arg(long, env = "FILE_SERVICE_POST_AUTH_REDIRECT_URI")
    )]
    pub post_auth_redirect_uri: Option<String>,
}

impl FileConnectorsConfig {
    /// Builds the [`FileService`] and the post-auth redirect.
    ///
    /// # Errors
    ///
    /// Returns an error if the file service's HTTP client cannot be built.
    pub fn build(self) -> crate::Result<(FileService, FileServiceRedirect)> {
        // A blank env var arrives as `Some("")` (clap's `env` does not treat an
        // empty value as unset); normalize it to `None` so an unconfigured
        // redirect falls back to the in-page result rather than producing an
        // empty, relative redirect that loops back onto the callback route.
        let redirect = self
            .post_auth_redirect_uri
            .filter(|uri| !uri.trim().is_empty());
        Ok((self.apps.build()?, FileServiceRedirect(redirect)))
    }
}

/// The frontend URL the cloud file OAuth callback redirects the browser to when
/// the flow finishes. A thin `State`-extractable wrapper so the callback handler
/// can reach the configured redirect.
#[derive(Debug, Clone, Default)]
pub struct FileServiceRedirect(pub Option<String>);
