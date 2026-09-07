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
/// id / secret / redirect flags — and their `long`/`id` — are declared per
/// provider to avoid collisions when the four are flattened together.
macro_rules! provider_app_config {
    (
        $(#[$meta:meta])*
        $name:ident,
        $id_long:literal, $id_env:literal,
        $secret_long:literal, $secret_env:literal,
        $redirect_long:literal, $redirect_env:literal
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
            /// OAuth redirect URI (the callback route registered with the provider).
            #[cfg_attr(
                feature = "cli",
                arg(id = $redirect_long, long = $redirect_long, env = $redirect_env)
            )]
            pub redirect_uri: Option<String>,
        }

        impl $name {
            /// The [`OAuthApp`], present only when all three values are set to a
            /// non-empty string. An empty value (e.g. `GOOGLE_DRIVE_CLIENT_ID=`
            /// in an env file) counts as unset, so a placeholder line does not
            /// mark the provider as configured.
            fn to_app(&self) -> Option<OAuthApp> {
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
                    redirect_uri: non_empty(&self.redirect_uri)?,
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
    "GOOGLE_DRIVE_CLIENT_SECRET",
    "google-drive-redirect-uri",
    "GOOGLE_DRIVE_REDIRECT_URI"
);
provider_app_config!(
    /// Dropbox OAuth app credentials.
    DropboxConfig,
    "dropbox-client-id",
    "DROPBOX_CLIENT_ID",
    "dropbox-client-secret",
    "DROPBOX_CLIENT_SECRET",
    "dropbox-redirect-uri",
    "DROPBOX_REDIRECT_URI"
);
provider_app_config!(
    /// OneDrive OAuth app credentials.
    OneDriveConfig,
    "onedrive-client-id",
    "ONEDRIVE_CLIENT_ID",
    "onedrive-client-secret",
    "ONEDRIVE_CLIENT_SECRET",
    "onedrive-redirect-uri",
    "ONEDRIVE_REDIRECT_URI"
);
provider_app_config!(
    /// Box OAuth app credentials.
    BoxConfig,
    "box-client-id",
    "BOX_CLIENT_ID",
    "box-client-secret",
    "BOX_CLIENT_SECRET",
    "box-redirect-uri",
    "BOX_REDIRECT_URI"
);

/// Deployment configuration for the cloud file-service OAuth apps: one struct per
/// provider. A provider whose three values are not all set is left unconfigured
/// and cannot be connected.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "cli", derive(clap::Args))]
pub struct OAuthAppsConfig {
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
    #[must_use]
    pub fn into_apps(self) -> OAuthApps {
        OAuthApps {
            google_drive: self.google_drive.to_app(),
            dropbox: self.dropbox.to_app(),
            onedrive: self.onedrive.to_app(),
            box_app: self.box_app.to_app(),
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
    fn provider_is_unset_when_any_value_is_empty_or_missing() {
        let config = OAuthAppsConfig {
            // Fully set (non-empty) -> configured.
            google_drive: GoogleDriveConfig {
                client_id: Some("id".to_owned()),
                client_secret: Some("secret".to_owned()),
                redirect_uri: Some("https://example.com/callback".to_owned()),
            },
            // All present but empty/blank (as an env file's `KEY=` yields) -> unset.
            dropbox: DropboxConfig {
                client_id: Some(String::new()),
                client_secret: Some("   ".to_owned()),
                redirect_uri: Some(String::new()),
            },
            // Partially set (missing secret) -> unset.
            onedrive: OneDriveConfig {
                client_id: Some("id".to_owned()),
                client_secret: None,
                redirect_uri: Some("https://example.com/callback".to_owned()),
            },
            // All None -> unset.
            box_app: BoxConfig::default(),
        };

        let apps = config.into_apps();
        assert!(apps.google_drive.is_some());
        assert!(apps.dropbox.is_none());
        assert!(apps.onedrive.is_none());
        assert!(apps.box_app.is_none());
    }
}
