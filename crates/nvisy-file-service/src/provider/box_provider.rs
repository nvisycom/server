//! Box provider, over the Box API and a bearer access token.
//!
//! Box configures an app's scopes in its developer console rather than in the
//! authorize URL, so no scope parameter is sent. Files are addressed by a stable
//! numeric-string `id` (the root folder is `"0"`). Downloads answer with a `302`
//! to a temporary URL that reqwest follows.
//!
//! Box rotates refresh tokens on every use: each refresh invalidates the old
//! token and returns a new one, which the caller must persist. The shared OAuth
//! flow already returns and stores the rotated token, so no special handling is
//! needed here.

use super::{ProviderRequest, encode_path_segment, response_stream};
use crate::client::{ByteStream, FileServiceClient, FileUpload};
use crate::error::Result;
use crate::oauth::OAuthProvider;

/// Provider identifier stored in the connection's `provider` column.
pub const PROVIDER_ID: &str = "box";

/// Box OAuth authorize endpoint.
const AUTH_URL: &str = "https://account.box.com/api/oauth2/authorize";
/// Box OAuth token endpoint.
const TOKEN_URL: &str = "https://api.box.com/oauth2/token";
/// Box API base.
const API_BASE: &str = "https://api.box.com/2.0";
/// Box upload host (separate from the main API host).
const UPLOAD_BASE: &str = "https://upload.box.com/api/2.0";
/// The root folder id.
const ROOT_FOLDER_ID: &str = "0";

/// The OAuth endpoints for Box. Scopes are configured on the app, not requested
/// in the authorize URL, so none are listed here.
#[must_use]
pub fn oauth_provider() -> OAuthProvider {
    OAuthProvider {
        auth_url: AUTH_URL.to_owned(),
        token_url: TOKEN_URL.to_owned(),
        scopes: Vec::new(),
        extra_authorize_params: Vec::new(),
    }
}

/// A connected Box client holding a valid access token.
pub struct BoxClient {
    http: reqwest::Client,
    access_token: String,
    /// The folder id that new exports are created in; `None` is the account root
    /// (`"0"`).
    root_folder_id: Option<String>,
}

impl BoxClient {
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
impl FileServiceClient for BoxClient {
    async fn verify(&self) -> Result<()> {
        self.http
            .get(format!("{API_BASE}/users/me"))
            .bearer_auth(&self.access_token)
            .send_checked(PROVIDER_ID)
            .await?;
        Ok(())
    }

    async fn get_stream(&self, id: &str) -> Result<ByteStream> {
        // /content answers 302 to a temporary dl.boxcloud.com URL; reqwest
        // follows it to the bytes. Encode the id into the path so a stored key
        // cannot alter the request URL.
        let id = encode_path_segment(id);
        let response = self
            .http
            .get(format!("{API_BASE}/files/{id}/content"))
            .bearer_auth(&self.access_token)
            .send_checked(PROVIDER_ID)
            .await?;
        Ok(response_stream(response))
    }

    async fn put_stream(&self, upload: FileUpload<'_>) -> Result<()> {
        // Multipart upload: the `attributes` JSON part MUST precede the `file`
        // part, or Box rejects it with metadata_after_file_contents.
        // `content_length` is the file's size, not the multipart body's, so it is
        // unused here; reqwest frames the multipart body.
        let FileUpload { name, body, .. } = upload;
        let parent = self.root_folder_id.as_deref().unwrap_or(ROOT_FOLDER_ID);
        let attributes =
            serde_json::json!({ "name": name, "parent": { "id": parent } }).to_string();

        let content_part = reqwest::multipart::Part::stream(reqwest::Body::wrap_stream(body))
            .file_name(name.to_owned());
        let form = reqwest::multipart::Form::new()
            .text("attributes", attributes)
            .part("file", content_part);

        self.http
            .post(format!("{UPLOAD_BASE}/files/content"))
            .bearer_auth(&self.access_token)
            .multipart(form)
            .send_checked(PROVIDER_ID)
            .await?;
        Ok(())
    }
}
