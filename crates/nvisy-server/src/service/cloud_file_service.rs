//! Host-side glue for cloud file-service connectors.
//!
//! The OAuth apps config and the connect/refresh logic live in the
//! [`nvisy_file_service`] crate ([`CloudFilesConfig`], [`CloudFileService`]).
//! This module adds only what is host-specific: the post-authorization redirect
//! ([`CloudFilesConfig`] here flattens the crate's config and adds it). Writing
//! refreshed tokens back through Postgres lives in
//! [`persist_oauth`](crate::service::persist_oauth).

use nvisy_file_service::CloudFileService;
use nvisy_file_service::client::OAuthAppsConfig;

/// Deployment configuration for cloud file-service connectors: the crate's
/// per-provider OAuth apps plus the host-side post-auth redirect.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "cli", derive(clap::Args))]
pub struct CloudFilesConfig {
    /// The per-provider OAuth app credentials.
    #[cfg_attr(feature = "cli", command(flatten))]
    pub apps: OAuthAppsConfig,

    /// Frontend URL the OAuth callback redirects the user's browser to once the
    /// flow finishes. The callback appends a `?connection=success` or
    /// `?connection=error` query. Unset falls back to a minimal in-page result.
    #[cfg_attr(feature = "cli", arg(long, env = "CLOUD_FILES_POST_AUTH_REDIRECT_URI"))]
    pub post_auth_redirect_uri: Option<String>,
}

impl CloudFilesConfig {
    /// Builds the [`CloudFileService`] and the post-auth redirect URI.
    pub fn build(self) -> (CloudFileService, Option<String>) {
        (self.apps.build(), self.post_auth_redirect_uri)
    }
}

/// The frontend URL the cloud file OAuth callback redirects the browser to when
/// the flow finishes. A thin `State`-extractable wrapper so the callback handler
/// can reach the configured redirect.
#[derive(Debug, Clone, Default)]
pub struct CloudFilesRedirect(pub Option<String>);
