//! Google Drive provider: the four Drive v3 REST calls the sync engine needs,
//! over a bearer access token and a shared `reqwest` client.
//!
//! Requests two scopes together: `drive.readonly` so a connection can import the
//! user's existing Drive files (the narrower `drive.file` scope alone only
//! exposes files the app created or the user explicitly opened, which cannot back
//! a general import), and `drive.file` so it can export by creating app-owned
//! files. `drive.readonly` is a Google *restricted* scope: production use
//! requires app verification and a CASA security assessment. The connection's
//! root folder, if set, scopes the listing (`'<root>' in parents`) and the
//! parent of exported files.

use bytes::Bytes;
use futures::stream::{self, StreamExt};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;

use super::response_stream;
use crate::client::{ByteStream, FileEntry, FileServiceClient};
use crate::error::{Error, Result};
use crate::oauth::OAuthProvider;

/// Provider identifier stored in the connection's `provider` column.
pub const PROVIDER_ID: &str = "google_drive";

/// Drive OAuth authorize endpoint.
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
/// Google OAuth token endpoint.
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
/// Read-only access to the user's Drive, so existing files can be imported.
/// A Google restricted scope (requires app verification + CASA assessment).
const DRIVE_READONLY_SCOPE: &str = "https://www.googleapis.com/auth/drive.readonly";
/// Per-file access for files the app creates, so exports can be uploaded.
const DRIVE_FILE_SCOPE: &str = "https://www.googleapis.com/auth/drive.file";

/// Drive v3 REST base.
const API_BASE: &str = "https://www.googleapis.com/drive/v3";
/// Drive v3 upload base (media/multipart uploads).
const UPLOAD_BASE: &str = "https://www.googleapis.com/upload/drive/v3";

/// The OAuth endpoints and scopes for Google Drive.
#[must_use]
pub fn oauth_provider() -> OAuthProvider {
    OAuthProvider {
        auth_url: AUTH_URL.to_owned(),
        token_url: TOKEN_URL.to_owned(),
        scopes: vec![DRIVE_READONLY_SCOPE.to_owned(), DRIVE_FILE_SCOPE.to_owned()],
        // Google issues a refresh token only with offline access and a forced
        // consent prompt.
        extra_authorize_params: vec![
            ("access_type".to_owned(), "offline".to_owned()),
            ("prompt".to_owned(), "consent".to_owned()),
        ],
    }
}

/// A connected Google Drive client, holding a valid access token.
pub struct DriveClient {
    http: reqwest::Client,
    access_token: String,
    /// The Drive folder id to scope listing and uploads to; `None` uses the
    /// user's root.
    root_folder_id: Option<String>,
}

impl DriveClient {
    /// Creates a client from an already-valid access token. Token refresh is the
    /// caller's responsibility, done before constructing this.
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

/// One page of a Drive `files.list` response.
#[derive(Debug, Deserialize)]
struct FileList {
    #[serde(default)]
    files: Vec<DriveFile>,
    #[serde(default, rename = "nextPageToken")]
    next_page_token: Option<String>,
}

/// A single Drive file's metadata.
#[derive(Debug, Deserialize)]
struct DriveFile {
    id: String,
    name: String,
    #[serde(default, rename = "mimeType")]
    mime_type: Option<String>,
}

/// Google-native document mime types (Docs, Sheets, ...) that have no direct
/// binary content and must be exported rather than downloaded; skipped on import.
fn is_google_native(mime_type: Option<&str>) -> bool {
    mime_type.is_some_and(|m| m.starts_with("application/vnd.google-apps."))
}

#[async_trait::async_trait]
impl FileServiceClient for DriveClient {
    async fn verify(&self) -> Result<()> {
        // Fetch the lightweight `about` resource: reachable and authorized iff
        // it returns, without listing or transferring any file.
        self.http
            .get(format!("{API_BASE}/about?fields=user(emailAddress)"))
            .bearer_auth(&self.access_token)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn list(&self) -> Result<Vec<FileEntry>> {
        // Files whose parent is the configured root (or the user's root), not
        // trashed, and not folders.
        let parent = self.root_folder_id.as_deref().unwrap_or("root");
        let query = format!(
            "'{parent}' in parents and trashed = false and mimeType != 'application/vnd.google-apps.folder'"
        );

        let mut entries = Vec::new();
        let mut page_token: Option<String> = None;
        loop {
            let mut request = self
                .http
                .get(format!("{API_BASE}/files"))
                .bearer_auth(&self.access_token)
                .query(&[
                    ("q", query.as_str()),
                    ("fields", "nextPageToken, files(id, name, mimeType)"),
                    ("pageSize", "1000"),
                ]);
            if let Some(token) = &page_token {
                request = request.query(&[("pageToken", token.as_str())]);
            }

            let page: FileList = request.send().await?.error_for_status()?.json().await?;

            entries.extend(
                page.files
                    .into_iter()
                    .filter(|f| !is_google_native(f.mime_type.as_deref()))
                    .map(|f| FileEntry {
                        id: f.id,
                        name: f.name,
                    }),
            );

            match page.next_page_token {
                Some(token) => page_token = Some(token),
                None => break,
            }
        }
        Ok(entries)
    }

    async fn get_stream(&self, id: &str) -> Result<ByteStream> {
        // Percent-encode the id into the path segment defensively; Drive ids are
        // normally URL-safe, but a stored key must never alter the request URL.
        let id = utf8_percent_encode(id, NON_ALPHANUMERIC);
        let response = self
            .http
            .get(format!("{API_BASE}/files/{id}?alt=media"))
            .bearer_auth(&self.access_token)
            .send()
            .await?
            .error_for_status()?;
        Ok(response_stream(response))
    }

    async fn put_stream(&self, name: &str, content_type: &str, body: ByteStream) -> Result<()> {
        // Drive's uploadType=multipart requires a `multipart/related` body (not
        // form-data): a JSON metadata part first, then the media part. reqwest's
        // multipart::Form only emits form-data, so build the related body by hand
        // and stream the media through it. The created file is app-owned
        // (drive.file scope), scoped to the connection's folder if set.
        let mut metadata = serde_json::json!({ "name": name });
        if let Some(parent) = &self.root_folder_id {
            metadata["parents"] = serde_json::json!([parent]);
        }
        let metadata = serde_json::to_vec(&metadata)
            .map_err(|err| Error::runtime("failed to encode upload metadata").with_source(err))?;

        let boundary = format!("nvisy-{}", uuid::Uuid::new_v4().simple());
        let mut preamble = Vec::new();
        preamble.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        preamble.extend_from_slice(b"Content-Type: application/json; charset=UTF-8\r\n\r\n");
        preamble.extend_from_slice(&metadata);
        preamble.extend_from_slice(format!("\r\n--{boundary}\r\n").as_bytes());
        preamble.extend_from_slice(format!("Content-Type: {content_type}\r\n\r\n").as_bytes());
        let epilogue = format!("\r\n--{boundary}--\r\n").into_bytes();

        // preamble bytes -> streamed media -> closing boundary, all one stream.
        let related = stream::once(async { Ok(Bytes::from(preamble)) })
            .chain(body)
            .chain(stream::once(async { Ok(Bytes::from(epilogue)) }));

        self.http
            .post(format!("{UPLOAD_BASE}/files?uploadType=multipart"))
            .bearer_auth(&self.access_token)
            .header(
                "Content-Type",
                format!("multipart/related; boundary={boundary}"),
            )
            .body(reqwest::Body::wrap_stream(related))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}
