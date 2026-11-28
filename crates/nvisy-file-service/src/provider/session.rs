//! An authorized session against one provider: the shared HTTP client, the
//! access token, and the provider every request is labelled with.

use futures::StreamExt;
use nvisy_core::{Error, Result, Secret, kind_for_status};
use reqwest::{IntoUrl, RequestBuilder, Response};

use super::{FileServiceProvider, TRACING_TARGET};

/// Cap on how many bytes of a failed provider response body are read, so a large
/// or unbounded error body cannot cause memory pressure or flood the logs.
/// Comfortably covers the JSON error objects providers return.
const MAX_ERROR_BODY_BYTES: usize = 8 * 1024;

/// Most characters of a provider's error body carried into the error message.
const MAX_ERROR_DETAIL_CHARS: usize = 300;

/// An authorized session against one provider.
///
/// Every request it builds carries the bearer token, and every request goes
/// through [`send`](Self::send), so a failure is classified by status and
/// carries the provider's own error body rather than an opaque status.
pub(crate) struct Session {
    http: reqwest::Client,
    access_token: Secret<String>,
    provider: FileServiceProvider,
}

impl Session {
    /// Opens a session for `provider` with an already-valid access token.
    pub(crate) fn new(
        http: reqwest::Client,
        access_token: Secret<String>,
        provider: FileServiceProvider,
    ) -> Self {
        Self {
            http,
            access_token,
            provider,
        }
    }

    /// An authorized `GET` request to `url`.
    pub(crate) fn get(&self, url: impl IntoUrl) -> RequestBuilder {
        self.http.get(url).bearer_auth(self.access_token.expose())
    }

    /// An authorized `POST` request to `url`.
    pub(crate) fn post(&self, url: impl IntoUrl) -> RequestBuilder {
        self.http.post(url).bearer_auth(self.access_token.expose())
    }

    /// An authorized `PUT` request to `url`.
    pub(crate) fn put(&self, url: impl IntoUrl) -> RequestBuilder {
        self.http.put(url).bearer_auth(self.access_token.expose())
    }

    /// Sends `request` and checks its status. On a non-2xx, reads a bounded
    /// prefix of the provider's response body, logs it, and returns an error
    /// carrying it, classified by status; on success, returns the response for
    /// the caller to consume.
    ///
    /// `reqwest`'s own `error_for_status` drops the body, but providers (Dropbox
    /// especially) put the actual reason there.
    ///
    /// # Errors
    ///
    /// An error classified by the response status, or by the transport failure
    /// if no response arrived.
    pub(crate) async fn send(&self, request: RequestBuilder) -> Result<Response> {
        let response = request.send().await?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }

        let provider: &'static str = self.provider.into();
        let body = read_bounded_body(response).await;
        tracing::warn!(
            target: TRACING_TARGET,
            provider, status = status.as_u16(), body = %body,
            "Provider request failed",
        );
        let detail: String = body.trim().chars().take(MAX_ERROR_DETAIL_CHARS).collect();
        let message = if detail.is_empty() {
            format!("{provider} request failed with status {}", status.as_u16())
        } else {
            format!("{provider}: {detail}")
        };
        Err(Error::new(kind_for_status(status.as_u16()), message))
    }
}

/// Reads at most [`MAX_ERROR_BODY_BYTES`] of a response body, then stops.
/// Best-effort and lossy by design: it only surfaces a diagnostic prefix, so a
/// stream error mid-read yields whatever was read, decoded lossily.
async fn read_bounded_body(response: Response) -> String {
    let mut stream = response.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    while buf.len() < MAX_ERROR_BODY_BYTES {
        match stream.next().await {
            Some(Ok(chunk)) => {
                let take = (MAX_ERROR_BODY_BYTES - buf.len()).min(chunk.len());
                buf.extend_from_slice(&chunk[..take]);
            }
            Some(Err(_)) | None => break,
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}
