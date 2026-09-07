//! OneDrive provider, over the Microsoft Graph API and a bearer access token.
//!
//! Files are addressed by their stable Graph `id`. The download endpoint answers
//! with a `302` to a short-lived, pre-authenticated URL; reqwest follows it (and
//! drops the bearer header on the cross-host hop, as the target requires). The
//! `common` tenant supports both personal and work/school accounts.

use reqwest::header::CONTENT_LENGTH;

use super::{ProviderRequest, encode_path_segment, response_stream};
use crate::client::{ByteStream, FileServiceClient, FileUpload};
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
        // Files.ReadWrite covers download and upload; offline_access yields the
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
    /// The drive item id of the folder new exports are created in; `None` is the
    /// drive root.
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

#[async_trait::async_trait]
impl FileServiceClient for OneDriveClient {
    async fn verify(&self) -> Result<()> {
        self.http
            .get(format!("{API_BASE}/me/drive"))
            .bearer_auth(&self.access_token)
            .send_checked(PROVIDER_ID)
            .await?;
        Ok(())
    }

    async fn get_stream(&self, id: &str) -> Result<ByteStream> {
        // /content answers 302 to a pre-authenticated URL; reqwest follows it and
        // drops the bearer header on the cross-host hop (the target rejects it).
        // Encode the id into the path segment so a stored key can never alter the
        // request URL.
        let id = encode_path_segment(id);
        let response = self
            .http
            .get(format!("{API_BASE}/me/drive/items/{id}/content"))
            .bearer_auth(&self.access_token)
            .send_checked(PROVIDER_ID)
            .await?;
        Ok(response_stream(response))
    }

    async fn put_stream(&self, upload: FileUpload<'_>) -> Result<()> {
        let FileUpload {
            name,
            content_type,
            content_length,
            body,
        } = upload;
        // Simple PUT upload of a new file into the configured folder (or root),
        // addressed by parent id and file name. Both are encoded into the path so
        // a `?`, `#`, or `/` in the name cannot retarget the request URL. The body
        // is streamed with an explicit length (the whole body is the file's bytes).
        let name = encode_path_segment(name);
        let url = match &self.root_folder_id {
            Some(id) => {
                let id = encode_path_segment(id);
                format!("{API_BASE}/me/drive/items/{id}:/{name}:/content")
            }
            None => format!("{API_BASE}/me/drive/root:/{name}:/content"),
        };
        self.http
            .put(url)
            .bearer_auth(&self.access_token)
            .header("Content-Type", content_type)
            .header(CONTENT_LENGTH, content_length)
            .body(reqwest::Body::wrap_stream(body))
            .send_checked(PROVIDER_ID)
            .await?;
        Ok(())
    }
}
