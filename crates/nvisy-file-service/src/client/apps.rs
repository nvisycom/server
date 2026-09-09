//! The configured OAuth apps, keyed by provider.

use crate::oauth::OAuthApp;
use crate::provider::FileServiceProvider;

/// The OAuth application credentials for each supported provider, supplied by the
/// host's configuration. A provider with no configured app cannot be connected.
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
}

impl OAuthApps {
    /// The configured app for `provider`, if any.
    #[must_use]
    pub fn for_provider(&self, provider: FileServiceProvider) -> Option<&OAuthApp> {
        match provider {
            FileServiceProvider::GoogleDrive => self.google_drive.as_ref(),
            FileServiceProvider::Dropbox => self.dropbox.as_ref(),
            FileServiceProvider::OneDrive => self.onedrive.as_ref(),
            FileServiceProvider::Box => self.box_app.as_ref(),
        }
    }
}
