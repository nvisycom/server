//! Deployment configuration for the cloud file-service OAuth apps.
//!
//! One [`clap::Args`]-derived struct per provider (behind the `cli` feature),
//! mirroring how `NatsConfig` and `PgConfig` are configured. [`OAuthAppsConfig`]
//! resolves these into an [`OAuthApps`] and a [`FileService`].

use super::{FileService, OAuthApps};
use crate::oauth::OAuthApp;

/// One provider's OAuth app credentials.
///
/// Each provider gets its own struct (rather than one flattened four times)
/// because clap-derive cannot prefix a flattened struct's args, so the client
/// id / secret flags — and their `long`/`id` — are declared per provider to avoid
/// collisions when the four are flattened together.
///
/// The redirect URI is *not* here: the server exposes a single OAuth callback
/// route shared by every provider, so it is one deployment-wide value
/// ([`OAuthAppsConfig::redirect_uri`]), not a per-provider one.
macro_rules! provider_app_config {
    (
        $(#[$meta:meta])*
        $name:ident,
        $id_long:literal, $id_env:literal,
        $secret_long:literal, $secret_env:literal
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Default)]
        #[cfg_attr(feature = "cli", derive(clap::Args))]
        pub struct $name {
            /// OAuth client id.
            #[cfg_attr(feature = "cli", arg(id = $id_long, long = $id_long, env = $id_env))]
            pub client_id: Option<String>,
            /// OAuth client secret.
            #[cfg_attr(
                feature = "cli",
                arg(id = $secret_long, long = $secret_long, env = $secret_env)
            )]
            pub client_secret: Option<String>,
        }

        impl $name {
            /// The [`OAuthApp`], present only when the client id and secret are set
            /// to non-empty strings and a shared `redirect_uri` is supplied. An
            /// empty value (e.g. `GOOGLE_DRIVE_CLIENT_ID=` in an env file) counts as
            /// unset, so a placeholder line does not mark the provider as
            /// configured.
            fn to_app(&self, redirect_uri: &str) -> Option<OAuthApp> {
                let non_empty = |value: &Option<String>| {
                    value
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned)
                };
                Some(OAuthApp {
                    client_id: non_empty(&self.client_id)?,
                    client_secret: non_empty(&self.client_secret)?,
                    redirect_uri: redirect_uri.to_owned(),
                })
            }
        }
    };
}

provider_app_config!(
    /// Google Drive OAuth app credentials.
    GoogleDriveConfig,
    "google-drive-client-id",
    "GOOGLE_DRIVE_CLIENT_ID",
    "google-drive-client-secret",
    "GOOGLE_DRIVE_CLIENT_SECRET"
);
provider_app_config!(
    /// Dropbox OAuth app credentials.
    DropboxConfig,
    "dropbox-client-id",
    "DROPBOX_CLIENT_ID",
    "dropbox-client-secret",
    "DROPBOX_CLIENT_SECRET"
);
provider_app_config!(
    /// OneDrive OAuth app credentials.
    OneDriveConfig,
    "onedrive-client-id",
    "ONEDRIVE_CLIENT_ID",
    "onedrive-client-secret",
    "ONEDRIVE_CLIENT_SECRET"
);
provider_app_config!(
    /// Box OAuth app credentials.
    BoxConfig,
    "box-client-id",
    "BOX_CLIENT_ID",
    "box-client-secret",
    "BOX_CLIENT_SECRET"
);

/// Deployment configuration for the cloud file-service OAuth apps: a shared
/// redirect URI plus one credential struct per provider. A provider whose client
/// id and secret are not both set — or when the shared redirect URI is unset — is
/// left unconfigured and cannot be connected.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "cli", derive(clap::Args))]
pub struct OAuthAppsConfig {
    /// The OAuth callback URI, shared by every provider.
    ///
    /// The server exposes a single connection-OAuth callback route, so all
    /// providers redirect to the same URI (which must be registered with each
    /// provider). It differs only by deployment (dev vs prod host), so it is one
    /// deployment-wide value rather than a per-provider one. When unset, no
    /// provider can be connected.
    #[cfg_attr(
        feature = "cli",
        arg(
            id = "file-service-oauth-redirect-uri",
            long = "file-service-oauth-redirect-uri",
            env = "FILE_SERVICE_OAUTH_REDIRECT_URI"
        )
    )]
    pub redirect_uri: Option<String>,

    /// Google Drive OAuth app.
    #[cfg_attr(feature = "cli", command(flatten))]
    pub google_drive: GoogleDriveConfig,
    /// Dropbox OAuth app.
    #[cfg_attr(feature = "cli", command(flatten))]
    pub dropbox: DropboxConfig,
    /// OneDrive OAuth app.
    #[cfg_attr(feature = "cli", command(flatten))]
    pub onedrive: OneDriveConfig,
    /// Box OAuth app.
    #[cfg_attr(feature = "cli", command(flatten))]
    pub box_app: BoxConfig,
}

impl OAuthAppsConfig {
    /// Resolves this configuration into the configured [`OAuthApps`].
    ///
    /// Every provider is left unconfigured when the shared redirect URI is unset
    /// or blank — no provider can complete an OAuth flow without the callback.
    #[must_use]
    pub fn into_apps(self) -> OAuthApps {
        let redirect_uri = self
            .redirect_uri
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let Some(redirect_uri) = redirect_uri else {
            return OAuthApps::default();
        };

        OAuthApps {
            google_drive: self.google_drive.to_app(redirect_uri),
            dropbox: self.dropbox.to_app(redirect_uri),
            onedrive: self.onedrive.to_app(redirect_uri),
            box_app: self.box_app.to_app(redirect_uri),
        }
    }

    /// Builds a [`FileService`] from this configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built (see
    /// [`FileService::new`]).
    pub fn build(self) -> crate::Result<FileService> {
        FileService::new(self.into_apps())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_is_unset_when_credentials_empty_or_missing() {
        let config = OAuthAppsConfig {
            redirect_uri: Some("https://example.com/callback".to_owned()),
            // Both credentials set (non-empty) -> configured.
            google_drive: GoogleDriveConfig {
                client_id: Some("id".to_owned()),
                client_secret: Some("secret".to_owned()),
            },
            // Present but empty/blank (as an env file's `KEY=` yields) -> unset.
            dropbox: DropboxConfig {
                client_id: Some(String::new()),
                client_secret: Some("   ".to_owned()),
            },
            // Partially set (missing secret) -> unset.
            onedrive: OneDriveConfig {
                client_id: Some("id".to_owned()),
                client_secret: None,
            },
            // All None -> unset.
            box_app: BoxConfig::default(),
        };

        let apps = config.into_apps();
        assert!(apps.google_drive.is_some());
        assert!(apps.dropbox.is_none());
        assert!(apps.onedrive.is_none());
        assert!(apps.box_app.is_none());

        // The configured provider carries the shared redirect URI.
        assert_eq!(
            apps.google_drive.unwrap().redirect_uri,
            "https://example.com/callback"
        );
    }

    #[test]
    fn every_provider_unset_without_the_shared_redirect_uri() {
        // Credentials fully set, but no shared redirect URI -> nothing configured,
        // since no provider can complete an OAuth flow without the callback.
        let config = OAuthAppsConfig {
            redirect_uri: None,
            google_drive: GoogleDriveConfig {
                client_id: Some("id".to_owned()),
                client_secret: Some("secret".to_owned()),
            },
            ..Default::default()
        };

        let apps = config.into_apps();
        assert!(apps.google_drive.is_none());
    }
}
