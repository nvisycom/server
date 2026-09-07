//! Google Drive provider: the Drive v3 REST calls the sync engine needs, over a
//! bearer access token and a shared `reqwest` client.
//!
//! Requests only `drive.file`, Google's least-privilege Drive scope: it grants
//! per-file access to files the app creates and files the user hands it through
//! the Google Picker. That covers both directions here — imports are the files
//! the user picks, exports are new app-created files — without the restricted
//! `drive.readonly` scope and its app-verification + CASA assessment. The
//! connection's root folder, if set, is the parent of exported files.

use bytes::Bytes;
use futures::stream::{self, StreamExt};

use super::{encode_path_segment, response_stream};
use crate::client::{ByteStream, FileServiceClient};
use crate::error::{Error, Result};
use crate::oauth::OAuthProvider;

/// Provider identifier stored in the connection's `provider` column.
pub const PROVIDER_ID: &str = "google_drive";

/// Drive OAuth authorize endpoint.
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
/// Google OAuth token endpoint.
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
/// Per-file access: files the app creates and files the user grants through the
/// picker. Google's least-privilege Drive scope; covers both import and export.
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
        scopes: vec![DRIVE_FILE_SCOPE.to_owned()],
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
    /// The Drive folder id that new exports are created in; `None` uses the
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

    async fn get_stream(&self, id: &str) -> Result<ByteStream> {
        // Percent-encode the id into the path segment defensively; Drive ids are
        // normally URL-safe, but a stored key must never alter the request URL.
        let id = encode_path_segment(id);
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
