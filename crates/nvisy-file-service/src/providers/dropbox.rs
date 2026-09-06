//! Dropbox provider: the file operations the sync engine needs, over the
//! Dropbox HTTP API v2 and a bearer access token.
//!
//! Dropbox splits its API across two hosts — metadata/JSON calls on
//! `api.dropboxapi.com`, byte transfers on `content.dropboxapi.com` — and passes
//! the per-call argument for transfers in a `Dropbox-API-Arg` header whose JSON
//! must be ASCII-safe. Files are addressed by a stable `id:...` key.

use futures::TryStreamExt;
use serde::Deserialize;

use crate::client::{ByteStream, FileEntry, FileServiceClient};
use crate::error::{Error, ErrorKind, kind_for_status};
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
    /// The folder path to import from; `None` (or empty) is the account root.
    root_path: Option<String>,
}

impl DropboxClient {
    /// Creates a client from an already-valid access token.
    pub fn new(http: reqwest::Client, access_token: String, root_path: Option<String>) -> Self {
        Self {
            http,
            access_token,
            root_path,
        }
    }
}

/// One page of a `files/list_folder` response.
#[derive(Debug, Deserialize)]
struct ListFolderPage {
    entries: Vec<Entry>,
    cursor: String,
    has_more: bool,
}

/// A single entry in a Dropbox listing.
#[derive(Debug, Deserialize)]
struct Entry {
    /// `file`, `folder`, or `deleted`.
    #[serde(rename = ".tag")]
    tag: String,
    name: String,
    /// Stable `id:...` identifier; absent on `deleted` tombstones.
    #[serde(default)]
    id: Option<String>,
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
    async fn verify(&self) -> Result<(), Error> {
        // get_current_account is the idiomatic whoami; it takes an empty body.
        self.http
            .post(format!("{API_BASE}/users/get_current_account"))
            .bearer_auth(&self.access_token)
            .send()
            .await
            .map_err(|err| Error::connection("Dropbox request failed").with_source(err))?
            .error_for_status()
            .map(|_| ())
            .map_err(map_reqwest_status)
    }

    async fn list(&self) -> Result<Vec<FileEntry>, Error> {
        // The root is the empty string; a configured folder uses a leading slash.
        let path = self.root_path.clone().unwrap_or_default();
        let mut entries = Vec::new();

        let mut page: ListFolderPage = self
            .http
            .post(format!("{API_BASE}/files/list_folder"))
            .bearer_auth(&self.access_token)
            .json(&serde_json::json!({ "path": path, "recursive": false }))
            .send()
            .await
            .map_err(|err| Error::runtime("Dropbox list request failed").with_source(err))?
            .error_for_status()
            .map_err(map_reqwest_status)?
            .json()
            .await
            .map_err(|err| Error::runtime("invalid Dropbox list response").with_source(err))?;

        loop {
            entries.extend(collect_files(&page.entries));
            if !page.has_more {
                break;
            }
            page = self
                .http
                .post(format!("{API_BASE}/files/list_folder/continue"))
                .bearer_auth(&self.access_token)
                .json(&serde_json::json!({ "cursor": page.cursor }))
                .send()
                .await
                .map_err(|err| Error::runtime("Dropbox list request failed").with_source(err))?
                .error_for_status()
                .map_err(map_reqwest_status)?
                .json()
                .await
                .map_err(|err| Error::runtime("invalid Dropbox list response").with_source(err))?;
        }
        Ok(entries)
    }

    async fn get_stream(&self, id: &str) -> Result<ByteStream, Error> {
        let arg = api_arg(&serde_json::json!({ "path": id }));
        let response = self
            .http
            .post(format!("{CONTENT_BASE}/files/download"))
            .bearer_auth(&self.access_token)
            .header("Dropbox-API-Arg", arg)
            .send()
            .await
            .map_err(|err| Error::runtime("Dropbox download request failed").with_source(err))?
            .error_for_status()
            .map_err(map_reqwest_status)?;

        let stream = response
            .bytes_stream()
            .map_err(|err| Error::runtime("Dropbox download stream failed").with_source(err));
        Ok(Box::pin(stream))
    }

    async fn put_stream(
        &self,
        name: &str,
        _content_type: &str,
        body: ByteStream,
    ) -> Result<(), Error> {
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
            .await
            .map_err(|err| Error::runtime("Dropbox upload request failed").with_source(err))?
            .error_for_status()
            .map(|_| ())
            .map_err(map_reqwest_status)
    }
}

/// Maps a listing's entries to importable file entries, keyed by stable id.
fn collect_files(entries: &[Entry]) -> Vec<FileEntry> {
    entries
        .iter()
        .filter(|e| e.tag == "file")
        .filter_map(|e| {
            e.id.clone().map(|id| FileEntry {
                id,
                name: e.name.clone(),
            })
        })
        .collect()
}

/// Maps a reqwest status error into a classified [`Error`].
fn map_reqwest_status(err: reqwest::Error) -> Error {
    let kind = err
        .status()
        .map_or(ErrorKind::Runtime, |s| kind_for_status(s.as_u16()));
    Error::new(kind, "Dropbox returned an error").with_source(err)
}
