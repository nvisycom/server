//! OneDrive provider, over the Microsoft Graph API and a bearer access token.
//!
//! Files are addressed by their stable Graph `id`. The download endpoint answers
//! with a `302` to a short-lived, pre-authenticated URL; reqwest follows it (and
//! drops the bearer header on the cross-host hop, as the target requires). The
//! `common` tenant supports both personal and work/school accounts.

use serde::Deserialize;

use super::http::response_stream;
use crate::client::{ByteStream, FileEntry, FileServiceClient};
use crate::error::Result;
use crate::oauth::OAuthProvider;

/// Provider identifier stored in the connection's `provider` column.
///
/// Must match `Provider::OneDrive`'s serde tag (`one_drive`) so the stored
/// provider column and the serialized config agree.
pub const PROVIDER_ID: &str = "one_drive";

/// Microsoft identity platform authorize endpoint (multi-tenant + personal).
const AUTH_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";
/// Microsoft identity platform token endpoint.
const TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";
/// Microsoft Graph v1.0 base.
const API_BASE: &str = "https://graph.microsoft.com/v1.0";

/// The OAuth endpoints and scopes for OneDrive (Microsoft Graph).
#[must_use]
pub fn oauth_provider() -> OAuthProvider {
    OAuthProvider {
        auth_url: AUTH_URL.to_owned(),
        token_url: TOKEN_URL.to_owned(),
        // Files.ReadWrite covers list/download/upload; offline_access yields the
        // refresh token; User.Read enables the cheap /me verify.
        scopes: vec![
            "Files.ReadWrite".to_owned(),
            "offline_access".to_owned(),
            "User.Read".to_owned(),
        ],
        extra_authorize_params: Vec::new(),
    }
}

/// A connected OneDrive client holding a valid access token.
pub struct OneDriveClient {
    http: reqwest::Client,
    access_token: String,
    /// The drive item id of the folder to import from; `None` is the drive root.
    root_folder_id: Option<String>,
}

impl OneDriveClient {
    /// Creates a client from an already-valid access token.
    pub fn new(
        http: reqwest::Client,
        access_token: String,
        root_folder_id: Option<String>,
    ) -> Self {
        Self {
            http,
            access_token,
            root_folder_id,
        }
    }
}

/// One page of a `children` listing.
#[derive(Debug, Deserialize)]
struct ChildrenPage {
    value: Vec<DriveItem>,
    #[serde(default, rename = "@odata.nextLink")]
    next_link: Option<String>,
}

/// A single drive item; the `folder`/`file` facet distinguishes a container.
#[derive(Debug, Deserialize)]
struct DriveItem {
    id: String,
    name: String,
    #[serde(default)]
    folder: Option<serde_json::Value>,
    #[serde(default)]
    package: Option<serde_json::Value>,
}

impl DriveItem {
    /// Whether this item is a container (folder or package), not a file.
    fn is_container(&self) -> bool {
        self.folder.is_some() || self.package.is_some()
    }
}

#[async_trait::async_trait]
impl FileServiceClient for OneDriveClient {
    async fn verify(&self) -> Result<()> {
        self.http
            .get(format!("{API_BASE}/me/drive"))
            .bearer_auth(&self.access_token)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn list(&self) -> Result<Vec<FileEntry>> {
        // The first page URL addresses the configured folder's children, or the
        // drive root's; subsequent pages follow the absolute @odata.nextLink.
        let mut next = Some(match &self.root_folder_id {
            Some(id) => format!("{API_BASE}/me/drive/items/{id}/children"),
            None => format!("{API_BASE}/me/drive/root/children"),
        });
        let mut entries = Vec::new();

        while let Some(url) = next {
            let page: ChildrenPage = self
                .http
                .get(url)
                .bearer_auth(&self.access_token)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;

            entries.extend(
                page.value
                    .into_iter()
                    .filter(|i| !i.is_container())
                    .map(|i| FileEntry {
                        id: i.id,
                        name: i.name,
                    }),
            );
            next = page.next_link;
        }
        Ok(entries)
    }

    async fn get_stream(&self, id: &str) -> Result<ByteStream> {
        // /content answers 302 to a pre-authenticated URL; reqwest follows it and
        // drops the bearer header on the cross-host hop (the target rejects it).
        let response = self
            .http
            .get(format!("{API_BASE}/me/drive/items/{id}/content"))
            .bearer_auth(&self.access_token)
            .send()
            .await?
            .error_for_status()?;
        Ok(response_stream(response))
    }

    async fn put_stream(&self, name: &str, content_type: &str, body: ByteStream) -> Result<()> {
        // Simple PUT upload of a new file into the configured folder (or root),
        // addressed by parent id and file name.
        let url = match &self.root_folder_id {
            Some(id) => format!("{API_BASE}/me/drive/items/{id}:/{name}:/content"),
            None => format!("{API_BASE}/me/drive/root:/{name}:/content"),
        };
        self.http
            .put(url)
            .bearer_auth(&self.access_token)
            .header("Content-Type", content_type)
            .body(reqwest::Body::wrap_stream(body))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}
