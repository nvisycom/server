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
            /// The [`OAuthApp`], present only when all three values are set.
            fn to_app(&self) -> Option<OAuthApp> {
                Some(OAuthApp {
                    client_id: self.client_id.clone()?,
                    client_secret: self.client_secret.clone()?,
                    redirect_uri: self.redirect_uri.clone()?,
                })
            }
        }
    };
}

provider_app_config!(
    /// Google Drive OAuth app credentials.
    GoogleDriveConfig,
    "google-drive-client-id", "GOOGLE_DRIVE_CLIENT_ID",
    "google-drive-client-secret", "GOOGLE_DRIVE_CLIENT_SECRET",
    "google-drive-redirect-uri", "GOOGLE_DRIVE_REDIRECT_URI"
);
provider_app_config!(
    /// Dropbox OAuth app credentials.
    DropboxConfig,
    "dropbox-client-id", "DROPBOX_CLIENT_ID",
    "dropbox-client-secret", "DROPBOX_CLIENT_SECRET",
    "dropbox-redirect-uri", "DROPBOX_REDIRECT_URI"
);
provider_app_config!(
    /// OneDrive OAuth app credentials.
    OneDriveConfig,
    "onedrive-client-id", "ONEDRIVE_CLIENT_ID",
    "onedrive-client-secret", "ONEDRIVE_CLIENT_SECRET",
    "onedrive-redirect-uri", "ONEDRIVE_REDIRECT_URI"
);
provider_app_config!(
    /// Box OAuth app credentials.
    BoxConfig,
    "box-client-id", "BOX_CLIENT_ID",
    "box-client-secret", "BOX_CLIENT_SECRET",
    "box-redirect-uri", "BOX_REDIRECT_URI"
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
