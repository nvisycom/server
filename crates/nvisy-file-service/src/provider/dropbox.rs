//! Dropbox provider: the file operations the sync engine needs, over the
//! Dropbox HTTP API v2 and a bearer access token.
//!
//! Dropbox splits its API across two hosts — metadata/JSON calls on
//! `api.dropboxapi.com`, byte transfers on `content.dropboxapi.com` — and passes
//! the per-call argument for transfers in a `Dropbox-API-Arg` header whose JSON
//! must be ASCII-safe. Files are addressed by a stable `id:...` key.

use super::response_stream;
use crate::client::{ByteStream, FileServiceClient};
use crate::error::Result;
use crate::oauth::OAuthProvider;

/// Provider identifier stored in the connection's `provider` column.
pub const PROVIDER_ID: &str = "dropbox";

/// Dropbox OAuth authorize endpoint.
const AUTH_URL: &str = "https://www.dropbox.com/oauth2/authorize";
/// Dropbox OAuth token endpoint.
const TOKEN_URL: &str = "https://api.dropboxapi.com/oauth2/token";
/// JSON/metadata API host.
const API_BASE: &str = "https://api.dropboxapi.com/2";
/// Byte-transfer (download/upload) API host.
const CONTENT_BASE: &str = "https://content.dropboxapi.com/2";

/// The OAuth endpoints and scopes for Dropbox.
#[must_use]
pub fn oauth_provider() -> OAuthProvider {
    OAuthProvider {
        auth_url: AUTH_URL.to_owned(),
        token_url: TOKEN_URL.to_owned(),
        scopes: vec![
            "account_info.read".to_owned(),
            "files.metadata.read".to_owned(),
            "files.content.read".to_owned(),
            "files.content.write".to_owned(),
        ],
        // Dropbox issues a refresh token only for an offline-access grant.
        extra_authorize_params: vec![("token_access_type".to_owned(), "offline".to_owned())],
    }
}

/// A connected Dropbox client holding a valid access token.
pub struct DropboxClient {
    http: reqwest::Client,
    access_token: String,
    /// The folder path new exports are written into; `None` (or empty) is the
    /// account root.
    root_path: Option<String>,
}

impl DropboxClient {
    /// Creates a client from an already-valid access token.
    ///
    /// The user-supplied `root_path` is normalized to Dropbox's path contract:
    /// the account root is the empty string, and a folder must begin with `/`
    /// and carry no trailing slash. This makes a user-entered `Team/Reports`
    /// behave the same as `/Team/Reports/`.
    pub fn new(http: reqwest::Client, access_token: String, root_path: Option<String>) -> Self {
        Self {
            http,
            access_token,
            root_path: normalize_root(root_path),
        }
    }
}

/// Normalizes a configured Dropbox root into a valid path: `None`/blank becomes
/// the account root (`None`), otherwise a single leading slash and no trailing
/// slash.
fn normalize_root(root_path: Option<String>) -> Option<String> {
    let trimmed = root_path?.trim().trim_matches('/').to_owned();
    if trimmed.is_empty() {
        None
    } else {
        Some(format!("/{trimmed}"))
    }
}

/// Serializes a `Dropbox-API-Arg` value with all non-ASCII bytes escaped as
/// `\uXXXX`, as Dropbox requires for the HTTP header.
fn api_arg(value: &serde_json::Value) -> String {
    let json = value.to_string();
    let mut escaped = String::with_capacity(json.len());
    for ch in json.chars() {
        if ch.is_ascii() && ch != '\u{7f}' {
            escaped.push(ch);
        } else {
            for unit in ch.encode_utf16(&mut [0u16; 2]) {
                escaped.push_str(&format!("\\u{unit:04x}"));
            }
        }
    }
    escaped
}

#[async_trait::async_trait]
impl FileServiceClient for DropboxClient {
    async fn verify(&self) -> Result<()> {
        // get_current_account is the idiomatic whoami; it takes an empty body.
        self.http
            .post(format!("{API_BASE}/users/get_current_account"))
            .bearer_auth(&self.access_token)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn get_stream(&self, id: &str) -> Result<ByteStream> {
        let arg = api_arg(&serde_json::json!({ "path": id }));
        let response = self
            .http
            .post(format!("{CONTENT_BASE}/files/download"))
            .bearer_auth(&self.access_token)
            .header("Dropbox-API-Arg", arg)
            .send()
            .await?
            .error_for_status()?;
        Ok(response_stream(response))
    }

    async fn put_stream(&self, name: &str, _content_type: &str, body: ByteStream) -> Result<()> {
        // Upload into the configured root (or account root), keeping the file
        // name; autorename avoids clobbering an existing name.
        let root = self.root_path.as_deref().unwrap_or("");
        let path = format!("{root}/{name}");
        let arg = api_arg(&serde_json::json!({
            "path": path,
            "mode": "add",
            "autorename": true,
        }));

        self.http
            .post(format!("{CONTENT_BASE}/files/upload"))
            .bearer_auth(&self.access_token)
            .header("Dropbox-API-Arg", arg)
            .header("Content-Type", "application/octet-stream")
            .body(reqwest::Body::wrap_stream(body))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}
